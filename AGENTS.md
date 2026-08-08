# Project Overview

expad is a Rust firmware project for an RP2350-based embedded target. It initializes a shift-register buffer chain and an ADC chain, then measures analog voltages and infers resistance distribution across three arms using a small topology solver.

## Repository Structure

- .vscode/: VS Code tasks and launch configuration for building and debugging the firmware.
- src/: application code for the embedded firmware.
  - src/hal/adc/: ADC chain driver, register abstractions, and measurement flow for the AD7718 devices.
  - src/hal/buf/: tri-state buffer control (quad_buffer.rs) and the SPI shift-register wrapper (shift_register.rs) for output channels.
  - src/hal/mod.rs: hardware abstraction layer module that re-exports the adc and buf submodules.
  - src/topology/: resistance-solving logic that interprets ADC measurements.
- build.rs: copies linker settings into the build output so the firmware links correctly.
- Cargo.toml: crate manifest and embedded dependencies.
- Embed.toml, memory.x, rp235x_riscv.x: board and linker configuration for the RP2350 target.

## Build & Development Commands

```bash
cargo build
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

Debug and flash from VS Code using the existing configuration in [.vscode/launch.json](.vscode/launch.json).
Use `cargo run` to upload the firmware to the RP2350 target and capture serial output. Needs to be canceled with Ctrl-C to stop the capture.

## Code Style & Conventions

- Use Rust 2024 edition conventions and keep modules small and explicit.
- Prefer `Result`-based error handling and typed config structs over ad-hoc values.
- Keep hardware-facing logic close to the relevant module, such as ADC or buffer handling.
- Use `cargo fmt` for formatting and keep code readable for embedded development.
- Follow a simple commit style such as `type(scope): summary` unless a stricter template is added later.

## Architecture Notes

```text
main
  -> ShiftRegisterChain
  -> QuadBufferChain
  -> AdcChain
topology::ResistanceSolver (not yet invoked from main)
  -> AdcChain, QuadBufferChain
```

The firmware starts in [src/main.rs](src/main.rs), where it configures the SPI-based shift-register chain, clears the quad-buffer outputs, and initializes the ADC chain to take direct channel measurements and then loop over continuous capture. The topology solver in [src/topology/solver.rs](src/topology/solver.rs) implements the tri-state toggling and resistance-inference logic but is not yet called from main.

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
- [src/topology/solver.rs](src/topology/solver.rs) is the main place to extend resistance-solving logic.
- [Embed.toml](Embed.toml) and [.vscode/launch.json](.vscode/launch.json) are the main extension points for flashing and debugging.

## Further Reading

- [Cargo.toml](Cargo.toml)
- [Embed.toml](Embed.toml)
- [src/main.rs](src/main.rs)
- [src/hal/adc/mod.rs](src/hal/adc/mod.rs)
- [src/hal/buf/mod.rs](src/hal/buf/mod.rs)
- [src/topology/solver.rs](src/topology/solver.rs)
