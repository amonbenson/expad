# Oscilloscope Measurement Guide

Practical companion to the measurement program in
[final-presentation-scaffold.md](final-presentation-scaffold.md). Written for someone who has
never used an oscilloscope. It covers the five measurements that need one: M4 (recognition time),
M5 (latency), M7 (rails and SPI), and M8 (tap settling).

Every board location in here was taken from `hardware/ExpressionController.kicad_pcb` and the
exported netlist, so the pin names and positions are checked rather than remembered. Coordinates
are KiCad page coordinates: use them to find a part in the PCB editor (View > Show > Cursor
coordinates), not with a ruler on the board. **Physically, find everything by its silkscreen
label** (`TP7`, `J2`, `C4`), which is what those labels are for.

---

# Part 1: Read this before you touch anything

## The one mistake that matters on this board

**A jack's sleeve contact is not ground.** On every other piece of audio equipment you have ever
seen, the sleeve of a 6.35 mm jack is ground, and clipping a scope's ground lead to it is the
normal thing to do. On this board it is wrong.

Verified from the netlist: jack 1's sleeve pad (`J2.S`) is on net
`/AnalogFrontend/AnalogQuadChannel1/JACK3`, which runs to a 10 kOhm resistor into the ADC and to
two analog switches. It is a measured, switched, high-impedance node. So are tip, ring and the tip
switch. None of the four is ground.

If you clip the scope's ground lead to a jack contact, you connect that measurement node to mains
earth through the scope. Nothing will catch fire (it goes through 1 kOhm pull resistors), but every
reading from that jack becomes meaningless and you may confuse yourself for an hour. Use a real
ground pad from the table in Part 4.

## Why the ground lead is dangerous in general

The black clip on a scope probe is not a neutral "reference". On a bench scope it is wired to the
chassis and through the mains cable to earth. Clipping it to a node that is not at ground potential
shorts that node to earth through a thin wire. On this board the nodes that would actually hurt are
`VEXT` (the barrel jack input, 12 V class), `+5V`, `+3.3VA` and `+2.5V`.

Rule: **the black clip only ever goes on GND.** If you want to look at the voltage between two
non-ground points, that is a differential measurement, and you do it with two channels both
referenced to GND plus the scope's `Ch1 - Ch2` math function. Never by moving a ground clip.

A second consequence: if you use two probes at once, both black clips are connected to each other
inside the scope. Putting one on GND and the other on a different node quietly shorts them.

## Search terms for this section

`oscilloscope probe ground lead safety`, `oscilloscope ground loop`, `differential measurement with
single-ended probes`, `floating an oscilloscope` (and why you should not).

---

# Part 2: Your scope and your probe

## The probe

A passive probe has four parts worth knowing:

- **Tip.** Usually a sprung hook that clips onto a wire or a component leg. Most probes have a
  removable hook revealing a rigid needle underneath, which is what you want for poking into a
  through-hole pad.
- **Ground lead.** A short black wire ending in a crocodile clip, coming off the probe body near
  the tip.
- **Attenuation switch.** A small slider marked `1X` and `10X`. In 10X the probe divides the
  signal by ten before it reaches the scope. This is the normal position: it raises the probe's
  input resistance to about 10 MOhm, lowers its capacitance, and preserves the probe's full
  bandwidth. **In 1X you get ten times more sensitivity but far less bandwidth (often about 6 MHz)
  and much heavier loading.** For the millivolt-level rail measurements that trade is worth it; for
  everything else use 10X.
- **Compensation trimmer.** A small screw, usually in the probe body or the BNC end, adjusted with
  a plastic tool. See Part 3.

**The scope must be told which attenuation you selected.** Many scopes do not detect it. If the
probe is on 10X and the scope thinks 1X, every voltage you read will be ten times too small, and
you will not notice because the waveform shape is right. Find the channel menu, `Probe` or
`Attenuation`, and set it to match. **Check this every time you move the slider.** More
measurements have been ruined by this than by anything else.

## The controls you will actually use

- **Vertical (Volts/div).** How many volts one vertical grid square represents. Turn it down until
  the waveform fills most of the screen; a waveform occupying one square wastes most of the scope's
  resolution.
- **Vertical position / offset.** Moves the trace up and down. Also find the "ground marker", a
  small arrow at the screen edge showing where 0 V is for that channel.
- **Coupling: DC or AC.** DC shows the real voltage. AC subtracts the average, so a 2.5 V rail with
  1 mV of ripple becomes a 1 mV wiggle around zero that you can then magnify. Use DC unless you are
  hunting for ripple.
- **Horizontal (Time/div).** How much time one horizontal square represents. Total screen width is
  usually 10 or 12 squares.
- **Trigger.** The scope needs to be told when to start drawing, or the trace smears. You set a
  **source** (which channel), a **type** (usually `Edge`), a **slope** (rising or falling) and a
  **level** (a voltage). The scope draws one screenful each time that channel crosses that voltage
  in that direction.
- **Trigger mode: Auto, Normal, Single.**
  - `Auto` draws something even when the trigger condition never happens. Good for finding a signal
    you are not sure is there. Misleading once you care about timing.
  - `Normal` draws only on a real trigger. Use this for repeating signals.
  - `Single` arms once, captures one screenful on the next trigger, then freezes. **This is the mode
    for one-off events like plugging in a pedal.** It is the single most useful mode in this guide.
- **Acquisition mode.** Usually `Normal`, `Average`, `Peak Detect`, `High Resolution`.
  - `Average` overlays many captures and averages them, reducing random noise. Only valid for a
    repeating, trigger-stable signal, and it will hide single-shot glitches.
  - `High Resolution` (or `Hi-Res`) oversamples and gives you extra effective bits at slow
    timebases. Excellent for rail ripple.
  - `Peak Detect` catches narrow spikes between samples. Use it to check you are not missing a
    glitch, not for measuring amplitudes.
- **Bandwidth limit.** A per-channel option, usually `20 MHz`. It throws away everything above
  20 MHz. On a slow board like this one, high-frequency content is almost all pickup and scope
  noise, so switching it on makes low-level measurements much cleaner and more honest.
- **Measure / Cursors.** `Measure` gives automatic readouts (peak-to-peak, RMS, rise time,
  frequency). `Cursors` gives two draggable lines with a readout of the difference between them,
  which is how you time anything the automatic measurements do not cover.

## Search terms

`oscilloscope volts per division`, `oscilloscope trigger modes auto normal single`,
`oscilloscope acquisition modes averaging high resolution`, `oscilloscope bandwidth limit`,
`10x probe attenuation`.

---

# Part 3: The setup ritual, once per session

Do this every time you sit down. It takes two minutes and it prevents most bad data.

1. **Power the scope on and let it warm up** while you do the rest.
2. **Set probe attenuation.** Slider to `10X`, and set the channel's probe setting to `10X`.
3. **Compensate the probe.** Every scope has a small metal tab or loop on the front marked
   something like `Probe Comp`, `Cal`, or `1 kHz`. It puts out a square wave.
   - Clip the probe tip to it and the ground lead to the adjacent ground tab.
   - Set the timebase and vertical so you see two or three square-wave cycles filling the screen.
   - Look at the top edge of the square wave. It should be **flat**. If it slopes up into a corner
     (overshoot) or sags (undershoot), turn the compensation trimmer with a plastic tool until it
     is flat.
   - **Why this matters:** an uncompensated probe distorts exactly the thing you are measuring in
     M8 (an RC settling curve) and M7 (edge shapes). An uncompensated probe can make a clean edge
     look like it overshoots by 20%.
   - Repeat for the second probe. Probes are not interchangeable once compensated; keep each one on
     its channel.
4. **Measure your own noise floor.** Clip the probe tip directly onto its own ground clip so the
   probe is measuring zero volts. Set AC coupling, bandwidth limit on, and the most sensitive
   Volts/div you plan to use. Read the peak-to-peak and RMS. **Write this number down.** Any ripple
   measurement you later make that is close to it is not a measurement of the board, it is a
   measurement of your scope. Saying so in the presentation is a strength, not a weakness.
5. **Check the board's ground is where you think.** With the board powered, DC coupling, put the
   probe tip on your chosen ground pad and the clip on another ground pad. It should read
   approximately 0 V. If it does not, one of them is not ground.

## Search terms

`oscilloscope probe compensation`, `probe compensation square wave overshoot`,
`oscilloscope noise floor measurement`.

---

# Part 4: Where to connect on this board

## Test points: your main probe targets

The board has 15 test points. All of them are **through-hole pads, 1.5 mm pad with a 0.7 mm hole**,
placed on the bottom side. Because they are drilled through, you can reach them from either side,
but the silkscreen labels are on the bottom, so **you will be reading the labels with the board
upside down.**

| Test point | Signal | What it is | KiCad position |
|---|---|---|---|
| TP1 | `SR_OE` | Shift register output enable (all four) | 128.78, 139.19 |
| TP2 | `SR_LAT` | Shift register latch / RCLK (all four) | 135.38, 140.46 |
| TP3 | `SR_CLK` | Shift register clock (all four) | 141.99, 141.73 |
| TP4 | `SR_DATA` | Shift register serial data into the first chip | 122.17, 137.92 |
| TP5 | `SPI0_RX` | ADC data out to the MCU (MISO), both ADCs | 166.62, 134.37 |
| TP6 | `SPI0_NCS1` | Chip select, ADC 1 (jacks 1 and 2) | 164.08, 134.37 |
| TP7 | `SPI0_CLK` | ADC SPI clock, both ADCs | 159.00, 134.37 |
| TP8 | `SPI0_TX` | MCU data out to the ADCs (MOSI), both | 156.46, 134.37 |
| TP9 | `SPI0_NCS2` | Chip select, ADC 2 (jacks 3 and 4) | 153.92, 134.37 |
| TP10 | `SPI0_NRDY1` | Conversion-ready from ADC 1 | 151.38, 134.37 |
| TP11 | `SPI0_NRDY2` | Conversion-ready from ADC 2 | 146.30, 134.37 |
| TP12 | `+5V_RAW` | 5 V regulator output, before jumper JP3 | 199.39, 147.32 |
| TP13 | `+3.3VA_RAW` | 3.3 V regulator output, before jumper JP1 | 178.82, 140.97 |
| TP14 | `+2.5V_RAW` | 2.5 V regulator output, before jumper JP2 | 171.20, 140.72 |
| TP15 | `VEXT` | Protected barrel jack input | 186.44, 140.21 |

TP5 to TP11 sit in one straight row on a 2.54 mm (0.1 inch) grid with two positions skipped, so
they look and behave like a header row. **Soldering a 0.1 inch pin header strip into TP5-TP11 is
worth the five minutes**: probe hooks grip header pins securely, and a probe tip that slips out of
a hole mid-capture while your other hand is holding a pedal is genuinely annoying.

The four rail test points sit on the regulator side of solder jumpers JP1, JP2 and JP3, so they read
each regulator's own output.

## Ground pads for the black clip

The board has 102 GND pads, but only 8 are through-hole and therefore easy to clip onto. Seven are
on the Pico's header, and one is the barrel jack's ground pin.

| Ground point | What it is | KiCad position |
|---|---|---|
| `A1` pin 3 | Pico header GND | 123.44, 145.29 |
| `A1` pin 8 | Pico header GND | 136.14, 145.29 |
| `A1` pin 13 | Pico header GND | 148.84, 145.29 |
| `A1` pin 18 | Pico header GND | 161.54, 145.29 |
| `A1` pin 23 | Pico header GND | 161.54, 127.51 |
| `A1` pin 28 | Pico header GND | 148.84, 127.51 |
| `A1` pin 38 | Pico header GND | 123.44, 127.51 |
| `J1` pin 1 | Barrel jack ground, biggest pad on the board | 189.83, 136.40 |

**Which one to use: always the nearest.** The ground lead is a wire loop, and a big loop picks up
interference and rings. Nearest ground for each probe point, computed from the layout:

| Probing | Nearest through-hole GND | Distance |
|---|---|---|
| TP1 `SR_OE` | `A1` pin 3 | 8.1 mm |
| TP2 `SR_LAT` | `A1` pin 8 | 4.9 mm |
| TP3 `SR_CLK` | `A1` pin 8 | 6.8 mm |
| TP4 `SR_DATA` | `A1` pin 3 | 7.5 mm |
| TP5 `SPI0_RX` | `A1` pin 23 | 8.5 mm |
| TP6 `NCS1` | `A1` pin 23 | 7.3 mm |
| TP7 `SPI0_CLK` | `A1` pin 23 | 7.3 mm |
| TP8 `SPI0_TX` | `A1` pin 23 | 8.5 mm |
| TP9 `NCS2` | `A1` pin 28 | 8.5 mm |
| TP10 `NRDY1` | `A1` pin 28 | 7.3 mm |
| TP11 `NRDY2` | `A1` pin 28 | 7.3 mm |

For the rails, do not use a through-hole ground at all: probe straight across the regulator's own
output capacitor, which is the shortest loop available (see M7a).

## Jack contacts, for the settling and timing measurements

Jack 1 is `J2`. Its pins are **through-hole, 3.4 mm pads with 1.5 mm holes** on the front side, so
they are the easiest things on the whole board to probe. Tip, ring and sleeve sit in a straight
vertical line 6.35 mm apart.

| Pad | Contact | KiCad position | Nearest GND |
|---|---|---|---|
| `J2.T` | Tip | 136.40, 177.22 | `U6` pin 8, 3.7 mm (SMD) |
| `J2.R` | Ring | 136.40, 183.56 | `U6` pin 8, 6.8 mm (SMD) |
| `J2.S` | Sleeve | 136.40, 189.91 | `C39` pad 2, 7.5 mm (SMD) |
| `J2.TN` | Tip switch | 120.17, 177.22 | `R54` pad 2, 5.3 mm (SMD) |

`J2.RN` and `J2.SN` are unconnected pads; ignore them.

There is no through-hole ground near the jacks. The nearest grounds are SMD pads, which are awkward
to clip to. Two options:

- **Best:** solder a 30 to 50 mm wire from any GND pad near the jack (`U6` pin 8 is the ground pin
  of jack 1's shift register) into a loop you can clip onto, and leave it there for the session.
- **Acceptable:** use `A1` pin 8 on the Pico header, about 32 mm away. The loop is larger, but the
  signals you are watching at the jack are slow (millisecond-scale RC curves and drive pattern
  changes), so a bit of extra loop inductance costs you nothing. **Do not** use this long ground for
  the SPI edge measurement, where it would matter.

## Search terms

`oscilloscope probe ground lead length inductance`, `probe ground spring`, `PCB test point probing`.

---

# M7a: Power supply rails

**What you are measuring.** How much the four rails wobble, at idle and while the LEDs and WiFi are
active. This closes item 1 of the milestone-1 evaluation plan.

**Name of this measurement:** *power supply ripple and noise measurement*. In power supply
datasheets the specified quantity is often called **PARD** (periodic and random deviation), and the
LDO's own spec sheet calls the frequency-dependent version **PSRR** (power supply rejection ratio)
and its self-noise **output noise spectral density**. Searching any of those gets you good material.

## Connections

One probe. For each rail in turn, probe **across the regulator's own output capacitor**, which means
tip on the capacitor's rail pad and the ground clip (or better, a bare wire) on the capacitor's
ground pad, 1.9 mm away. This is the smallest possible loop and therefore the honest measurement.

| Rail | Capacitor | Rail pad | GND pad | Equivalent test point |
|---|---|---|---|---|
| `VEXT` (input) | `C5` | pad 1 at 191.77, 144.97 | pad 2 at 191.77, 143.07 | TP15 |
| `+3.3VA` | `C4` | pad 1 at 178.82, 143.07 | pad 2 at 178.82, 144.97 | TP13 |
| `+2.5V` | `C8` | pad 1 at 171.20, 143.07 | pad 2 at 171.20, 144.97 | TP14 |
| `+5V` | `C6` | pad 1 at 199.39, 144.97 | pad 2 at 199.39, 143.07 | TP12 |

These are 0805 parts, so the pads are about 1 x 1.45 mm. If that is too fiddly, fall back to the
test point with the nearest ground (TP13 to `U3` pin 2 is only 2.9 mm; TP14 to `C8` pad 2 is 4.2 mm;
TP12 to `C6` pad 2 is 4.3 mm; TP15 to `U1` pin 2 is 3.2 mm) and say in the presentation that you
measured at the test point rather than across the cap.

Also worth measuring, because they are what the ADC actually sees: `C13` (138.94, 160.85 rail /
138.94, 162.75 GND) and `C20` (181.61, 160.85 / 181.61, 162.75) are the 2.5 V decoupling caps right
next to the two ADCs, after the jumper.

## Scope settings

Written for the Rigol MSO5104 with a PVP2350 probe. Menu names are given by function; if a label
differs slightly on your firmware, the function is what to look for.

### The DC value comes first, and not from the scope

Read each rail's DC voltage with a **multimeter**, not the scope. An 8-bit scope's DC gain accuracy
is a few percent, so a scope reading of "2.493 V" really means somewhere in 2.42 to 2.57 V. The
multimeter number is the one to put on a slide.

The board cannot check this itself: `AIN10` is tied to the same +2.5 V that is the ADC reference, so
it reads full scale by construction whatever the rail is actually doing.

### Three settings decide whether this measurement works at all

1. **Probe ratio 1X**, if your probe has a 1X position on its slider. Set the channel's probe ratio
   to 1X to match.
   In 10X the probe divides by ten before the scope, so the scope's own input noise appears **ten
   times larger** referred to the probe tip, and the most sensitive setting you can reach at the tip
   is 10 x 500 uV/div = 5 mV/div. You cannot resolve LDO ripple through that. 1X costs bandwidth
   (roughly 35 MHz), which does not matter for ripple.
   *If the probe is 10X only:* you are stuck with a floor of a few mV RMS. Say so explicitly in the
   presentation and report the rails as "at or below the measurement floor", which is a legitimate
   result. Steps 2 and 3 still help.
2. **Bandwidth limit 20 MHz.** Channel menu > `BW Limit` > `20M`. Noise grows with the square root of
   bandwidth, so going from 100 MHz to 20 MHz is about a 2.2x improvement.
3. **Acquisition mode `High Res`.** `Acquire` > `Mode` > `High Res`. This oversamples and averages
   adjacent samples, buying extra effective bits. Large effect at the slow timebases used here.
   Set `Acquire` > `Mem Depth` to a fixed high value (10M) rather than `Auto`, so there is plenty of
   oversampling for High Res to work with.

After all three, re-measure the floor. It should land well under 1 mV RMS. If it does not, something
else is wrong; do not proceed to the rails.

### AC coupling is what makes "RMS" mean "ripple"

Channel menu > `Coupling` > `AC`.

This matters more than it sounds. `Vrms` on the scope is the RMS of whatever is on screen,
**including DC**. On a DC-coupled 2.5 V rail, `Vrms` reads about 2.5 V and tells you nothing. AC
coupling removes the DC in the analog front end before the ADC, so `Vrms` becomes the ripple RMS,
and it also lets you turn the vertical scale right down into the ripple without the trace flying off
screen.

### Adding the measurements

`Measure` > add from the `Vertical` category:

- **`Vpp`** (peak to peak)
- **`Vrms`** (with AC coupling on, this is the ripple RMS)

Then turn **`Statistic` on**. The scope then shows `Cur`, `Avg`, `Max`, `Min`, `Dev` and `Count` for
each measurement, accumulated over many acquisitions.

**Report the `Avg` column for `Vrms`, and the `Max` column for `Vpp`.** Press `Reset Stat` at the
start of each condition, then wait.

The `Cur` column jitters from acquisition to acquisition, because each acquisition is a finite
sample of random noise. That is expected and is exactly what the statistics are there to absorb:
`Cur` is never the number you report.

How long to wait is not a fixed time, it is a question of how many acquisitions went in. Watch two
columns:

- **`Count`** is the real progress indicator. Deep memory makes acquisitions slow, often only a
  couple per second at 10 ms/div with 10M points, so a wall-clock minute may only be a hundred
  acquisitions.
- **`Dev`** is the spread between acquisitions. The uncertainty on `Avg` falls as `Dev` divided by
  the square root of `Count`.

**Stop when `Avg` has stopped moving in the digit you intend to report**, and write down `Count`
next to the result. `Count` matters most for `Vpp`: peak to peak is a maximum over everything seen,
so it keeps creeping up the longer you watch, and a `Vpp` quoted without its acquisition count is
not comparable to anyone else's.

Why both statistics matter:

- **`Vrms` is the reproducible number.** It is stable, it converges as more acquisitions accumulate,
  and it is what regulator datasheets quote. This is your headline figure.
- **`Vpp` is the worst-case number, and it is not reproducible.** Peak to peak is max minus min over
  the record, so on a noisy trace it grows with record length and with the number of acquisitions.
  Two people measuring the same rail get different `Vpp`. Quote it as a worst case, alongside the
  record length and acquisition count, never on its own.

### Timebase: take two captures per rail

- **10 ms/div** (100 ms of screen) with `High Res`. This is the low-frequency ripple capture: 50 Hz
  and 100 Hz mains content, and the roughly 100 ms WiFi beacon period. `High Res` is right here.
- **1 us/div** with `Acquire` > `Mode` > `Normal` and the 20 MHz limit still on. This is the
  broadband capture. Use `Normal`, not `High Res`: High Res is a filter, so it would under-report
  fast noise.

Report which acquisition mode produced which number. All three regulators are linear (ADP7142), so
there is no switching converter of the board's own, but the **Pico module has its own onboard buck**
on VSYS and the WS2812B LEDs draw in bursts, so high-frequency content can still appear.

Trigger: `Auto` mode is fine, since you are looking at noise rather than a specific event. For the
10 ms/div capture you can trigger on the channel itself near 0 V to stabilise a repeating component.

### The in-situ noise floor: the control test that decides everything

The bench noise floor from Part 3 is measured with the probe on its own clip, away from the board.
That is not the floor that applies at the measurement.

**Do this at the board, with the board powered and WiFi running:** touch the probe tip to the **same
ground pad its ground clip is attached to**, without changing anything else, and record `Vpp` and
`Vrms`.

The probe is now measuring zero volts through the real loop in the real electromagnetic environment.
Whatever it reads is pickup, not rail ripple.

- If the in-situ floor is close to what you measured on a rail, **you measured the loop, not the
  board.** Shrink the loop: probe across the regulator's output capacitor (1.9 mm between the rail
  pad and the ground pad) instead of test-point-to-header-pin, or fit a ground spring.
- If the in-situ floor is small and the rails are not, the rail figures are real.

**Run this once per condition** (idle, LEDs on, WiFi active), because pickup changes with what the
board is doing. It is the single most informative measurement in M7a: without it, a rail number is
just a number with no error bar.

A cross-check that costs nothing: the four rails sit in a cascade, `VEXT` into U1 (3.3 V) into U3
(2.5 V), and `VEXT` into U2 (5 V). Every LDO has substantial power supply rejection, so a
disturbance on `VEXT` cannot reach the 3.3 V rail at full amplitude, and certainly not the 2.5 V
rail behind two regulators. **If all four rails report the same ripple, the measurement is dominated
by something common to all four, which means pickup.** Real rail ripple must shrink as you move down
the cascade.

## Conditions to capture

Repeat the set for each state, because this is the comparison that makes the slide interesting:

- Board powered, Pico removed: the quiet baseline, just the regulators and the two ADCs.
- Idle: firmware running, no pedal plugged in.
- All four LEDs on at full allowed brightness. Worth a supply-current reading too, since the 200 mA
  rating of the 5 V regulator is the actual design constraint.
- WiFi access point up with a browser connected and the interface streaming.
- A pedal plugged in and being swept.

For each condition record six numbers per rail: `Vrms` average and `Vpp` maximum for the rail, and
the same two for the in-situ floor, plus the acquisition mode and timebase. A rail figure without
its floor is not reportable.

Note that supply current below about 10 mA will read as `0.00 A` on the DP932E, which is its display
resolution, not a measurement of zero. Expect a few mA with the Pico removed (two AD7718s plus the
regulators' quiescent current); use a multimeter in series if the exact figure matters.

## What to look out for

- **Attenuation mismatch.** If the rail reads 0.25 V instead of 2.5 V, the scope thinks the probe is
  1X. Fix the channel setting.
- **You are probably measuring your scope.** This is not a failure: it means you can honestly state
  "rail noise is at or below X, which is our measurement floor". Compare every rail figure against
  the in-situ floor for the same condition.
- **A rail reading below your floor proves contamination.** If a rail measures smaller than the
  floor alone, the "measurement" is entirely noise, since a real signal on top of noise cannot come
  out smaller than the noise by itself. Treat that as a settings problem, not a quiet rail.
- **Subtracting the floor.** Noise powers add, so for RMS values the ripple is
  `sqrt(measured^2 - floor^2)`. A rail at 500 uV against a 350 uV floor is really about 357 uV of
  ripple, not 500. This only works for RMS, never for peak to peak, and it becomes unreliable when
  the two are close: **if a rail reads less than about 1.4 times the floor, report it as "at or
  below the measurement floor" rather than subtracting.** With a low-noise LDO like the ADP7142 that
  is a likely and perfectly respectable outcome for the quiet conditions.
- **Remember to put the probe back to 10X** afterwards, both the slider and the channel setting, or
  every later measurement in this guide will read ten times too small.
- **AC coupling has a low corner** (a few Hz). Very slow drift will not show. That is what M10
  (warm-up drift, measured with the board's own ADC) is for.
- **Do not touch the probe tip with your fingers** while at millivolt sensitivity; you become an
  antenna.

---

# M7b: SPI signal integrity

**What you are measuring.** Whether the digital control signals are clean enough at 4 MHz: edge
shape, overshoot, and how long the chip select stays asserted around a transfer. This closes item 2
of the milestone-1 plan.

**Name of this measurement:** *signal integrity*. The specific quantities are **rise time** and
**fall time** (conventionally 10% to 90%), **overshoot** and **undershoot** (as a percentage of the
step), **ringing**, **setup and hold time**, and **propagation delay**. For the mechanism behind
what you will see, search **transmission line reflections**, **source series termination** and
**ground bounce**.

## Connections

Two probes, both with short ground leads to `A1` pin 23 (161.54, 127.51) or pin 28 (148.84, 127.51),
whichever is nearer.

- **Channel 1: TP7, `SPI0_CLK`** (159.00, 134.37). Nearest ground `A1` pin 23, 7.3 mm.
- **Channel 2**, pick per capture:
  - **TP8, `SPI0_TX`** (MOSI, 156.46, 134.37) to see data against clock.
  - **TP5, `SPI0_RX`** (MISO, 166.62, 134.37) to see the ADC's reply, which is the interesting one
    because it comes from the far end of the bus.
  - **TP6, `SPI0_NCS1`** (164.08, 134.37) to see the chip select framing, which is what the
    firmware's chip-select guard controls.

## Something to expect, not to be alarmed by

**Every digital line on this board has a 47 Ohm series resistor at its driver.** Verified: R3 to R6
and R46 to R49 at the microcontroller, R8 and R10 on the ADCs' data outputs, R7 and R9 on the ready
lines. That is deliberate source series termination: it slows the edges on purpose, to stop
reflections from ringing on an unterminated 2-layer board.

So a clean result here looks like a **visibly sloped edge with no ringing**, not a razor-sharp one.
Your measurement is a check that the slowing was enough and not excessive, not a hunt for ringing.
Do not "fix" a slow edge.

## Scope settings

1. Both probes `10X`, both channels set to `10X`, **bandwidth limit off** for this one. You are
   looking for fast detail, so do not filter it away.
2. Vertical: 1 V/div on both. The logic swings 0 to 3.3 V, so that fills three squares. Set the two
   channels' vertical positions so the traces do not overlap.
3. Coupling: **DC**. You care about the actual logic levels.
4. Trigger: source Channel 1, type `Edge`, slope rising, level about 1.65 V (mid-supply), mode
   `Normal`.
5. Three captures at three timebases. This is the part beginners skip, and it is where the
   information is:
   - **20 us/div or slower:** the whole transfer. You see the chip select go low, a burst of clock,
     and the chip select come back up. Measure the total burst length and the time from chip select
     falling to the first clock edge, and from the last clock edge to chip select rising. That last
     one is the **chip-select guard**, which the firmware sets to 10 us and which used to need 50 us
     on the breadboard.
   - **100 ns/div:** individual clock cycles. Check the period is about 250 ns (4 MHz) and that the
     duty cycle looks even.
   - **10 ns/div, zoomed onto one rising edge:** the edge itself. Use `Measure` to read **rise
     time**. Look at the flat part after the edge for overshoot and ringing.
6. For the edge captures, switch acquisition to **Average** (say 16 captures). The edges repeat
   identically, so averaging removes scope noise and gives you a clean edge to measure. **Then take
   one capture in `Peak Detect` as a cross-check**, to confirm averaging has not hidden a glitch.

## What to look out for

- **This bus is not continuously busy.** The firmware reads at 819 Hz, so transfers come in bursts
  roughly every 1.2 ms with long silence between. In `Auto` trigger mode you will see mostly flat
  line. Use `Normal` mode so the scope waits for a real transfer.
- **A long ground lead invents ringing.** If you see 20 MHz ringing on every edge, before believing
  it, shorten the ground connection and look again. This is the classic false positive, and it is why
  the nearest-ground table exists.
- **Probe both chip selects (TP6 and TP9) at some point.** The two ADCs share clock and data, and
  the driver converts on both chips at once, so confirming the chip selects never overlap is a real
  correctness check, not just a picture.
- **TP10 and TP11 (`NRDY1`, `NRDY2`) are worth one capture each.** These go active when a conversion
  finishes. Putting one of them on Channel 2 against the chip select on Channel 1 shows you the
  actual conversion timing: how long after the ready signal the firmware starts reading. That is
  directly the 0.35 ms of overhead mentioned in `docs/fast-tracking.md`.
- **Do not probe `SR_CLK` and `SPI0_CLK` expecting them to be related.** They are separate buses,
  SPI1 and SPI0.

---

# M8: Tap settling

**What you are measuring.** When the firmware changes which contacts are pulled up and down, the
jack contact and the ADC's input filter have to charge. The firmware waits a computed time before
reading. This measurement checks that model against the real curve.

This is the most valuable scope measurement in the programme, because it is the only one where the
answer is not already known.

**Name of this measurement:** *step response* of an RC network, and **settling time**. The number
you extract is the **time constant** (tau). Search `RC step response`, `RC time constant 63%`,
`settling time to 1 percent`, `first order step response`.

## The model you are testing

The firmware waits `2 x (network_resistance + 10 kOhm) x 10 nF`, clamped to between 1 ms and 400 ms.
Verified in `src/topology/scanner.rs` (`SettleConfig`) and the override at
`src/bin/expression_controller.rs:347`.

Two time constants is the part worth checking. After two time constants an RC curve has covered
about 86.5% of the step, leaving **13.5% of the step still uncorrected** when the firmware starts
the conversion. The argument that this is nevertheless fine is that the AD7718's sinc-cubed filter
averages over three conversion periods afterwards, so the residual is averaged down rather than read
directly.

That argument is plausible and untested. If your capture shows the residual is significant, that is
a real finding and belongs in the presentation. If it shows the curve is flat well before the
firmware reads, you have validated a design decision with a measurement, which is exactly what the
advisor asked for.

## Connections

Two probes.

- **Channel 1: TP2, `SR_LAT`** (135.38, 140.46), ground clip to `A1` pin 8 (136.14, 145.29), only
  4.9 mm away. This is the shift register latch: it pulses at the exact instant new switch settings
  take effect, so it is your time zero. This is the whole trick of this measurement.
- **Channel 2: a jack 1 contact.** Use `J2.R` (ring, 136.40, 183.56) or `J2.S` (sleeve, 136.40,
  189.91), whichever the drive pattern is switching. Ground: solder a wire loop from `U6` pin 8
  (jack 1's shift register ground, 3.7 mm from `J2.T`), or accept `A1` pin 8 about 32 mm away, which
  is fine at these speeds.

**Remember Part 1: the sleeve is not ground.** Channel 2's ground clip goes on a GND pad, not on the
jack.

## Scope settings

1. Both probes `10X`, channels set to `10X`, **bandwidth limit on** (you are looking at a
   millisecond curve; 20 MHz of noise only obscures it).
2. Channel 1 (`SR_LAT`): 1 V/div, DC coupling. Position it near the top of the screen, out of the
   way.
3. Channel 2 (the contact): 500 mV/div, DC coupling. The contact swings between roughly 0 V and
   2.5 V, so that is five squares. Position the trace so the whole swing is on screen.
4. Trigger: source **Channel 1**, type `Edge`, slope rising, level about 1.65 V, mode `Normal`.
   Triggering on the latch rather than on the analog curve is what makes the timing meaningful: you
   are measuring from the command to the response.
5. Timebase: start at **500 us/div**, which gives a 5 ms window, enough to contain a 1 ms wait plus
   the curve. Then zoom to **100 us/div** once you have found the edge.
6. Acquisition: **Average** over 16 or 64 captures, since the pattern repeats. Take one `Normal`
   single capture as well to confirm averaging is not smoothing away something real.

## What to measure

Use cursors rather than automatic measurements, because the quantities you want are specific:

- **Time constant.** Put one cursor at the start of the step, read the voltage there and at the end,
  then find the time at which the curve has covered 63.2% of the difference. That time is tau.
  Compare against `(network + 10 kOhm) x 10 nF` for the resistance you have plugged in.
- **How far the curve has settled when the firmware reads.** Measure at `2 x tau` after the latch
  edge and express the remaining error as a percentage of the step. The prediction is 13.5%.
- **Whether the 1 ms floor dominates.** For a 10 kOhm pedal the computed wait is
  `2 x (10 + 10) x 10 nF = 400 us`, which the 1 ms minimum overrides. So for common pedals the real
  wait is 1 ms, which is 2.5 time constants, not 2. Check that on screen: it is a nicer result than
  the calculation suggests, and worth saying.

## Run it with known resistors

This measurement is much more informative with a decade box than with a pedal, because you can set
the resistance and predict tau in advance. Do at least 10 kOhm, 100 kOhm and 500 kOhm and plot
measured tau against predicted. At 500 kOhm the predicted wait is
`2 x (500 + 10) x 10 nF = 10.2 ms`, which is comfortably visible, and it is the case where getting
it wrong would matter most.

## What to look out for

- **The probe adds capacitance.** A 10X probe adds roughly 10 to 15 pF, against the 10 nF already
  there. That is about 0.1%, so it is genuinely negligible here. Worth knowing, because on other
  measurements it would not be.
- **The contact you probe may not be the one moving.** The firmware changes which contacts are
  driven depending on what state the jack is in. Look at the Channel 2 trace in `Auto` mode first to
  find a contact that actually steps, then set up the trigger.
- **`SR_LAT` pulses for every jack, not just jack 1.** The four shift registers share the latch, so
  you will see latch pulses that do not correspond to any change on the contact you are watching. In
  `Normal` mode with averaging this shows up as a blurred Channel 2 trace. If it does, trigger on
  Channel 2's own edge instead and accept that you lose the absolute timing reference, or use the
  scope's `Trigger Holdoff` to skip pulses.

---

# M4: Recognition time, and the state machine on screen

**What you are measuring.** How long it takes from inserting a plug to the firmware tracking the
pedal. Documented as under about 0.1 s. This one doubles as the best picture in the presentation,
because the drive pattern on a contact changes visibly as the state machine advances, so one capture
shows plug detection, identification and following as three distinct regions.

**Name of this measurement:** *single-shot capture* of a one-off event. Related searches:
`oscilloscope single shot trigger`, `oscilloscope pre-trigger`, `oscilloscope deep memory`,
`oscilloscope segmented memory`.

## Why the tip switch is your trigger

Each jack has a tip switch contact (`TN`) that touches the tip while no plug is inserted. The
firmware checks for a plug every 50 ms by pulling the tip low and `TN` high, then reading `TN`: it
reads about **1.25 V with no plug** (two 1 kOhm pulls in series) and about **2.5 V with a plug**
(the tip switch has been pushed open). Verified in `docs/fast-tracking.md` and
`topology/src/monitor.rs`.

So `TN` carries a clean, monotonic, roughly 1.25 V step at the moment of insertion. That is your
trigger, and it is far more reliable than trying to trigger on the mechanical scrape of the plug
going in.

## Connections

Two probes, on jack 1 (`J2`).

- **Channel 1: `J2.TN`** (tip switch, 120.17, 177.22). Ground: a wire from `R54` pad 2 (5.3 mm away)
  if you solder one, otherwise `A1` pin 8.
- **Channel 2: `J2.R`** (ring, 136.40, 183.56) or `J2.T` (tip, 136.40, 177.22). This is the trace
  that shows the drive pattern changing through the states.

## Scope settings

1. Both probes `10X`, channels `10X`, bandwidth limit on.
2. Both channels: 1 V/div, DC coupling, vertical positions set so the two traces are separated and
   both fit.
3. Timebase: **20 ms/div**, giving a 200 ms window. The documented recognition time is about 100 ms,
   so this fits comfortably with room either side. If your scope has deep memory, 50 ms/div with a
   500 ms window shows the whole story including a few plug-check cycles beforehand.
4. Trigger: source **Channel 1** (`TN`), type `Edge`, slope **rising**, level about **1.9 V**
   (between the 1.25 V unplugged and 2.5 V plugged levels), mode **`Single`**.
5. **Set the pre-trigger position to about 25% from the left**, so you capture some of the
   before-state as well as the after. The control is usually a horizontal position knob or a
   `Delay` setting, and the little marker at the top of the screen shows where the trigger sits.
6. Press `Single` to arm. The scope shows `Ready` or `Armed`. Then insert the plug.
7. The scope freezes on the capture. Save it as a screenshot **and as a data file if the scope
   supports it**, because you will want to annotate it later.

## What you should see, and how to read it

Because `TN` is only driven during the plug check, Channel 1 will show a **series of pulses every
50 ms** rather than a continuous level. The insertion shows up as the pulse amplitude changing from
about 1.25 V to about 2.5 V. That periodic pattern is itself worth pointing at on the slide: it is
the 50 ms plug poll, visible.

On Channel 2 you should then be able to mark out three regions and measure each with cursors:

- **Before:** quiet, with brief activity every 50 ms as each plug check happens.
- **Identification:** a dense burst of activity, as the solver drives pairs of contacts against each
  other back to back. Documented at roughly 45 ms.
- **Following:** a settled, regular pattern, since a tracked pedal is driven with the two track ends
  held and only the wiper read, with one end tap checked every 25 ms.

The interval from the first high `TN` pulse to the start of the settled pattern is your recognition
time. Compare it against the documented figure of under 0.1 s.

## Also capture the unplug

Same setup, trigger slope **falling**, level 1.9 V again. Documented as noticed within about 0.1 s.

## What to look out for

- **Press `Single` before you plug in.** Obvious, easy to forget, and you will do it wrong at least
  once.
- **You get one attempt per arming.** If the capture is bad, press `Single` again and pull the plug
  out first.
- **Mechanical insertion is not instantaneous.** A plug takes tens of milliseconds to slide in, and
  it connects contacts in sequence on the way. That is not scope error, it is the real phenomenon,
  and it is exactly what caused the plug-insertion bug described in the presentation scaffold
  (section 5.3). Capturing it is a bonus.
- **Insert the plug slowly once, deliberately,** as a separate capture. The transient sequence will
  be clearer and it is the more interesting picture.
- **Timebase too fast is the usual failure.** At 1 ms/div you capture 10 ms and see nothing
  meaningful. Start slow, then zoom into the saved capture if your scope allows it.

---

# M5: End-to-end latency

**What you are measuring.** The time from the pedal's resistance changing to the MIDI message
reaching the host. This is item 6 of the milestone-1 plan and the one measurement with no obvious
instrument, so it needs splitting into two parts measured two different ways.

**Name of this measurement:** *propagation delay* or *time interval measurement* between two
channels. For the two-part approach, the general technique is called **latency budgeting**. For the
software half, search `MIDI timestamp latency measurement`, `USB MIDI latency`.

## The split

- **Part A, sensor to packet:** measurable on the scope, as the time from a resistance step to the
  firmware handing the MIDI packet to USB.
- **Part B, packet to host:** not visible on a scope, because it happens inside the USB stack and
  the host operating system. Measure it in software.

Report both and their sum, and say which dominates. A single number with no breakdown invites the
question "where does that time go?", which you then cannot answer.

## Part A: the scope half

**You need a firmware change first:** a spare GPIO that toggles at the moment the firmware hands a
packet to USB. This is a few lines in `src/bin/expression_controller.rs`.

**Check this before planning around it.** GPIO0 to GPIO5, GPIO7 to GPIO10, GPIO12 and GPIO26 to
GPIO28 are unconnected in the netlist, so a free pin exists in the design. But the Pico module is
soldered to the board with a header (`Pico_2W_with_Header`), so whether you can reach a specific
unused pin depends on how the header is populated and which side. **Look at the actual board before
writing the firmware change.** If the header is fully populated, any unused GPIO pin is directly
clippable, which makes this easy.

**Fallback if no pin is reachable:** trigger on `SR_LAT` at TP2 or a chip select at TP6, as a proxy
for firmware activity. Less direct, since it tells you when the firmware measured rather than when
it sent, but it needs no hardware change and still bounds the sensor-side latency.

### Connections

- **Channel 1: the resistance step.** Wire a pushbutton or a switch to short out part of a resistor
  network feeding jack 1, and probe the node that steps. Simplest version: a switch that shorts one
  arm, with the probe on the jack contact that changes (`J2.T` or `J2.R`). Ground as in M8.
- **Channel 2: the spare GPIO** on the Pico header, ground clip to the nearest `A1` GND pin, which is
  at most a few pins away.

### Scope settings

1. Both probes `10X`, channels `10X`, bandwidth limit on.
2. Channel 1: 500 mV/div, DC. Channel 2: 1 V/div, DC.
3. Trigger: source **Channel 1**, `Edge`, slope matching your step direction, level in the middle of
   the step, mode **`Single`** (the event is one-off, since you press the button once).
4. Timebase: **1 ms/div** to start, giving 10 ms. The update period at 819 Hz is about 1.2 ms and
   the full path should be a handful of milliseconds, so 10 ms should contain it. Widen to 5 ms/div
   if the GPIO edge falls off the right of the screen.
5. Pre-trigger position about 10% from the left, since everything you care about happens after the
   trigger.
6. Measure the interval with **cursors**, or use the scope's automatic `Delay` measurement between
   channels if it has one (often under `Measure > Delay` or `Time > Ch1 to Ch2`).

### What to look out for

- **Take many captures, not one.** The firmware polls, so the delay depends on where in the poll
  cycle your step lands. You will see it vary by roughly one update period. **Report a minimum,
  maximum and typical, not a single number** — the spread is the interesting part, and quoting one
  number when the real answer varies by a millisecond is the kind of thing that draws an awkward
  question.
- **A mechanical switch bounces.** The contact will chatter for a few milliseconds and the scope may
  trigger on the first blip. Either accept it and note it, or use the scope's **trigger holdoff** to
  ignore edges shortly after the first, or debounce the switch with a small RC. Bounce is also why
  an analog switch or a signal generator step is cleaner if you have one.
- **The step must be big enough to cross a MIDI value boundary.** One MIDI step is 1/127 of the
  configured range, so a tiny resistance change may produce no message at all and no GPIO edge. Make
  the step large, at least 10% of travel.

## Part B: the software half

Two options, both without a scope:

- **Round trip with the existing loopback firmware.** `src/bin/midi_loopback.rs` echoes every USB
  MIDI packet back to the host. Send a packet from the host, timestamp it, catch the echo,
  timestamp that. Half the round trip is a fair estimate of one direction, and it includes the host
  stack on both sides, so it is an upper bound.
- **Timestamping MIDI monitor.** Run a MIDI monitoring tool that timestamps incoming messages, and
  compare against a host-side reference event.

State clearly on the slide which of Part A and Part B you measured how, and that the sum is a sum of
two separately measured terms rather than one end-to-end capture. That is a completely respectable
result and much stronger than a single unexplained number.

---

# Appendix A: Glossary

- **Attenuation (1X / 10X).** How much the probe divides the signal. Must match the scope's channel
  setting.
- **Coupling (AC / DC).** Whether the scope shows the true voltage (DC) or subtracts the average to
  magnify the wobble (AC).
- **Volts/div, Time/div.** The scale of one grid square, vertically and horizontally.
- **Trigger level, slope, source.** The voltage, direction and channel that tell the scope when to
  start drawing.
- **Holdoff.** A dead time after a trigger during which the scope ignores further triggers. Cures
  false triggering on bouncing or bursty signals.
- **Pre-trigger.** How much of the screen shows time *before* the trigger. Essential for one-off
  events.
- **Persistence.** How long old traces stay on screen. Infinite persistence accumulates every
  capture, which is how you see jitter and rare outliers as a band rather than a line.
- **Peak detect.** An acquisition mode that keeps the extreme sample in each interval, so narrow
  spikes are not missed between samples.
- **Averaging.** Overlaying many triggered captures to cancel random noise. Requires a repeating,
  trigger-stable signal.
- **High resolution mode.** Oversampling and filtering to gain effective bits. Good for small
  signals at slow timebases.
- **Bandwidth limit.** A filter that discards high-frequency content, usually above 20 MHz.
- **Rise time.** How long an edge takes to go from 10% to 90% of its final value.
- **Overshoot.** How far past its final value an edge goes, as a percentage of the step.
- **Time constant (tau).** For an RC network, `R x C`. The curve covers 63.2% of a step in one tau,
  86.5% in two, 95% in three, 99.3% in five.
- **Settling time.** How long until a signal stays within some tolerance of its final value.
- **PARD.** Periodic and random deviation: the power-supply industry's name for ripple plus noise.
- **PSRR.** Power supply rejection ratio: how well a regulator attenuates disturbance on its input.

# Appendix B: Troubleshooting

| Symptom | Likely cause |
|---|---|
| Voltages ten times too small or too large | Probe attenuation does not match the channel setting |
| Flat line, nothing at all | Wrong pin; or the signal is bursty and the trigger mode is `Normal` with no trigger arriving; or the board is not powered (the analog rails need the barrel jack, not just USB) |
| Trace smears horizontally | No stable trigger. Set an appropriate source and level, and use `Normal` |
| Trace jumps around vertically | Probably a genuine floating node, or a ground clip that is not on ground |
| Ringing on every fast edge | Ground lead too long. Shorten it before believing the ringing |
| Square wave has rounded or peaked corners | Probe not compensated (Part 3 step 3) |
| Rail ripple looks the same on every rail | You are measuring the scope's noise floor, not the board |
| 50 Hz sine riding on everything | Mains pickup. Shorten the ground loop, keep the probe cable away from the power supply, and note the documented 50 Hz hum figures |
| Two channels short something when both connected | Both ground clips are joined inside the scope. Both must be on GND |
| Everything reads about 0 V at a jack | The contact is floating in the current state, which is a valid state, not a fault |

# Appendix C: Things to get before you start

- A second probe, compensated, if you do not have two. M8, M4 and M5 all need two channels.
- A 0.1 inch pin header strip, soldered into TP5 to TP11, and optionally TP1 to TP4.
- A short ground wire soldered to a GND pad near the jacks (`U6` pin 8 is the closest to jack 1),
  looped so you can clip onto it.
- A plastic trimmer tool for probe compensation.
- The barrel jack power supply. The analog frontend derives from it, `VBUS` is unconnected, and a
  Schottky blocks the Pico from back-feeding the 5 V rail, so **USB alone will not power what you
  are measuring.** Confirm this on the bench once, at the start.
- A decade resistance box or a set of precision resistors with a TRS plug wired to them. This is
  worth more to the evaluation than any scope measurement here, since it is what M2, M3 and the
  quantitative half of M8 all need.
