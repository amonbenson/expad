# expad - Expression Adapter

A four-jack USB MIDI adapter for expression and sustain pedals, built on a Raspberry Pi Pico 2 W.
It works out what is plugged into each jack - whichever contact a pedal's wiper is on, and whether
it is a potentiometer, a switch or a two-wire rheostat - and sends every pedal's position as a MIDI
control change, ~190 times per second. A WiFi access point serves a web interface for watching the
jacks and changing the settings live.

## Hardware

The PCB is the KiCad project in [hardware/](hardware/). Each jack's tip, ring and sleeve (plus the
tip's switch contact, for plug detection) can be switched to a shared 1 kΩ pull-up to the 2.5 V
reference or a 1 kΩ pull-down, and is read back by one of two AD7718 24-bit ADCs. The four
WS2812B LEDs below the jacks light up in each jack's color while a pedal is plugged in, and turn
green while it moves or its switch is pressed; they run off a linear regulator, so the firmware caps
them at ~20% brightness.

## Supported pedals

| Pedal | Cable | Shown as | Value |
|---|---|---|---|
| Expression pedal (potentiometer), wiper on tip or ring | stereo (TRS) | Tracking | wiper position, counting up away from the sleeve |
| Sustain pedal / footswitch | mono (TS) or stereo | Switch | 100% closed, 0% open |
| Two-wire (rheostat) expression pedal | mono or stereo | Rheostat | resistance against the nearest standard value (10k, 25k, 50k ...) |
| Anything else (dual footswitches, other wirings) | | Other pedal | none |

Pedals are recognised within ~0.1 s of being plugged in, and unplugging is noticed just as fast.

- A normally-closed sustain pedal, or an expression pedal wired the other way round, needs
  **Invert**.
- A rheostat pedal on a mono cable first reads as a potentiometer at rest, until its resistance has
  changed by a fifth.
- A potentiometer pedal needs a stereo cable: with a mono one, most pedals short their ring to the
  sleeve, which leaves a resistance that rises and falls again over the travel.
- A mono cable with nothing at the other end reads as a released switch.

## Using it

1. Plug the Pico into a computer over USB. It shows up as a USB MIDI device; every jack sends
   CC 11 (Expression) on MIDI channel 1 by default.
2. Join the WiFi network **Expression Adapter** (password: the one it was built with, see below)
   and open http://192.168.4.1.
3. Per jack, set the MIDI channel and controller, **Invert**, and the **Range**: move the pedal to
   one end and press **Min**, to the other and press **Max**, so its travel covers the full MIDI
   range. **Drive** bends the response against a pedal's nonlinear track: centered is linear,
   turning it right makes the value rise early in the travel, left late (double-click resets it).
   **Wiper** only matters for pedals whose wiper is on the ring or sleeve and that rest on an
   end stop when plugged in; **Auto** handles the common wirings. The LED slider sets the
   brightness.

The topology panel on the right draws the selected jack's measured network.

## Building and flashing

Needs Rust (with the `thumbv8m.main-none-eabihf` target), Node.js for the web interface, and a
debug probe for `probe-rs`.

```bash
cp .env.example .env   # then set EXPAD_WIFI_PASSWORD (8-63 characters)
cargo run --bin expression_controller   # build, flash and show the log
```

Other programs in [src/bin/](src/bin/) test parts of the board, e.g. `detect_pin_mapping` checks
every contact's switches and ADC input against the wiring table. Run the host tests with
`cargo test -p expad-topology --target <host triple>`, and develop the web interface without
hardware with `npm run dev:mock` in web/.

[AGENTS.md](AGENTS.md) describes the repository in detail, and
[docs/fast-tracking.md](docs/fast-tracking.md) the measurements and design behind the pedal
detection.

## General TODOs
- missing standard trait implementations for all types (Debug, Default, Clone, Copy, PartialEq, Eq, etc.)
- missing with_field builder methods for all config structs
- thiserror (nostd) or similar better error handling
