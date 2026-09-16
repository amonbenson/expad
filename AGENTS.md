# Project Overview

expad is a Rust firmware project for an RP2350-based embedded target. It initializes a shift-register buffer chain and an ADC chain, then measures analog voltages and infers resistance distribution across three arms using a small topology solver. On the Pico 2 W it can also open a WiFi access point and serve a live web interface (Vue web app in web/) that shows the device status and edits its settings in real time.

## Repository Structure

- .vscode/: VS Code tasks and launch configuration for building, running, and debugging the firmware, with a picker for which `src/bin/*.rs` program to target.
- src/: firmware code, split into a shared library and one binary per application.
  - src/lib.rs: `#![no_std]` library crate (`expad`) that re-exports `hal`, `topology`, and (with the `web` feature) `web` for every binary to share.
  - src/bin/: one file per flashable application, each with its own `#[embassy_executor::main]`. `capture.rs` holds the original main-loop firmware (shift-register/buffer init, ADC init, direct measurements, continuous capture); `detect_pin_mapping.rs` maps buffer outputs to ADC channels; `rainbow.rs` drives a WS2812B strip through a rainbow pattern; `potentiometer.rs` buffers ADC channels 0 and 2 to the low/high rails, reads a potentiometer wiper on channel 1, and shows its position on the LED strip; `midi_loopback.rs` echoes every USB MIDI packet it receives back to the host; `web_interface.rs` serves the web interface over WiFi with dummy status data and logs every settings change.
  - src/hal/adc/: ADC chain driver, register abstractions, and measurement flow for the AD7718 devices.
  - src/hal/buf/: tri-state buffer control (quad_buffer.rs) and the SPI shift-register wrapper (shift_register.rs) for output channels.
  - src/hal/led/: PIO-backed WS2812B ("NeoPixel") LED strip driver (ws2812.rs), configurable by LED count, plus a stateful per-LED color and brightness driver on top of it (strip.rs).
  - src/hal/usb/: USB MIDI device driver (midi.rs) built on `embassy-usb`'s MIDI class, using `usbd-midi` for packet and message types.
  - src/hal/wifi/: Pico 2 W CYW43439 access point (access_point.rs: WiFi driver on PIO1 plus the `embassy-net` stack) and its DHCP server (dhcp.rs). Only built with the `web` feature.
  - src/hal/mod.rs: hardware abstraction layer module that re-exports the adc, buf, led, usb, and wifi submodules.
  - src/topology/: resistance-solving logic that interprets ADC measurements.
  - src/web/: web interface backend (`web` feature). interface.rs defines the `Status`/`Settings` protocol types and the global `INTERFACE` state; server.rs runs the `picoserve` HTTP server that serves the embedded UI at `/` and a WebSocket at `/ws`.
- web/: web interface frontend (Vue 3, Vite, Tailwind CSS 4, PrimeVue 5, VueUse, TypeScript). src/interface.ts mirrors the Rust protocol types, src/composables/useInterface.ts owns the WebSocket connection, and src/App.vue plus src/components/ render the UI. It builds into a single gzipped index.html.
- firmware/cyw43/: vendored CYW43439 WiFi firmware blobs (Infineon permissive binary license) embedded by the wifi HAL.
- build.rs: copies linker settings into the build output, forwards `EXPAD_*` variables from the environment or `.env` to the crate, and (with the `web` feature) validates `EXPAD_WIFI_PASSWORD` and runs `npm ci`/`npm run build` in web/, writing the web interface to `OUT_DIR/web`.
- .gitattributes: marks the vendored `*.bin` firmware blobs as binary so line-ending conversion never touches them.
- .env.example: template for the untracked `.env` (WiFi password, PrimeUI license key, dev proxy target).
- Cargo.toml: crate manifest and embedded dependencies. Declares the `expad` lib target plus one `[[bin]]` entry per file in src/bin/, and the default `web` feature that gates the networking dependencies.
- Embed.toml, memory.x, rp235x_riscv.x: board and linker configuration for the RP2350 target.

## Build & Development Commands

```bash
cargo build --bin capture   # or: cargo build (builds the lib + every bin)
cargo build --no-default-features   # skip the `web` feature (no Node.js needed)
cargo fmt
cargo clippy --all-features
cargo test

cd web
npm run dev          # web interface dev server, proxies /ws to EXPAD_DEVICE_ADDRESS (default 192.168.4.1)
npm run type-check
npm run lint
```

The default `web` feature needs Node.js (see `engines` in [web/package.json](web/package.json)). build.rs runs `npm ci` whenever `web/package-lock.json` is newer than the installed packages. Copy [.env.example](.env.example) to `.env` and fill in the WiFi password (required: the build fails without a valid 8-63 character WPA2 password) and optionally the PrimeUI license key before building.

Debug and flash from VS Code using the existing configuration in [.vscode/launch.json](.vscode/launch.json) and [.vscode/tasks.json](.vscode/tasks.json) — both prompt with a dropdown of the available `src/bin/*.rs` programs via a shared `binName` input, so building, running, and debugging all target the same chosen binary.
Use `cargo run --bin <name>` to upload the chosen firmware to the RP2350 target and capture serial output. Needs to be canceled with Ctrl-C to stop the capture.

### Adding a new application

1. Add `src/bin/<name>.rs` with `#![no_std]`, `#![no_main]`, and its own `#[embassy_executor::main]`, importing shared code via `expad::hal::...` / `expad::topology::...`.
2. Add a matching `[[bin]]` entry to [Cargo.toml](Cargo.toml) (`name = "<name>"`, `path = "src/bin/<name>.rs"`, `test = false`, `doctest = false`).
3. Append `"<name>"` to the `binName` input's `options` in both [.vscode/tasks.json](.vscode/tasks.json) and [.vscode/launch.json](.vscode/launch.json) so it shows up in the picker.

### Adding the web interface to an application

See [src/bin/web_interface.rs](src/bin/web_interface.rs) for a complete example:

1. Add `required-features = ["web"]` to the binary's `[[bin]]` entry.
2. Bind `PIO1_IRQ_0 => pio::InterruptHandler<PIO1>` and `DMA_IRQ_0 => dma::InterruptHandler<DMA_CHx>` for the chosen DMA channel. All DMA channels share `DMA_IRQ_0`, so list every channel's handler there (e.g. `DMA_CH0` for the LED strip and `DMA_CH1` for WiFi).
3. `let stack = start_access_point(spawner, AccessPointPeripherals { .. }, Irqs, AccessPointConfig::default()).await;`, then `spawn_web_server(spawner, stack);`.
4. Publish measurements with `INTERFACE.status.sender().send(status)`, take one `INTERFACE.settings.receiver()` at startup, and apply every `settings.changed().await`. Sending to `INTERFACE.settings` (e.g. values loaded from flash) updates every open interface.

To extend the protocol, change [src/web/interface.rs](src/web/interface.rs) and [web/src/interface.ts](web/src/interface.ts) together.

## Code Style & Conventions

- Use Rust 2024 edition conventions and keep modules small and explicit.
- Use non-abbreviated, self-descriptive names for functions, types, and variables. Avoid single-letter names except in very local contexts (e.g., loop indices).
- Use self-documenting code over comments whereever possible. Separate larger expressions into named variables to clarify intent.
- Prefer `Result`-based error handling and typed config structs over ad-hoc values.
- Keep hardware-facing logic close to the relevant module, such as ADC or buffer handling.
- Use `cargo fmt` for formatting and keep code readable for embedded development.
- In web/, write `<script setup lang="ts">` single-file components, import PrimeVue components individually (`primevue/<name>`) and icons from `@primeicons/vue/<name>`, style with Tailwind utilities (including the `tailwindcss-primeui` color tokens), and prefer VueUse composables over hand-written browser glue. Keep `npm run type-check` and `npm run lint` clean.

## Architecture Notes

```text
expad (lib)
  hal::{adc, buf, led, usb}
  topology::solver
bin/capture
  -> ShiftRegisterChain
  -> QuadBufferChain
  -> AdcChain
bin/detect_pin_mapping
  -> ShiftRegisterChain, QuadBufferChain
  -> AdcChain
bin/rainbow
  -> LedStrip
    -> Ws2812Chain
bin/potentiometer
  -> ShiftRegisterChain, QuadBufferChain
  -> AdcChain
  -> LedStrip
    -> Ws2812Chain
bin/midi_loopback
  -> UsbMidi (+ UsbMidiDevice run future)
bin/web_interface
  -> start_access_point (cyw43 + embassy-net + DHCP tasks)
  -> spawn_web_server (picoserve tasks)
    <-> INTERFACE (status / settings watches) <-> browser (web/, WebSocket /ws)
topology::ResistanceSolver (not yet invoked from any binary)
  -> AdcChain, QuadBufferChain
```

Shared drivers and logic live in the `expad` library crate ([src/lib.rs](src/lib.rs)), which every file under [src/bin/](src/bin/) depends on. The `capture` binary ([src/bin/capture.rs](src/bin/capture.rs)) is the firmware entry point today: it configures the SPI-based shift-register chain, clears the quad-buffer outputs, and initializes the ADC chain to take direct channel measurements and then loop over continuous capture. The `rainbow` binary ([src/bin/rainbow.rs](src/bin/rainbow.rs)) drives a WS2812B strip via `LedStrip` (which tracks per-LED color state and a global brightness on top of the low-level `Ws2812Chain` PIO driver) and cycles a rainbow pattern across it. The `potentiometer` binary ([src/bin/potentiometer.rs](src/bin/potentiometer.rs)) drives buffer outputs 0 and 2 to the low/high rails, continuously measures the floating wiper on channel 1 relative to those rails, prints the resulting voltage and position, and mirrors the position on the LED strip by splitting brightness between the two nearest LEDs. The topology solver in [src/topology/solver.rs](src/topology/solver.rs) implements the tri-state toggling and resistance-inference logic but is not yet called from any binary.

The web interface separates the WiFi transport from the server. `start_access_point` ([src/hal/wifi/access_point.rs](src/hal/wifi/access_point.rs)) runs the CYW43439 as a WPA2-protected access point at 192.168.4.1/24 and spawns the WiFi, network, and DHCP tasks. `spawn_web_server` ([src/web/server.rs](src/web/server.rs)) accepts any `embassy_net::Stack` and spawns `MAX_SESSIONS` picoserve tasks on port 80. Each WebSocket session sends `{"settings": ...}` on connect, then `{"status": ...}` or `{"settings": ...}` whenever the matching `embassy_sync::watch::Watch` in `INTERFACE` ([src/web/interface.rs](src/web/interface.rs)) changes. Every text message from the browser is a full `Settings` JSON object and is broadcast to the firmware and all sessions. Non-finite floats (e.g. in `ArmResistances::DISCONNECTED`) are serialized as `null`.

## Testing Strategy

- No dedicated test suite is present yet.
- Add unit tests for the topology solver and register encoding logic when behavior changes.
- Run `cargo test` locally before merging changes.
- For web interface changes, run `npm run dev` in web/ against a flashed `web_interface` device, or against any WebSocket server that speaks the same protocol (set `EXPAD_DEVICE_ADDRESS`).
- Validate hardware behavior on-device with the existing debug/RTT setup in [.vscode/launch.json](.vscode/launch.json).

## Security & Compliance

- Keep secrets, credentials, and private board-specific values out of source control. The WiFi password and PrimeUI license key live in the untracked `.env`. Both are compiled into the firmware image, and the license key is visible in the served page.
- PrimeVue 5 and `@primeicons/vue` use the proprietary PrimeUI license (free Community License with a yearly key), not MIT.
- Keep [web/package-lock.json](web/package-lock.json) checked in.
- Review any pin, SPI, or register changes carefully because they affect hardware behavior.
- Keep [Cargo.lock](Cargo.lock) checked in and update dependencies intentionally.
- Preserve the existing license in [LICENSE](LICENSE) for redistributed code.

## Agent Guardrails

- Do not change linker scripts, board targets, or pin assignments without verifying the hardware implications.
- Avoid broad rewrites of the ADC or buffer abstractions unless the change is justified and tested.
- Prefer small, reviewable edits and verify them with `cargo build` first.
- Do not modify generated artifacts under [target](target) directly.

## Extensibility Hooks

- [src/hal/adc/mod.rs](src/hal/adc/mod.rs) exposes `AdcChainConfig` and the ADC measurement flow for new channels or modes.
- [src/hal/buf/quad_buffer.rs](src/hal/buf/quad_buffer.rs) defines the `TriState` model and output-state encoding for new buffer behavior.
- [src/hal/led/ws2812.rs](src/hal/led/ws2812.rs) defines `Ws2812Chain`, generic over the LED count, for driving WS2812B strips from a PIO block.
- [src/hal/led/strip.rs](src/hal/led/strip.rs) defines `LedStrip`, the stateful per-LED color and global-brightness driver built on top of `Ws2812Chain`.
- [src/hal/usb/midi.rs](src/hal/usb/midi.rs) defines `UsbMidi` (`receive`, `send_packet`, `send_message`) and `UsbMidiConfig`; `UsbMidi::new` also returns the `UsbMidiDevice`, whose `run()` future must be polled concurrently (e.g. with `join`).
- [src/topology/solver.rs](src/topology/solver.rs) is the main place to extend resistance-solving logic.
- [src/hal/wifi/access_point.rs](src/hal/wifi/access_point.rs) defines `start_access_point`, `AccessPointConfig` (SSID, password, channel, address), and `AccessPointPeripherals`.
- [src/web/interface.rs](src/web/interface.rs) defines the web interface protocol (`Status`, `JackStatus`, `Settings`, `JackSettings`) and the shared `INTERFACE` state; [src/web/server.rs](src/web/server.rs) defines the routes and `spawn_web_server`.
- [web/src/](web/src/) holds the web interface UI; [web/vite.config.ts](web/vite.config.ts) configures the single-file gzip build and the dev proxy.
- [src/bin/](src/bin/) is where new applications go — see "Adding a new application" above.
- [Embed.toml](Embed.toml) and [.vscode/launch.json](.vscode/launch.json) are the main extension points for flashing and debugging.

## Further Reading

- [Cargo.toml](Cargo.toml)
- [Embed.toml](Embed.toml)
- [src/lib.rs](src/lib.rs)
- [src/bin/capture.rs](src/bin/capture.rs)
- [src/hal/adc/mod.rs](src/hal/adc/mod.rs)
- [src/hal/buf/mod.rs](src/hal/buf/mod.rs)
- [src/topology/solver.rs](src/topology/solver.rs)
- [src/web/server.rs](src/web/server.rs)
- [web/package.json](web/package.json)
