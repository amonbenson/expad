# AGENTS.md

expad is Rust firmware for an RP2350 (Raspberry Pi Pico 2 W) on the expression controller PCB
(KiCad, hardware/). It switches every jack contact to shared pull-up/pull-down resistors, reads
the contacts through two AD7718 ADCs, identifies what is plugged into each jack with a small
topology solver, then follows it with single readings and sends the position as a MIDI control
change over USB. A WiFi access point serves a live web interface (Vue app in web/) for status and
settings.

Start at the index, then read only the section it points to.

## 0. Index

| Looking for | Go to |
|---|---|
| Build, flash, test, lint, format commands | [1. Commands](#1-commands) |
| How the crates and layers depend on each other | [2. Architecture](#2-architecture) |
| Pins, chips, jack order, board constants (`JACKS`, `REFERENCE_VOLTAGE`, `ADC_UPDATE_RATE`, ...) | [3.1 board](#31-board-srcboardrs) |
| AD7718 driver, registers, SPI timing | [3.2 hal::adc](#32-hal-srchal) |
| Pull switches (74HC595 + TMUX1511), `TriState` | [3.2 hal::buf](#32-hal-srchal) |
| LED strip, brightness cap | [3.2 hal::led](#32-hal-srchal), [4. Concepts](#4-concepts) |
| USB MIDI, WiFi access point, DHCP/DNS captive portal | [3.2 hal::usb / hal::wifi](#32-hal-srchal) |
| Solver arithmetic, `Network` classification, positions | [3.3 expad-topology](#33-expad-topology-topology) |
| `JackMonitor` states, `JackMode`, plug detection, following | [3.3 monitor](#33-expad-topology-topology), docs/fast-tracking.md |
| Scheduling readings on the hardware, settling | [3.4 scanner](#34-scanner-srctopologyscannerrs) |
| LED colors per jack | [3.5 indicator](#35-indicator-srcindicatorrs) |
| Web protocol (`Status`, `Settings`), server, WebSocket | [3.6 web](#36-web-srcweb-web-feature) |
| Flashable programs | [3.7 binaries](#37-binaries-srcbin) |
| Frontend (Vue) | [3.8 web/ frontend](#38-web-frontend-web) |
| Units, index conventions, the three drive enums | [4. Concepts](#4-concepts) |
| Adding a binary, the web interface, a protocol field, a board change | [5. Recipes](#5-recipes) |
| Naming, formatting, style | [6. Conventions](#6-conventions) |
| What to test and how | [7. Testing](#7-testing) |
| Secrets, licenses, hardware guardrails | [8. Guardrails](#8-guardrails) |

## 1. Commands

```bash
cargo build                                  # lib + every bin (the default `web` feature runs npm in web/)
cargo build --no-default-features            # skip `web`: no Node.js needed
cargo run --bin expression_controller        # flash over probe-rs and stream the defmt log (Ctrl-C to stop)
cargo run --bin expression_controller --features no-wifi   # radio held powered down, for electrical tests
cargo run --bin detect_pin_mapping           # board self-test, jacks unplugged
cargo test -p expad-topology --target x86_64-pc-windows-msvc   # host tests; use your host triple
cargo fmt
cargo clippy --all-features                  # also run plain `cargo clippy`: --all-features enables no-wifi

cd web
npm run dev:mock     # UI against the mock device, no hardware
npm run dev          # UI against a device at EXPAD_DEVICE_ADDRESS (default 192.168.4.1), proxying /ws
npm run type-check
npm run lint         # ESLint with @stylistic is the formatter: it fixes in place

cd measurements
uv run create_plots.py        # bench plots for the slides (outside the firmware)
```

- Setup: copy .env.example to `.env` and set `EXPAD_WIFI_PASSWORD` (8-63 printable ASCII; the
  `web` build fails without it), optionally `VITE_PRIMEUI_LICENSE`. Node.js per `engines` in
  web/package.json.
- Target `thumbv8m.main-none-eabihf` and the probe-rs runner (10 MHz SWD) are set in
  .cargo/config.toml; dev builds use `opt-level = 1`, so flashing takes ~13 s.
- VS Code: .vscode/tasks.json and launch.json share a `binName` dropdown of the binaries, so
  build, run and debug target the same one; settings.json formats on save.
- Formatting is a tool's job: run `cargo fmt` and `npm run lint`, never hand-format.

## 2. Architecture

Two crates in one workspace:

- `expad` (src/, `#![no_std]`, firmware target only): board wiring, drivers, the hardware half of
  the solver, the web server and one binary per program.
- `expad-topology` (topology/, `#![no_std]`, builds for the host too): the solver's arithmetic
  and the per-jack state machine, with no hardware dependency, so it is unit tested on the host.
  It holds no board constants; the firmware passes them in (`MonitorConfig::new`,
  `SolverConfig::from_voltage_noise`).

```text
expad (lib): board -> hal::{adc, buf, led, usb, wifi}, indicator, topology::scanner -> expad-topology, web

capture, detect_pin_mapping, adc_characterization -> board -> PullSwitchChain, AdcChain
rainbow                     -> board -> LedStrip -> Ws2812Chain
potentiometer               -> board -> PullSwitchChain, AdcChain, LedStrip
midi_loopback               -> UsbMidi (+ UsbMidiDevice run future)
web_interface               -> start_access_point, spawn_web_server <-> INTERFACE <-> browser
expression_controller       -> board, JackScanner -> JackMonitor x4, PullSwitchChain, AdcChain
                            -> JackIndicators -> LedStrip; UsbMidi
                            -> start_access_point, spawn_web_server <-> INTERFACE <-> browser (web/, WebSocket /ws)
```

Layering rules: only `board` knows the PCB; `hal` drivers know chips, not jacks; `expad-topology`
knows neither; binaries wire them together. `lib.rs` allows dead code in `hal` and `topology`,
which expose more register and driver surface than the binaries use.

## 3. Modules

### 3.1 board (src/board.rs)

The PCB's wiring and the only place that knows it.

- Counts: `JACK_COUNT`, `SWITCH_CHIPS`, `ADC_CHIPS`, `LED_COUNT`; `ADC_CHANNEL_COUNT` (ten-channel
  mode), `ADC_GROUND_CHANNEL`/`ADC_REFERENCE_CHANNEL` (self-test inputs).
- Analog values: `REFERENCE_VOLTAGE` (2.5 V, also the pull-up rail), `PULL_RESISTANCE`,
  `INPUT_FILTER_RESISTANCE`/`INPUT_FILTER_CAPACITANCE`, `ADC_UPDATE_RATE` (819 Hz) and
  `ADC_VOLTAGE_NOISE` (measured at that rate; every solver tolerance derives from it).
- `Contact` (tip, ring, sleeve, tip switch; discriminant = switch tap) and `JACKS`, one
  `JackWiring` per jack (switch chip, ADC chip, ADC channel per contact, LED), in the case's order
  J5 to J2, since the PCB is mounted upside down. `JackWiring::config` gives the scanner's
  `JackConfig`.
- Constructors on the board's pins: `pull_switches`, `adcs`, `leds`.

### 3.2 hal (src/hal/)

- adc/ (AD7718 chain on SPI0): `AdcChain` with `AdcChainConfig` builders (`with_channel_count`,
  `with_range`, `with_coding`, `with_update_rate`, `with_spi_frequency`,
  `with_chip_select_guard`, `with_reference_voltage`; 4 MHz, 10 µs by default). `init`
  resets, checks IDs, writes registers and calibrates.
  - Readings: `start_conversion`/`finish_conversion`, `measure_channel`, `measure_parallel` (one
    channel per chip, all chips converting at once). The control register is only rewritten
    when the channel changes.
  - Continuous: `measure_continuous` (continuous conversion mode on one channel) vs.
    `start_continuous_capture`/`wait_for_next_result` (cycles every channel with single
    conversions).
  - `set_update_rate` switches the filter and keeps the old calibration.
  - registers/: one type per register over `RegisterValue<ADDRESS, WIDTH>` with bit/field helpers.
- buf/ (pull switches on SPI1): `PullSwitchChain` (one 74HC595 per jack driving two TMUX1511s;
  `TriState` per tap, `set_output` then `update`; chip 0 nearest the MCU) over
  `ShiftRegisterChain` (outputs stay disabled until the first write, then latch atomically).
- led/: `Ws2812Chain` (PIO WS2812B driver, scales every frame to at most `MAX_CHANNEL_VALUE`,
  ~4%) and `LedStrip` (per-LED color and global brightness on top).
- usb/: `UsbMidi` (embassy-usb MIDI class, usbd-midi packet types): `wait_connection`, `receive`,
  `send_packet`, `send_message`; transfers end with `EndpointError::Disabled` once the host
  disconnects. `UsbMidi::new` also returns the `UsbMidiDevice` whose `run()` must be polled
  concurrently. driver.rs wraps embassy-rp's driver: clears a stale suspend before a bus reset
  (embassy-rp 0.10 bug) and paces an event flood.
- wifi/ (`web` feature): `start_access_point` (CYW43439 on PIO1, WPA2, `AccessPointConfig`,
  `AccessPointPeripherals`) spawns the WiFi, network, DHCP (dhcp.rs, names the device gateway and
  DNS server) and DNS (dns.rs, every name resolves to the device) tasks.

### 3.3 expad-topology (topology/)

All resistances in kΩ, currents in mA, voltages in V, times in ms.

- sequence.rs: `SolveSequence` hands out the pairs of `PAIR_SEQUENCE` (arm 0 high against arms 1
  and 2, then 1 against 2 only when needed) and resolves them; `SolveError`.
- measurement.rs: `PairMeasurement::from_voltages` reduces one pair's three tap voltages
  (`PairVoltages`, `ArmDrive`).
- resolve.rs: arm resistances from the floating taps' voltage ratios, scaled by the SNR-weighted
  total the loop currents imply.
- config.rs: `SolverConfig`, every tolerance in standard deviations of the ADC noise.
- resistances.rs: `ArmResistances` (relative share per arm + total; `0` shorted, `inf` isolated,
  `NaN` unresolved), `ArmResistances::network` classifies a `Network` (disconnected,
  potentiometer, end stop, near end, tip-sleeve element, other); `position_with_wiper` counts
  from the track's start, the sleeve (`GROUNDED_END`) when it is a track end (`track_ends`).
- monitor.rs: `JackMonitor`, the per-jack state machine handing out one `Reading` at a time and
  reporting a `JackReport` (`JackMode`, position, resistances, voltages, drives).
  - States: rails -> empty (plug check: tip low, tip switch high; ~1.25 V without a plug,
    ~2.5 V with one) -> solve -> confirm plug (solve found no current) -> follow.
  - Follow: a potentiometer (track ends driven, wiper read, end taps checked every 25 ms) or a
    tip-sleeve `Element` (open, switch, rheostat: tip high, sleeve low, tip read; plug check
    every 50 ms, ring check every 100 ms, open plugs solved every 1 s).
  - `Pedal` remembers what was learned until the plug comes out or turns out open; the wiper
    of the last potentiometer is remembered for the next ambiguous end stop.
  - Design and measurements: docs/fast-tracking.md; state diagram: docs/diagrams/.
- tests/: `star_network` models the real circuit (and holds the PCB's noise and pull values the
  tests use); solve.rs drives the solver, monitor.rs a simulated jack over simulated time.

### 3.4 scanner (src/topology/scanner.rs)

`JackScanner` runs one `JackMonitor` per jack on `PullSwitchChain` and `AdcChain`: every `step`
takes one reading per ADC at once, for its jack due longest, applies that jack's drives (kept
while the ADC's other jack is read) and waits `SettleConfig::delay` (`time_constants` x
(settle resistance + filter resistance) x filter capacitance) only when they change.
`JackConfig` is one jack's entry from the board table. src/topology/mod.rs re-exports the
topology crate's types.

### 3.5 indicator (src/indicator.rs)

`JackIndicators` owns the `LedStrip` and colors each jack's LED from its `JackMode` and sent value:
off when empty, dim white for an unknown plug, a recognised pedal's color for 1 s once per
plug-in (purple expression, lime footswitch, yellow rheostat), then the value from dark red
through red, orange and yellow to white.

### 3.6 web (src/web/, `web` feature)

- interface.rs: protocol types. `Status` (per jack `JackStatus`: mode, raw position, value,
  resistances, every contact's voltage and `ArmPull`), `Settings` (`led_brightness`, per jack
  `JackSettings`: MIDI channel and controller, `minimum`/`maximum`, `inverted`, `drive`
  (`drive_curve`), `WiperContact`, `Color` as `#RRGGBB`). `JackSettings::value` maps a position
  to the sent value. `INTERFACE` holds the `status` and `settings` watches.
- server.rs: `spawn_web_server` spawns `MAX_SESSIONS` picoserve tasks on port 80: the embedded,
  gzipped UI at `/`, the WebSocket at `/ws`, and a redirect of every other path to the UI
  (`CaptivePortal`), so devices open it as the network's sign-in page.
- Protocol: a session sends `{"settings": ...}` on connect, then `{"status": ...}` or
  `{"settings": ...}` on every watch change; every text message from the browser is a full
  `Settings` object, broadcast to the firmware and all sessions. Non-finite floats serialize as
  `null`.

### 3.7 binaries (src/bin/)

One flashable program per file, each with its own `#[embassy_executor::main]`:

- `expression_controller`: the full firmware. Runs a `JackScanner` over every jack, maps each
  position through the jack's settings, sends MIDI control changes (a newer one replaces one
  still waiting), drives `JackIndicators`, publishes the status (~30 Hz) and logs mode changes and
  position updates per second every 5 s. `no-wifi` holds the radio powered down.
- `detect_pin_mapping`: board self-test: ADC ground/reference inputs, then every contact pulled up
  and down against `board::JACKS`.
- `capture`: holds every contact in the `DRIVE` pattern and captures every ADC input with bipolar
  coding; the tool for measuring `ADC_VOLTAGE_NOISE`.
- `adc_characterization`: SPI settings and every filter rate compared on a pot pedal in jack 1
  (results in docs/fast-tracking.md).
- `potentiometer`: one pedal's wiper on jack 1, mirrored on the LEDs.
- `rainbow`: LED strip test. `midi_loopback`: echoes USB MIDI. `web_interface`: the web interface
  with dummy status data.

### 3.8 web/ frontend (web/)

Vue 3, Vite, Tailwind CSS 4, PrimeVue 5, VueUse, TypeScript, built by build.rs into one gzipped
index.html (`OUT_DIR/web`).

- src/interface.ts mirrors src/web/interface.rs; src/composables/useInterface.ts owns the
  WebSocket.
- src/App.vue lays the jacks out as a mixing desk; components/: JackStrip.vue (per-jack settings,
  DriveCurve.vue previews the curve), TopologyPanel.vue with ResistorCircuit.vue (the selected
  jack's circuit) and JackNarrative.vue (`describeJack` in src/narrative.ts explains it).
- src/theme.ts: Nora-based preset, surface ramp, `INDICATOR_COLORS`; jack colors come from the
  device's settings.
- mock/device.ts is the firmware stand-in (speaks the same protocol); vite.config.ts configures
  the gzip build and dev proxy.

### 3.9 Everything else

- build.rs: copies memory.x, forwards `EXPAD_*` variables from the environment or `.env`, and
  with `web` validates the WiFi password and runs `npm ci`/`npm run build` in web/.
- Cargo.toml: workspace, `[[bin]]` per program, features `web` (default: networking and
  `expad-topology/serde`) and `no-wifi`.
- memory.x, rp235x_riscv.x, Embed.toml: linker and probe configuration.
- firmware/cyw43/: vendored CYW43439 blobs (Infineon permissive binary license).
- docs/: fast-tracking.md (ADC measurements, pull resistor analysis, monitor design), diagrams/
  (TikZ state diagram of `JackMonitor` with PDF/SVG/PNG exports; build steps in its README).
- presentations/, measurements/: milestone slides and bench measurement scripts, outside the
  firmware.

## 4. Concepts

- Units: kΩ, mA, V, nF, ms throughout the solver (kΩ x mA = V, kΩ x nF = µs).
- Arm indices: `TIP` 0, `RING` 1, `SLEEVE` 2, `TIP_SWITCH` 3 (contact index only); use the
  constants, never literals. `ARM_COUNT` 3, `CONTACT_COUNT` 4.
- Jack index 0-3 is the case's order (J5 to J2); switch chip, ADC chip and LED come from
  `board::JACKS`, never from the jack index.
- Three drive enums, one per layer: `Drive` (topology: High/Low/Floating), `TriState` (hal::buf:
  High/Low/HiZ, mapped in the scanner), `ArmPull` (web protocol: up/down/floating).
- Position vs. value: the monitor reports a raw position in `0.0..=1.0`; `JackSettings::value`
  applies range, inversion and drive to give the sent value.
- Power budget: the 5 V rail is a 200 mA linear regulator shared by the LEDs and the Pico, so LED
  brightness is capped in `Ws2812Chain` and must never be raised past ~4%; `led_brightness`
  scales within that cap.

## 5. Recipes

### Add a binary

1. `src/bin/<name>.rs` with `#![no_std]`, `#![no_main]` and its own `#[embassy_executor::main]`;
   build drivers through `expad::board`, name the peripherals `peripherals`, use `defmt::unwrap!`.
2. A `[[bin]]` entry in Cargo.toml (`test = false`, `doctest = false`).
3. Append `"<name>"` to the `binName` options in .vscode/tasks.json and .vscode/launch.json.

### Add the web interface to a binary (see src/bin/web_interface.rs)

1. `required-features = ["web"]` on its `[[bin]]` entry.
2. Bind `PIO1_IRQ_0 => pio::InterruptHandler<PIO1>` and `DMA_IRQ_0` with the handler of every DMA
   channel in use (they share the interrupt), e.g. `DMA_CH0` for WiFi, `DMA_CH1` for the LEDs.
3. `let stack = start_access_point(spawner, AccessPointPeripherals { .. }, Irqs, AccessPointConfig::default()).await;`
   then `spawn_web_server(spawner, stack);`.
4. Publish with `INTERFACE.status.sender().send(status)`, take one `INTERFACE.settings.receiver()`
   at startup and apply every change; sending to `INTERFACE.settings` updates every open page.

### Extend the protocol

Change src/web/interface.rs, web/src/interface.ts and web/mock/device.ts together.

### Change solving or monitoring

Edit topology/, extend topology/tests/ and run the host tests; update docs/fast-tracking.md and
the state diagram when states or thresholds change.

### Change the board

Derive wiring from hardware/ (e.g. `kicad-cli sch export netlist`), edit only src/board.rs, then
run `cargo run --bin detect_pin_mapping` with the jacks unplugged.

## 6. Conventions

- Rust 2024; small, self-contained modules; hardware-facing logic stays in its driver module.
- Non-abbreviated, self-descriptive names; single letters only for very local indices.
- Self-documenting code over comments: split larger expressions into named variables.
- `Result`-based errors, typed config structs with `with_*` builders or `new` over ad-hoc values;
  magic numbers become named constants in the module that owns the concept (board values in
  board.rs).
- In binaries, `defmt::unwrap!` rather than `.unwrap()`.
- web/: `<script setup lang="ts">` single-file components, PrimeVue components imported
  individually (`primevue/<name>`), icons from `@primeicons/vue/<name>`, Tailwind utilities
  (with `tailwindcss-primeui` tokens), VueUse over hand-written browser glue, jack-specific color
  through `--jack-color` and the `.jack-theme` block in src/main.css. Keep `npm run lint` and
  `npm run type-check` clean.

## 7. Testing

- Host: `cargo test -p expad-topology --target <host triple>` before merging; extend topology/tests/
  whenever solving or monitoring changes. The `expad` crate only builds for the firmware target, so
  anything needing a host test belongs in topology/.
- Register encoding: add unit tests when it changes.
- Web: check the browser under `npm run dev:mock` (.claude/launch.json runs the mock device and
  Vite on port 5190, since 5173 is reserved on the dev machine), then against a flashed
  `web_interface` with `npm run dev`.
- Hardware: RTT/defmt via .vscode/launch.json; after any change to the switch driver or
  `board::JACKS`, run `detect_pin_mapping` with the jacks unplugged.

## 8. Guardrails

- Secrets stay in the untracked `.env` (WiFi password, PrimeUI license key); both are compiled into
  the image and the license key is visible in the served page.
- PrimeVue 5 and `@primeicons/vue` use the proprietary PrimeUI license (free Community License,
  yearly key), not MIT.
- Keep Cargo.lock, web/package-lock.json and LICENSE files; update dependencies intentionally.
- Never change linker scripts, board targets, pins, SPI settings or registers without checking the
  hardware implications; review such changes carefully.
- Avoid broad rewrites of the ADC or buffer abstractions unless justified and tested.
- Prefer small, reviewable edits verified with `cargo build`; never edit target/.

## 9. Maintaining this file

- Update it in the same change as any larger one: new, renamed or removed modules, binaries,
  features, dependencies, commands or protocols. Small fixes inside a file need no update.
- Touch only the affected lines; add new terms to the index.
- Keep it condensed: one line per item, no changelog, history or rationale the code already states.
  Rewrite or delete stale lines instead of appending.
