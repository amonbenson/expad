# Project Overview

expad is a Rust firmware project for an RP2350 target. On the expression controller PCB (hardware/, KiCad) it switches every jack contact to shared pull-up/pull-down resistors through shift-register-driven analog switches, reads the contacts back through an ADC chain, and identifies what is plugged into each jack with a small topology solver, then follows it with single readings - a potentiometer pedal's wiper, or a switch's or rheostat's tip - sent as MIDI control changes over USB. On the Pico 2 W it can also open a WiFi access point serving a live web interface (Vue app in web/) that shows device status and edits settings in real time. The workspace's second crate, topology/, holds the solver's arithmetic without any hardware dependency, so it can be unit tested on the host.

## Repository Structure

These files are also the extensibility hooks: the types and functions named here are where new behavior goes.

- .vscode/: build, run, and debug tasks plus launch configuration; with Embed.toml, the main extension point for flashing and debugging. settings.json turns on format-on-save (rustfmt for Rust, the ESLint extension for web/) and extensions.json recommends the extensions that provide it.
- src/lib.rs: `#![no_std]` library crate (`expad`), re-exporting `board`, `hal`, `topology`, and (with the `web` feature) `web` to every binary.
- src/board.rs: the PCB's wiring and the only place that knows it - pins, chip counts, `REFERENCE_VOLTAGE`, pull and ADC filter values, `Contact` (tip, ring, sleeve, tip switch) and the `JACKS` table of `JackWiring` (switch chip, ADC chip, ADC channel per contact; `config()` gives the scanner's `JackConfig`). `pull_switches`, `adcs` and `leds` construct the drivers on the board's pins.
- src/bin/: one flashable application per file, each with its own `#[embassy_executor::main]` — see "Adding a new application".
  - `adc_characterization.rs`: with a pot pedal in jack 1 driven like a tracked one, compares SPI settings (clock, chip select guard) at 819 Hz by reading time, noise and outlying codes, then measures reading time, noise, resolution, mains hum and offsets at every filter rate, and whether calibration survives a rate change; results in docs/fast-tracking.md.
  - `capture.rs`: holds every jack's contacts in the `DRIVE` pattern (all floating, all low/high, or one high and one low to reproduce a solver pair on a pedal), reads each ADC input once, then continuously captures both ADCs at the controller's 819 Hz with bipolar coding, so the noise around 0 V is visible - the tool for measuring `ADC_VOLTAGE_NOISE`.
  - `detect_pin_mapping.rs`: board self-test - checks each ADC's grounded (AIN6) and reference (AIN10) input, then pulls every contact of every jack up and down, verifying against `board::JACKS` that exactly the expected ADC input follows (the tip switch also follows the tip while no plug is inserted).
  - `rainbow.rs`: cycles a rainbow pattern across a WS2812B strip.
  - `potentiometer.rs`: pulls the first jack's sleeve low and ring high, measures the tip (wiper) against them, prints voltage and position, and mirrors it by splitting brightness across the two nearest LEDs.
  - `midi_loopback.rs`: echoes every USB MIDI packet back to the host.
  - `web_interface.rs`: serves the web interface over WiFi with dummy status data, logging every settings change.
  - `expression_controller.rs`: the full firmware - runs a `JackScanner` over every jack, maps each position through the jack's range, inversion and drive settings, sends it as a MIDI control change, lights the jack's LED in its color (green while the pedal moves or a switch is pressed) and publishes the status to the web interface (~30 Hz); logs position updates per second every 5 s.
- src/hal/mod.rs: hardware abstraction layer, re-exporting:
  - adc/: AD7718 chain driver, register abstractions, measurement flow; mod.rs exposes `AdcChainConfig` for new channels, modes and SPI timing (`with_spi_frequency`, `with_chip_select_guard`; 4 MHz and 10 µs by default), skips rewriting the control register while the channel stays the same, `start_conversion`/`finish_conversion` and `measure_parallel` (one channel per chip, all chips converting at once), `set_update_rate` (keeps the old calibration) and `measure_continuous` (one channel, back-to-back results).
  - buf/: pull switches — pull_switch.rs (`PullSwitchChain`: one 74HC595 per jack driving two TMUX1511s, `TriState` per tap and its bit encoding, chip 0 nearest the MCU), shift_register.rs (SPI shift-register wrapper; outputs stay disabled until the first write, then latch atomically).
  - led/: ws2812.rs defines `Ws2812Chain`, a PIO-backed WS2812B ("NeoPixel") driver generic over LED count that scales every frame to at most `MAX_CHANNEL_VALUE` (~20%), since the strip runs off the 5 V linear regulator; strip.rs defines `LedStrip`, the stateful per-LED color and global-brightness driver over it.
  - usb/midi.rs: USB MIDI over `embassy-usb`'s MIDI class with `usbd-midi` packet and message types. Defines `UsbMidiConfig` and `UsbMidi` (`receive`, `send_packet`, `send_message`); `UsbMidi::new` also returns a `UsbMidiDevice` whose `run()` must be polled concurrently (e.g. with `join`).
  - wifi/ (`web` only): access_point.rs drives the Pico 2 W CYW43439 on PIO1 plus the `embassy-net` stack — `start_access_point`, `AccessPointConfig` (SSID, password, channel, address), `AccessPointPeripherals`; dhcp.rs is its DHCP server.
- src/topology/scanner.rs: the hardware half - `JackScanner` runs one `JackMonitor` per jack on `PullSwitchChain` and `AdcChain`: every `step` takes one reading per ADC at once, for the jack on it due longest, applying its drives (kept applied while the ADC's other jack is read) and waiting `SettleConfig::delay` - `time_constants` x (the monitor's settle resistance + `tap_series_resistance`) x `tap_capacitance` - only when they change. `JackConfig` is one jack's entry in the board table. mod.rs re-exports the topology crate's types.
- topology/: the `expad-topology` crate, the solver's hardware-independent arithmetic. `SolveSequence` hands out the pairs of `PAIR_SEQUENCE` (arm 0 high against arms 1 and 2, then 1 against 2 only when needed) and resolves them in resolve.rs from the floating taps' voltage ratios, scaled by the SNR-weighted total the loop currents imply - `PairMeasurement::from_voltages` reduces one pair's tap voltages, `SolverConfig` expresses every tolerance in standard deviations of the ADC noise, and `ArmResistances::potentiometer` classifies a solved network (one wiper, or an end stop with two candidates within `END_STOP_RELATIVE`) for `wiper_position_with_hint`; positions count from the track's start, the sleeve (`GROUNDED_END`) when it is a track end, as `track_ends` defines. monitor.rs holds `JackMonitor`, the per-jack state machine handing out one `Reading` at a time: rails, a plug check through the tip switch (tip low, tip switch high: 1.25 V without a plug, 2.5 V with one), full solves until the network is classified (`JackMode` empty, identifying, tracking, switch, rheostat, other, open), then tracking - a potentiometer with its track ends driven and only the wiper read (end taps' pull drops checked every 25 ms), a tip-sleeve switch or rheostat (mono or stereo plug, also an open plug) with the tip driven high, the sleeve low and only the tip read (plug check every 50 ms, mono plug ring check every 100 ms). tests/ drives the solver from `StarNetwork`, a model of the real circuit, and the monitor from a simulated jack over simulated time, so both are covered on the host.
- src/web/ (`web` feature): interface.rs defines the protocol types (`Status`, `JackStatus` with the jack's mode, raw position and every contact's voltage and pull (tip switch included), `ArmPull`, `Settings`, `JackSettings` with the range `minimum`/`maximum`, `inverted`, the `drive` curve (`drive_curve`), the end-stop `WiperContact` and the jack's `Color` (`#RRGGBB`, `DEFAULT_JACK_COLORS`), applied by `JackSettings::value`) and the global `INTERFACE` state; server.rs defines `spawn_web_server` and the `picoserve` HTTP routes serving the embedded UI at `/` and a WebSocket at `/ws`.
- web/: frontend (Vue 3, Vite, Tailwind CSS 4, PrimeVue 5, VueUse, TypeScript; eslint.config.ts formats it through `@stylistic`, sorts imports and orders Tailwind classes) built into a single gzipped index.html. src/interface.ts mirrors the Rust protocol types, src/composables/useInterface.ts owns the WebSocket, src/theme.ts defines the flat Nora-based preset, the blue-gray surface ramp and the red/green/blue `INDICATOR_COLORS` (also `--p-indicator-*`), while jack colors come from the device's settings, src/App.vue lays the jacks out as a mixing desk, and src/components/ renders it (JackStrip.vue per jack with its MIDI, range, drive (DriveCurve.vue previews the curve) and wiper settings, TopologyPanel.vue with ResistorCircuit.vue drawing the selected jack's whole circuit: the solved star, the tip switch, the shared pulls and every pull switch, and JackNarrative.vue explaining it in words through `describeJack` in src/narrative.ts: what the jack does now, why, and what it waits for). mock/device.ts is the firmware stand-in (two pedals, a sustain pedal and a scripted plug-in story through every mode) and vite.config.ts configures the gzip build and dev proxy.
- docs/: design notes too long for this file - fast-tracking.md holds the ADC measurements, the pull resistor analysis, the plug-detect/tracking design and how switches and rheostats behind mono and stereo plugs are told apart.
- firmware/cyw43/: vendored CYW43439 firmware blobs (Infineon permissive binary license) embedded by the wifi HAL; .gitattributes marks these `*.bin` files binary so line endings are never converted.
- build.rs: copies linker settings into the build output, forwards `EXPAD_*` variables from the environment or `.env` to the crate, and with `web` validates `EXPAD_WIFI_PASSWORD` and runs `npm ci`/`npm run build` in web/, writing to `OUT_DIR/web`.
- .env.example: template for the untracked `.env` (WiFi password, PrimeUI license key, dev proxy target).
- Cargo.toml: manifest and embedded dependencies — the workspace (expad plus topology/), the `expad` lib target, one `[[bin]]` per file in src/bin/, and the default `web` feature gating the networking dependencies and `expad-topology/serde`.
- Embed.toml, memory.x, rp235x_riscv.x: board and linker configuration for the RP2350.

## Build & Development Commands

```bash
cargo build --bin capture   # or: cargo build (builds the lib + every bin)
cargo build --no-default-features   # skip the `web` feature (no Node.js needed)
cargo fmt
cargo clippy --all-features
cargo test -p expad-topology --target x86_64-pc-windows-msvc   # host triple: the firmware's own target cannot run tests

cd web
npm run dev:mock     # dev server + mock device: develop the UI with live dummy data, no hardware needed
npm run dev          # dev server against real hardware, proxies /ws to EXPAD_DEVICE_ADDRESS (default 192.168.4.1)
npm run type-check
npm run lint          # formats and fixes: ESLint carries the @stylistic rules, so it is the formatter
```

Formatting is a tool's job in both halves of the repository: run `cargo fmt` for Rust and
`npm run lint` in web/ instead of hand-editing code to satisfy the formatter or the linter. Only
what neither can fix automatically, such as a missing return type, is worth fixing by hand.

The default `web` feature needs Node.js (`engines` in [web/package.json](web/package.json)); build.rs reruns `npm ci` whenever `web/package-lock.json` is newer than the installed packages. Before building, copy [.env.example](.env.example) to `.env` and set the WiFi password — required, the build fails without a valid 8-63 character WPA2 password — plus, optionally, the PrimeUI license key.

Debug and flash from VS Code with [.vscode/launch.json](.vscode/launch.json) and [.vscode/tasks.json](.vscode/tasks.json); both prompt with a dropdown of `src/bin/*.rs` programs via a shared `binName` input, so building, running, and debugging target the same binary. `cargo run --bin <name>` uploads that firmware to the RP2350 and captures serial output until canceled with Ctrl-C. Dev builds use `opt-level = 1` and the probe runs at 10 MHz (.cargo/config.toml), which keeps flashing to ~13 s. `expression_controller` logs every jack's mode change (with the tip switch voltage: ~2.5 V plugged, ~1.25 V empty) and the position updates per second every 5 s.

### Adding a new application

1. Add `src/bin/<name>.rs` with `#![no_std]`, `#![no_main]`, and its own `#[embassy_executor::main]`, importing shared code via `expad::hal::...` / `expad::topology::...`.
2. Add a matching `[[bin]]` entry to Cargo.toml (`name = "<name>"`, `path = "src/bin/<name>.rs"`, `test = false`, `doctest = false`).
3. Append `"<name>"` to the `binName` input's `options` in both .vscode/tasks.json and .vscode/launch.json so it shows up in the picker.

### Adding the web interface to an application

See [src/bin/web_interface.rs](src/bin/web_interface.rs) for a complete example:

1. Add `required-features = ["web"]` to the binary's `[[bin]]` entry.
2. Bind `PIO1_IRQ_0 => pio::InterruptHandler<PIO1>` and `DMA_IRQ_0 => dma::InterruptHandler<DMA_CHx>` for the chosen DMA channel. All DMA channels share `DMA_IRQ_0`, so list every channel's handler there (e.g. `DMA_CH0` for WiFi, `DMA_CH1` for the LED strip, as in expression_controller.rs).
3. `let stack = start_access_point(spawner, AccessPointPeripherals { .. }, Irqs, AccessPointConfig::default()).await;`, then `spawn_web_server(spawner, stack);`.
4. Publish measurements with `INTERFACE.status.sender().send(status)`, take one `INTERFACE.settings.receiver()` at startup, and apply every `settings.changed().await`. Sending to `INTERFACE.settings` (e.g. values loaded from flash) updates every open interface.

To extend the protocol, change [src/web/interface.rs](src/web/interface.rs) and [web/src/interface.ts](web/src/interface.ts) together.

## Code Style & Conventions

- Rust 2024 edition conventions; small, explicit modules, `cargo fmt` formatting (run it, never format by hand), code readable for embedded development.
- Non-abbreviated, self-descriptive names; avoid single-letter ones outside very local contexts (e.g. loop indices).
- Prefer self-documenting code over comments, splitting larger expressions into named variables to clarify intent. Prefer `Result`-based error handling and typed config structs over ad-hoc values, and keep hardware-facing logic close to its module, such as ADC or buffer handling.
- In web/, write `<script setup lang="ts">` single-file components, import PrimeVue components individually (`primevue/<name>`) and icons from `@primeicons/vue/<name>`, style with Tailwind utilities (including `tailwindcss-primeui` color tokens), and prefer VueUse composables over hand-written browser glue. Color anything jack-specific through `--jack-color` and the `.jack-theme` design-token block in src/main.css instead of styling controls one by one. Leave layout, quoting, import order and Tailwind class order to `npm run lint`, which fixes them in place, and keep both it and `npm run type-check` clean.

## Architecture Notes

Shared drivers and logic live in the `expad` library crate, which every file under src/bin/ depends on:

```text
expad (lib): board -> hal::{adc, buf, led, usb}, topology::scanner -> expad-topology (topology/)

capture, detect_pin_mapping -> board -> PullSwitchChain -> ShiftRegisterChain, AdcChain
rainbow                     -> board -> LedStrip -> Ws2812Chain
potentiometer               -> board -> PullSwitchChain, AdcChain, LedStrip
midi_loopback               -> UsbMidi (+ UsbMidiDevice run future)
web_interface               -> start_access_point (cyw43 + embassy-net + DHCP tasks)
                            -> spawn_web_server (picoserve tasks)
                               <-> INTERFACE (status/settings watches) <-> browser (web/, WebSocket /ws)
expression_controller       -> board, JackScanner -> JackMonitor x4, AdcChain, PullSwitchChain
                            -> LedStrip, UsbMidi, start_access_point, spawn_web_server
                               <-> INTERFACE (status/settings watches) <-> browser
```

The web interface separates WiFi transport from server. `start_access_point` runs the CYW43439 as a WPA2-protected access point at 192.168.4.1/24 and spawns the WiFi, network, and DHCP tasks; `spawn_web_server` takes any `embassy_net::Stack` and spawns `MAX_SESSIONS` picoserve tasks on port 80. Each WebSocket session sends `{"settings": ...}` on connect, then `{"status": ...}` or `{"settings": ...}` whenever the matching `embassy_sync::watch::Watch` in `INTERFACE` changes; every text message from the browser is a full `Settings` JSON object, broadcast to the firmware and all sessions. Non-finite floats (e.g. in `ArmResistances::DISCONNECTED`) serialize as `null`.

The board's 5 V rail is a 200 mA linear regulator shared by the LEDs and the Pico, so LED brightness is capped in `Ws2812Chain` and must never be raised past ~20%; `Settings::led_brightness` and `LedStrip` brightness scale within that cap.

## Testing Strategy

- topology/tests/ covers the solver's arithmetic against a simulated network and the jack monitor against a simulated jack (plugging, moving, end stops, switches and rheostats behind mono and stereo plugs, mismatched pulls, noise); run it with `cargo test -p expad-topology --target <host triple>` before merging, and extend it whenever the solving behavior changes. The `expad` crate itself only builds for the firmware target, so anything that needs a host test belongs in topology/.
- Add unit tests for register encoding when its behavior changes.
- For web interface changes, check the browser under `npm run dev:mock` (.claude/launch.json runs the mock device and Vite on port 5190 separately, since 5173 is reserved on the dev machine), then against a flashed `web_interface` device with `npm run dev`. Keep [web/mock/device.ts](web/mock/device.ts), which speaks the firmware's protocol, in sync with protocol changes.
- Validate hardware behavior on-device with the existing debug/RTT setup in [.vscode/launch.json](.vscode/launch.json); after any change to the switch driver or `board::JACKS`, run `cargo run --bin detect_pin_mapping` with the jacks unplugged.

## Security & Compliance

- Keep secrets, credentials, and private board-specific values out of source control. The WiFi password and PrimeUI license key live in the untracked `.env`; both are compiled into the firmware image, and the license key is visible in the served page.
- PrimeVue 5 and `@primeicons/vue` use the proprietary PrimeUI license (free Community License with a yearly key), not MIT.
- Keep Cargo.lock, web/package-lock.json, and the existing LICENSE for redistributed code in place; update dependencies intentionally.
- Review any pin, SPI, or register change carefully, since it affects hardware behavior.

## Agent Guardrails

- Do not change linker scripts, board targets, or pin assignments without verifying the hardware implications; derive wiring from the KiCad project in hardware/ (e.g. `kicad-cli sch export netlist`).
- Avoid broad rewrites of the ADC or buffer abstractions unless justified and tested.
- Prefer small, reviewable edits, verified with `cargo build` first.
- Do not modify generated artifacts under target/ directly.

## Maintaining This File

- Update it in the same change as any larger one: new, renamed, or removed modules, binaries, features, dependencies, build steps, commands, or protocols. Small fixes inside an existing file need no update.
- Touch only the affected lines, usually in Repository Structure, Build & Development Commands, and Architecture Notes.
- Keep it condensed: one line per item, no changelog, history, rationale, or anything the code already states. Rewrite or delete stale lines instead of appending to them.
