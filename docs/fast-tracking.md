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
| Identify | Plug check, then back-to-back full solves | The solve classifies the network (below) |
| Tracking | Track ends driven (lower-numbered end low, higher high), only the wiper read; one end tap read every ~25 ms, alternating | An end tap's pull drop disagrees with the identified total (unplugged, polarity switched): Identify |
| Other | A full solve every ~40 ms (switch pedals, rheostats, TS cables), reported like today | The classification changes, or the solve finds nothing: Identify |

Identify classifies each solve:

- **Potentiometer** (every arm resolved, exactly one near the star point): Tracking, remembering
  which arm is the wiper until the jack is next seen empty.
- **Potentiometer at an end stop** (two arms near the star point, the third finite): the wiper
  touches one track end, shorting two pins. Position is still unambiguous unless the shorted
  pair is tip and sleeve (the ring's index lies between them, so the two readings of which pin is
  the wiper give opposite positions). Report the position using, in order, the wiper remembered
  for the jack, a per-jack setting, or the tip; keep solving (~25 Hz, and a pedal resting at a
  stop is not moving) and switch to Tracking as soon as the pedal leaves the stop.
- **Nothing conducts**: plug check, then Empty or (plugged cable with nothing at the end) a full
  solve every 100 ms.
- **Anything else**: Other.

While tracking, a pedal returning to an end stop is harmless: the wiper then reads exactly one
end tap, and the remembered roles keep the position at 0 or 1. The same ambiguity currently
lets `wiper_position` flip between 0 and 1 at the tip-sleeve stop, where the wiper's contact
resistance and the shorted track end are both near 0 Ω.

### Scheduling

Both ADCs convert in parallel: every slot applies the drives the next due jack on each ADC needs
in one shift-register write, starts a conversion on both chips and waits for both. Tracked jacks'
drives stay applied while the other jack on the same ADC is read, so alternating between them
costs no tap settling, only the ADC's own. Each jack carries a due time (as soon as possible
while tracking or identifying, every 50 ms while empty), and each ADC serves its most overdue
jack.

### Expected rates at 819 Hz (4.4 ms per slot, ~3.7 ms with trimmed SPI overhead)

| Situation | Update rate per pedal |
|---|---|
| One pedal on its ADC | ~200 Hz (~15% of slots go to end-tap checks) |
| Two pedals on one ADC, or all four | ~100 Hz |
| Plug-in to tracking | ≤ ~0.1 s (50 ms plug poll + ~40 ms identify) |
| Unplug noticed | ≤ ~50 ms |
