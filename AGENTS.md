# Project Overview

expad is a Rust firmware project for an RP2350 target. It initializes a shift-register buffer chain and an ADC chain, measures analog voltages, and infers resistance distribution across three arms with a small topology solver. On the Pico 2 W it can also open a WiFi access point serving a live web interface (Vue app in web/) that shows device status and edits settings in real time.

## Repository Structure

These files are also the extensibility hooks: the types and functions named here are where new behavior goes.

- .vscode/: build, run, and debug tasks plus launch configuration; with Embed.toml, the main extension point for flashing and debugging.
- src/lib.rs: `#![no_std]` library crate (`expad`), re-exporting `hal`, `topology`, and (with the `web` feature) `web` to every binary.
- src/bin/: one flashable application per file, each with its own `#[embassy_executor::main]` — see "Adding a new application".
  - `capture.rs`: the entry point today — configures the SPI shift-register chain, clears the quad-buffer outputs, initializes the ADC chain for direct channel measurements, then loops over continuous capture.
  - `detect_pin_mapping.rs`: maps buffer outputs to ADC channels.
  - `rainbow.rs`: cycles a rainbow pattern across a WS2812B strip.
  - `potentiometer.rs`: drives buffer outputs 0 and 2 to the low/high rails, measures the floating wiper on channel 1 against them, prints voltage and position, and mirrors it by splitting brightness across the two nearest LEDs.
  - `midi_loopback.rs`: echoes every USB MIDI packet back to the host.
  - `web_interface.rs`: serves the web interface over WiFi with dummy status data, logging every settings change.
- src/hal/mod.rs: hardware abstraction layer, re-exporting:
  - adc/: AD7718 chain driver, register abstractions, measurement flow; mod.rs exposes `AdcChainConfig` for new channels or modes.
  - buf/: output channels — quad_buffer.rs (tri-state control, `TriState` model, output-state encoding), shift_register.rs (SPI shift-register wrapper).
  - led/: ws2812.rs defines `Ws2812Chain`, a PIO-backed WS2812B ("NeoPixel") driver generic over LED count; strip.rs defines `LedStrip`, the stateful per-LED color and global-brightness driver over it used by `rainbow` and `potentiometer`.
  - usb/midi.rs: USB MIDI over `embassy-usb`'s MIDI class with `usbd-midi` packet and message types. Defines `UsbMidiConfig` and `UsbMidi` (`receive`, `send_packet`, `send_message`); `UsbMidi::new` also returns a `UsbMidiDevice` whose `run()` must be polled concurrently (e.g. with `join`).
  - wifi/ (`web` only): access_point.rs drives the Pico 2 W CYW43439 on PIO1 plus the `embassy-net` stack — `start_access_point`, `AccessPointConfig` (SSID, password, channel, address), `AccessPointPeripherals`; dhcp.rs is its DHCP server.
- src/topology/solver.rs: resistance-solving logic over ADC measurements, and the place to extend it. Implements tri-state toggling and resistance inference, but no binary calls it yet.
- src/web/ (`web` feature): interface.rs defines the protocol types (`Status`, `JackStatus`, `Settings`, `JackSettings`) and the global `INTERFACE` state; server.rs defines `spawn_web_server` and the `picoserve` HTTP routes serving the embedded UI at `/` and a WebSocket at `/ws`.
- web/: frontend (Vue 3, Vite, Tailwind CSS 4, PrimeVue 5, VueUse, TypeScript) built into a single gzipped index.html. src/interface.ts mirrors the Rust protocol types, src/composables/useInterface.ts owns the WebSocket, src/App.vue and src/components/ render the UI, mock/device.ts is the firmware stand-in, and vite.config.ts configures the gzip build and dev proxy.
- firmware/cyw43/: vendored CYW43439 firmware blobs (Infineon permissive binary license) embedded by the wifi HAL; .gitattributes marks these `*.bin` files binary so line endings are never converted.
- build.rs: copies linker settings into the build output, forwards `EXPAD_*` variables from the environment or `.env` to the crate, and with `web` validates `EXPAD_WIFI_PASSWORD` and runs `npm ci`/`npm run build` in web/, writing to `OUT_DIR/web`.
- .env.example: template for the untracked `.env` (WiFi password, PrimeUI license key, dev proxy target).
- Cargo.toml: manifest and embedded dependencies — the `expad` lib target, one `[[bin]]` per file in src/bin/, and the default `web` feature gating the networking dependencies.
- Embed.toml, memory.x, rp235x_riscv.x: board and linker configuration for the RP2350.

## Build & Development Commands

```bash
cargo build --bin capture   # or: cargo build (builds the lib + every bin)
cargo build --no-default-features   # skip the `web` feature (no Node.js needed)
cargo fmt
cargo clippy --all-features
cargo test

cd web
npm run dev:mock     # dev server + mock device: develop the UI with live dummy data, no hardware needed
npm run dev          # dev server against real hardware, proxies /ws to EXPAD_DEVICE_ADDRESS (default 192.168.4.1)
npm run type-check
npm run lint
```

The default `web` feature needs Node.js (`engines` in [web/package.json](web/package.json)); build.rs reruns `npm ci` whenever `web/package-lock.json` is newer than the installed packages. Before building, copy [.env.example](.env.example) to `.env` and set the WiFi password — required, the build fails without a valid 8-63 character WPA2 password — plus, optionally, the PrimeUI license key.

Debug and flash from VS Code with [.vscode/launch.json](.vscode/launch.json) and [.vscode/tasks.json](.vscode/tasks.json); both prompt with a dropdown of `src/bin/*.rs` programs via a shared `binName` input, so building, running, and debugging target the same binary. `cargo run --bin <name>` uploads that firmware to the RP2350 and captures serial output until canceled with Ctrl-C.

### Adding a new application

1. Add `src/bin/<name>.rs` with `#![no_std]`, `#![no_main]`, and its own `#[embassy_executor::main]`, importing shared code via `expad::hal::...` / `expad::topology::...`.
2. Add a matching `[[bin]]` entry to Cargo.toml (`name = "<name>"`, `path = "src/bin/<name>.rs"`, `test = false`, `doctest = false`).
3. Append `"<name>"` to the `binName` input's `options` in both .vscode/tasks.json and .vscode/launch.json so it shows up in the picker.

### Adding the web interface to an application

See [src/bin/web_interface.rs](src/bin/web_interface.rs) for a complete example:

1. Add `required-features = ["web"]` to the binary's `[[bin]]` entry.
2. Bind `PIO1_IRQ_0 => pio::InterruptHandler<PIO1>` and `DMA_IRQ_0 => dma::InterruptHandler<DMA_CHx>` for the chosen DMA channel. All DMA channels share `DMA_IRQ_0`, so list every channel's handler there (e.g. `DMA_CH0` for the LED strip, `DMA_CH1` for WiFi).
3. `let stack = start_access_point(spawner, AccessPointPeripherals { .. }, Irqs, AccessPointConfig::default()).await;`, then `spawn_web_server(spawner, stack);`.
4. Publish measurements with `INTERFACE.status.sender().send(status)`, take one `INTERFACE.settings.receiver()` at startup, and apply every `settings.changed().await`. Sending to `INTERFACE.settings` (e.g. values loaded from flash) updates every open interface.

To extend the protocol, change [src/web/interface.rs](src/web/interface.rs) and [web/src/interface.ts](web/src/interface.ts) together.

## Code Style & Conventions

- Rust 2024 edition conventions; small, explicit modules, `cargo fmt` formatting, code readable for embedded development.
- Non-abbreviated, self-descriptive names; avoid single-letter ones outside very local contexts (e.g. loop indices).
- Prefer self-documenting code over comments, splitting larger expressions into named variables to clarify intent. Prefer `Result`-based error handling and typed config structs over ad-hoc values, and keep hardware-facing logic close to its module, such as ADC or buffer handling.
- In web/, write `<script setup lang="ts">` single-file components, import PrimeVue components individually (`primevue/<name>`) and icons from `@primeicons/vue/<name>`, style with Tailwind utilities (including `tailwindcss-primeui` color tokens), and prefer VueUse composables over hand-written browser glue. Keep `npm run type-check` and `npm run lint` clean.

## Architecture Notes

Shared drivers and logic live in the `expad` library crate, which every file under src/bin/ depends on:

```text
expad (lib): hal::{adc, buf, led, usb}, topology::solver

capture, detect_pin_mapping -> ShiftRegisterChain, QuadBufferChain, AdcChain
rainbow                     -> LedStrip -> Ws2812Chain
potentiometer               -> ShiftRegisterChain, QuadBufferChain, AdcChain, LedStrip -> Ws2812Chain
midi_loopback               -> UsbMidi (+ UsbMidiDevice run future)
web_interface               -> start_access_point (cyw43 + embassy-net + DHCP tasks)
                            -> spawn_web_server (picoserve tasks)
                               <-> INTERFACE (status/settings watches) <-> browser (web/, WebSocket /ws)
topology::ResistanceSolver  -> AdcChain, QuadBufferChain   (not yet invoked from any binary)
```

The web interface separates WiFi transport from server. `start_access_point` runs the CYW43439 as a WPA2-protected access point at 192.168.4.1/24 and spawns the WiFi, network, and DHCP tasks; `spawn_web_server` takes any `embassy_net::Stack` and spawns `MAX_SESSIONS` picoserve tasks on port 80. Each WebSocket session sends `{"settings": ...}` on connect, then `{"status": ...}` or `{"settings": ...}` whenever the matching `embassy_sync::watch::Watch` in `INTERFACE` changes; every text message from the browser is a full `Settings` JSON object, broadcast to the firmware and all sessions. Non-finite floats (e.g. in `ArmResistances::DISCONNECTED`) serialize as `null`.

## Testing Strategy

- No dedicated test suite yet. Add unit tests for the topology solver and register encoding when behavior changes, and run `cargo test` before merging.
- For web interface changes, check the browser under `npm run dev:mock`, then against a flashed `web_interface` device with `npm run dev`. Keep [web/mock/device.ts](web/mock/device.ts), which speaks the firmware's protocol, in sync with protocol changes.
- Validate hardware behavior on-device with the existing debug/RTT setup in [.vscode/launch.json](.vscode/launch.json).

## Security & Compliance

- Keep secrets, credentials, and private board-specific values out of source control. The WiFi password and PrimeUI license key live in the untracked `.env`; both are compiled into the firmware image, and the license key is visible in the served page.
- PrimeVue 5 and `@primeicons/vue` use the proprietary PrimeUI license (free Community License with a yearly key), not MIT.
- Keep Cargo.lock, web/package-lock.json, and the existing LICENSE for redistributed code in place; update dependencies intentionally.
- Review any pin, SPI, or register change carefully, since it affects hardware behavior.

## Agent Guardrails

- Do not change linker scripts, board targets, or pin assignments without verifying the hardware implications.
- Avoid broad rewrites of the ADC or buffer abstractions unless justified and tested.
- Prefer small, reviewable edits, verified with `cargo build` first.
- Do not modify generated artifacts under target/ directly.

## Maintaining This File

- Update it in the same change as any larger one: new, renamed, or removed modules, binaries, features, dependencies, build steps, commands, or protocols. Small fixes inside an existing file need no update.
- Touch only the affected lines, usually in Repository Structure, Build & Development Commands, and Architecture Notes.
- Keep it condensed: one line per item, no changelog, history, rationale, or anything the code already states. Rewrite or delete stale lines instead of appending to them.
