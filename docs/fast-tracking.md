# Fast pedal tracking

How the expression controller gets from 2.6 Hz to well over 50 Hz per pedal: what limits the
update rate, what was measured on the PCB, and the design that follows from it.

## Where the time went

The sweep that solved every jack in turn took 382 ms:

- Every reading changes ADC channel, so the AD7718's sinc³ filter has to settle from scratch:
  three conversion periods, 9.5 ms at the old 315 Hz, plus ~0.7 ms of SPI traffic and
  chip-select guards (measured: 10.2 ms per reading).
- Every jack takes 9 readings (3 pairs x 3 taps). Empty jacks need the third pair to tell "arm 0
  isolated" from "nothing connected", and a pedal whose wiper is on the tip needs it because the
  tip, at ~0 Ω, is always the shared arm and leaves the first two pairs ill-conditioned.
- 4 jacks x 9 readings x 10.3 ms + 12 x 1 ms settling = 382 ms, with the two ADCs taking turns.

50 Hz leaves 20 ms per update: about two readings. No full solve fits, so speed has to come from
reading less (tracking instead of solving) and reading faster (a higher filter rate).

## Measurements

`adc_characterization` on a 10.85 kΩ pot pedal in jack 1 (tip = wiper, ring driven high, sleeve
low), bipolar coding, calibrated at every rate:

| Rate | Filter word | Reading incl. channel switch | Reading σ (wiper) | Output step | Position σ | Continuous period | Continuous σ (white) | 20 ms moving average σ |
|---|---|---|---|---|---|---|---|---|
| 1365 Hz | 3 | 2.89 ms | 1.93 mV | 3.88 mV | 887 ppm | 0.73 ms | 4.08 mV | 105 µV |
| 1024 Hz | 4 | 3.63 ms | 0.80 mV | 0.41 mV | 374 ppm | 0.98 ms | 2.00 mV | 71 µV |
| 819 Hz | 5 | 4.36 ms | 0.39 mV | 0.21 mV | 203 ppm | 1.22 ms | 1.18 mV | 52 µV |
| 682 Hz | 6 | 5.09 ms | 0.40 mV | 0.12 mV | 184 ppm | 1.47 ms | 0.78 mV | 40 µV |
| 315 Hz | 13 | 10.22 ms | 0.11 mV | 0.012 mV | 62 ppm | 3.18 ms | 0.13 mV | 25 µV |

One MIDI step is 1/127 = 7874 ppm of travel.

- **Output resolution scales with the cube of the filter word**, as the sinc³ decimation predicts
  (0.97 mV continuous at 3 vs 12 µV at 13 = (13/3)³). Single conversions at the fastest word
  are 4x coarser still (3.88 mV), which rules it out.
- **Continuous conversion is 2.5-3x noisier than single conversions** at the fast rates, with
  anticorrelated successive samples (white > σ): high-frequency noise, most likely SPI reads
  coupling into the conversion running underneath them. It gives more results per second but
  no less noise per unit time, so it is not worth a separate code path.
- **Mains hum dominates the pedal's noise at slow rates**: 100-140 µV at 50 Hz at every rate,
  ≤45 µV at 100 and 150 Hz. A 20 ms moving average removes it, but at ~60 ppm of travel it is
  irrelevant for MIDI anyway.
- **Calibration does not carry over between filter words**: ground reads -2.9, +0.9, +1.75,
  +1.5 and +1.3 mV at words 3-13, and running at word 3 on a calibration taken at 13 shifts the
  gain by -0.18%. The solver cancels offset and gain because it measures its own rails, but only
  when rails and taps share one filter word - so the firmware should use one word throughout.
- Switching-mode readings take 3 conversion periods plus ~0.7 ms of overhead at every rate.

**Conclusion: run everything at 819 Hz.** Its single-reading noise (0.39 mV, 0.45 mV white) is
what `ADC_VOLTAGE_NOISE` already assumes, so the solver keeps its tolerances while reading 2.3x
faster, with no averaging and no filter switching.

### Phase 1 result: 819 Hz and the third pair

At 819 Hz the full sweep dropped from 382 ms to 156 ms, but exposed a solver weakness that
already cost accuracy at 315 Hz. With the wiper on the tip, the two pairs sharing it are
conditioned roughly t / ((t + r)(t + s)), t being the wiper's contact resistance share: 0.03 at
0.5% (third pair measured, accurate) but 0.083 at 1.5%, just above the old threshold of 0.05.
The solver then split the track halves through a few millivolts across the wiper and amplified
the noise ~25x. `min_ratio_conditioning` is now 0.5, so such a pedal always takes the third pair
(its conditioning here is 0.95), at ~15 ms per sweep:

| Pedal at 0.21, held still | Position σ | Total σ | Sweep |
|---|---|---|---|
| 315 Hz, threshold 0.05 | 2588 ppm | 0.91% | 382 ms |
| 819 Hz, threshold 0.05 | 4880 ppm | 1.75% | 156 ms |
| 819 Hz, threshold 0.5 | 274 ppm | 0.11% | 171 ms |

## Pull resistor value

Voltage across a pedal of R with pulls of Rp: 2.5 V x R / (R + 2Rp); drop across each pull
(what the loop current is read from): 2.5 V x Rp / (R + 2Rp).

| Pulls | 10 kΩ pedal (span / drop) | 100 kΩ | 500 kΩ |
|---|---|---|---|
| 1 kΩ (fitted) | 2.08 V / 208 mV | 2.45 V / 24 mV | 2.49 V / 5 mV |
| 2.2 kΩ | 1.74 V / 382 mV | 2.40 V / 53 mV | 2.48 V / 11 mV |
| 4.7 kΩ | 1.29 V / 606 mV | 2.29 V / 107 mV | 2.45 V / 23 mV |
| 10 kΩ | 0.83 V / 833 mV | 2.08 V / 208 mV | 2.45 V / 49 mV |

Position comes from the span and is fine with any of them. Only the absolute total resistance of
high-value pedals depends on the drop; with 1 kΩ and 0.4 mV noise every pedal up to ~500 kΩ still
resolves, so the resistors stay. 4.7 kΩ would be the value to fit if accurate totals of
250-500 kΩ pedals ever matter.

## Design

Implemented in `expad-topology`'s `JackMonitor` (monitor.rs, host-tested in tests/monitor.rs)
and the firmware's `JackScanner` (src/topology/scanner.rs).

### Plug detection

Each jack's tip switch contact (TN) touches the tip while no plug is inserted. Pulling the tip
low and TN high with ring and sleeve floating, TN reads 1.25 V (the two 1 kΩ pulls in series)
without a plug and 2.5 V with one. Nothing flows through the pedal while ring and sleeve float,
so the check is independent of how the pedal is wired - including a wiper on the tip - and of
its position, end stops and shorted pins included. It costs one reading plus the re-settling of
whatever the pedal was driven with before.

### Per-jack states

| State | Readings | Leaves when |
|---|---|---|
| Empty | Plug check every 50 ms | Plug seen: Identify |
| Identify | Back-to-back full solves (rails re-measured first when older than 30 s) | The solve classifies the network (below) |
| Tracking | Track ends driven (the start low, the other end high; see Position below), only the wiper read; one end tap read every ~25 ms, alternating | An end tap's pull drop strays from what the solve implied (6σ, or 25% on its first reading and 3% after), or the wiper leaves the span of the ends (unplugged, polarity switched): Identify |
| Other | Back-to-back full solves (switch pedals, rheostats, TS cables), reported like today | The classification changes, or the solve finds nothing |
| Open | A plug check and a full solve every 100 ms | Something conducts, or the plug is gone |

Identify classifies each solve:

- **Potentiometer** (every arm resolved, exactly one near the star point): Tracking, remembering
  which arm is the wiper until the jack is next seen empty.
- **Potentiometer at an end stop** (a second arm within 2% of the star point, the third carrying
  the track): the wiper touches one track end, shorting two pins. Position is still unambiguous
  unless the shorted pair is ring and sleeve (a ring wiper on the sleeve end, or a sleeve wiper
  on the ring end - opposite positions). Track straight away, taking the per-jack wiper
  setting, the wiper remembered for the jack, the tip or the ring as the wiper. A wrong
  guess drives the true wiper as a track end, so the end-to-end resistance the track ends see
  changes with the pedal's travel; the 3% end-drop tolerance (after the first reading) notices that within ~5% of travel
  (host test) and the jack is identified again once the pedal slows down.

  The end-stop threshold was 10% at first, the wiper's own allowance: the PCB's pedal fully
  closed leaves 9% of its track on the sleeve, so it was taken for an end stop and solved over
  and over instead of tracked.
- **Nothing conducts**: plug check, then Empty or (plugged cable with nothing at the end) a full
  solve every 100 ms.
- **Anything else**: Other.

While tracking, a pedal returning to an end stop is harmless: the wiper then reads exactly one
end tap, and the remembered roles keep the position at 0 or 1. Before, the ambiguity let the
position flip between 0 and 1 at the ambiguous stop, where the wiper's contact resistance and
the shorted track end are both near 0 Ω.

### Position

The position is the share of the track between its start and the wiper. The start is the sleeve
whenever the sleeve is a track end, so the value rises as the wiper leaves the sleeve - the TRS
convention of both common wirings (wiper on tip, ring as reference, sleeve grounded; or wiper on
ring, tip as reference) - and the lower-numbered end otherwise. The first version counted from
the lower-numbered end, which read the PCB's tip-wiper pedal backwards (bright when closed).
With the sleeve convention that pedal reads 0.09 closed and 1.0 open.

Each track end's first reading is checked against the pull drop the identifying solve implies,
so a pedal unplugged the moment tracking starts is not taken for the reference; that check is
loose (25%), since solves of a pedal on an end stop were seen 10% off in total resistance on the
PCB - held to 3% from the start, tracking dropped such a pedal at its first end-tap check. From
then on the first reading is the reference, refined by every agreeing one and held to 3%. A freshly plugged network is settled for as if it were 100 kΩ until its first solve;
after three rejected solves in a row, for as long as allowed (400 ms per pair).

### Range and output

The web interface sets a range per jack (`minimum`/`maximum` of the wiper position, with
buttons taking the current position), which `JackSettings::value` stretches to the full
expression range before inverting. The status carries both the raw position and that value.

### Scheduling

Both ADCs convert in parallel: every slot applies the drives the next due jack on each ADC needs
in one shift-register write, starts a conversion on both chips and waits for both. Tracked jacks'
drives stay applied while the other jack on the same ADC is read, so alternating between them
costs no tap settling, only the ADC's own. Each jack carries a due time (as soon as possible
while tracking or identifying, every 50 ms while empty), and each ADC serves its most overdue
jack.

### Rates at 819 Hz

Measured on the PCB with the pedal in jack 1, after trimming the SPI overhead (phase 6):

| At 819 Hz | 1 MHz SPI, 50 µs chip select guard | 4 MHz, 10 µs (default) |
|---|---|---|
| Reading, switching channels | 4357 µs | 4021 µs |
| Reading, same channel (no control register write) | 4188 µs | 3985 µs |
| Wiper noise (σ) | 442 µV | 431 µV |
| Outlying codes in 300 readings | 0 | 0 |

Three conversion periods alone take 3.66 ms, so ~0.35 ms of overhead is left per reading.

| Situation | Update rate per pedal |
|---|---|
| One pedal on its ADC | 168/s before the SPI trim, 188-189/s after (measured; ~15% of readings go to end-tap checks) |
| Two pedals on one ADC, or all four | ~95/s (expected: the two jacks alternate) |
| Plug-in to tracking | ≤ ~0.1 s (50 ms plug poll + ~45 ms identify; ≤ 120 ms in the host tests) |
| Unplug noticed | ≤ ~0.1 s (next end-tap check within 25 ms, a solve finding nothing, a plug check) |

Before: one full solve of every jack per 382 ms sweep, 2.6 updates per second.
