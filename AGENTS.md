# Project Overview

expad is a Rust firmware project for an RP2350-based embedded target. It initializes a shift-register buffer chain and an ADC chain, then measures analog voltages and infers resistance distribution across three arms using a small topology solver.

## Repository Structure

- .vscode/: VS Code tasks and launch configuration for building, running, and debugging the firmware, with a picker for which `src/bin/*.rs` program to target.
- src/: firmware code, split into a shared library and one binary per application.
  - src/lib.rs: `#![no_std]` library crate (`expad`) that re-exports `hal` and `topology` for every binary to share.
  - src/bin/: one file per flashable application, each with its own `#[embassy_executor::main]`. `capture.rs` holds the original main-loop firmware (shift-register/buffer init, ADC init, direct measurements, continuous capture); `detect_pin_mapping.rs` maps buffer outputs to ADC channels; `rainbow.rs` drives a WS2812B strip through a rainbow pattern.
  - src/hal/adc/: ADC chain driver, register abstractions, and measurement flow for the AD7718 devices.
  - src/hal/buf/: tri-state buffer control (quad_buffer.rs) and the SPI shift-register wrapper (shift_register.rs) for output channels.
  - src/hal/led/: PIO-backed WS2812B ("NeoPixel") LED strip driver (ws2812.rs), configurable by LED count, plus a stateful per-LED color and brightness driver on top of it (strip.rs).
  - src/hal/mod.rs: hardware abstraction layer module that re-exports the adc, buf, and led submodules.
  - src/topology/: resistance-solving logic that interprets ADC measurements.
- build.rs: copies linker settings into the build output so the firmware links correctly.
- Cargo.toml: crate manifest and embedded dependencies. Declares the `expad` lib target plus one `[[bin]]` entry per file in src/bin/.
- Embed.toml, memory.x, rp235x_riscv.x: board and linker configuration for the RP2350 target.

## Build & Development Commands

```bash
cargo build --bin capture   # or: cargo build (builds the lib + every bin)
cargo fmt
cargo clippy --all-features
cargo test
```

Debug and flash from VS Code using the existing configuration in [.vscode/launch.json](.vscode/launch.json) and [.vscode/tasks.json](.vscode/tasks.json) — both prompt with a dropdown of the available `src/bin/*.rs` programs (currently just `capture`) via a shared `binName` input, so building, running, and debugging all target the same chosen binary.
Use `cargo run --bin <name>` to upload the chosen firmware to the RP2350 target and capture serial output. Needs to be canceled with Ctrl-C to stop the capture.

### Adding a new application

1. Add `src/bin/<name>.rs` with `#![no_std]`, `#![no_main]`, and its own `#[embassy_executor::main]`, importing shared code via `expad::hal::...` / `expad::topology::...`.
2. Add a matching `[[bin]]` entry to [Cargo.toml](Cargo.toml) (`name = "<name>"`, `path = "src/bin/<name>.rs"`, `test = false`, `doctest = false`).
3. Append `"<name>"` to the `binName` input's `options` in both [.vscode/tasks.json](.vscode/tasks.json) and [.vscode/launch.json](.vscode/launch.json) so it shows up in the picker.

## Code Style & Conventions

- Use Rust 2024 edition conventions and keep modules small and explicit.
- Use non-abbreviated, self-descriptive names for functions, types, and variables. Avoid single-letter names except in very local contexts (e.g., loop indices).
- Use self-documenting code over comments whereever possible. Separate larger expressions into named variables to clarify intent.
- Prefer `Result`-based error handling and typed config structs over ad-hoc values.
- Keep hardware-facing logic close to the relevant module, such as ADC or buffer handling.
- Use `cargo fmt` for formatting and keep code readable for embedded development.

## Architecture Notes

```text
expad (lib)
  hal::{adc, buf, led}
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
topology::ResistanceSolver (not yet invoked from any binary)
  -> AdcChain, QuadBufferChain
```

Shared drivers and logic live in the `expad` library crate ([src/lib.rs](src/lib.rs)), which every file under [src/bin/](src/bin/) depends on. The `capture` binary ([src/bin/capture.rs](src/bin/capture.rs)) is the firmware entry point today: it configures the SPI-based shift-register chain, clears the quad-buffer outputs, and initializes the ADC chain to take direct channel measurements and then loop over continuous capture. The `rainbow` binary ([src/bin/rainbow.rs](src/bin/rainbow.rs)) drives a WS2812B strip via `LedStrip` (which tracks per-LED color state and a global brightness on top of the low-level `Ws2812Chain` PIO driver) and cycles a rainbow pattern across it. The topology solver in [src/topology/solver.rs](src/topology/solver.rs) implements the tri-state toggling and resistance-inference logic but is not yet called from any binary.

## Testing Strategy

- No dedicated test suite is present yet.
- Add unit tests for the topology solver and register encoding logic when behavior changes.
- Run `cargo test` locally before merging changes.
- Validate hardware behavior on-device with the existing debug/RTT setup in [.vscode/launch.json](.vscode/launch.json).

## Security & Compliance

- Keep secrets, credentials, and private board-specific values out of source control.
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
- [src/topology/solver.rs](src/topology/solver.rs) is the main place to extend resistance-solving logic.
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
