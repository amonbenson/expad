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
WS2812B LEDs below the jacks glow a dim white while a plug is in but no known pedal is behind it.
A recognised pedal is announced for a second in its own color - purple for an expression pedal,
lime for a footswitch, yellow for a rheostat - after which the LED shows the value it sends, from
dark red at 0 through red, orange and yellow to white at 127. They run off a linear regulator, so
the firmware caps them at ~4% brightness.

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

1. Plug the Pico into a computer over USB. It shows up as a USB MIDI device; by default the
   first two jacks send CC 11 (Expression) and the last two CC 1 (Modulation), all on MIDI
   channel 1.
2. Join the WiFi network **Expression Adapter** (password: the one it was built with, see below).
   Phones, tablets and computers open the interface by themselves as the network's sign-in page;
   otherwise open http://192.168.4.1.
3. Per jack, set the MIDI channel and controller, **Invert**, and the **Range**: move the pedal to
   one end and press **Min**, to the other and press **Max**, so its travel covers the full MIDI
   range. **Drive** bends the response against a pedal's nonlinear track: centered is linear,
   turning it right makes the value rise early in the travel, left late (double-click resets it).
   **Wiper** only matters for pedals whose wiper is on the ring or sleeve and that rest on an
   end stop when plugged in; **Auto** handles the common wirings. The LED slider sets the
   brightness.

The topology panel on the right draws the selected jack's measured network.

## Setup

Needs Rust, Node.js (see `engines` in [web/package.json](web/package.json)) for the web interface,
and a debug probe (e.g. a Raspberry Pi Debug Probe) on the Pico's SWD pins.

```bash
rustup target add thumbv8m.main-none-eabihf
cargo install probe-rs-tools --locked
cp .env.example .env   # then set EXPAD_WIFI_PASSWORD (8-63 characters)
```

## Building and flashing

```bash
cargo run --bin expression_controller   # build, flash and show the log
cargo run --bin expression_controller --features no-wifi   # the same with the radio off
cargo build --no-default-features       # without the web interface (no Node.js needed)
```

The other programs in [src/bin/](src/bin/) test parts of the board:

| Program | What it does |
|---|---|
| `detect_pin_mapping` | checks every contact's switches and ADC input against the wiring table (unplug all jacks) |
| `capture` | holds every contact in one state and logs every ADC input, for noise measurements |
| `adc_characterization` | compares ADC filter rates and SPI settings on a pedal in jack 1 |
| `potentiometer` | reads a pedal in jack 1 and shows its position on the LEDs |
| `rainbow` | LED strip test |
| `midi_loopback` | echoes every USB MIDI message back to the host |
| `web_interface` | the web interface with made-up data |

## Development

```bash
cargo test -p expad-topology --target x86_64-pc-windows-msvc   # solver tests, on your host triple
cargo fmt
cargo clippy --all-features
cd web && npm run dev:mock   # web interface against a simulated device, no hardware needed
```

[AGENTS.md](AGENTS.md) describes the repository in detail, and
[docs/fast-tracking.md](docs/fast-tracking.md) the measurements and design behind the pedal
detection.

## General TODOs
- missing standard trait implementations for all types (Debug, Default, Clone, Copy, PartialEq, Eq, etc.)
- missing with_field builder methods for all config structs
- thiserror (nostd) or similar better error handling
