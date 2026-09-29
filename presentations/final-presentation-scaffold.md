# Final Presentation Scaffold

Planning document for the final presentation of the Expression Controller project
(Sensor and Actuator Systems). This is not the presentation: it describes every section,
what it has to deliver, and what the audience should take away from it.

## How to read the verification markers

Every hardware and firmware claim below was checked against the KiCad project and the source.
Markers:

- **[V]** verified against a file in this repository. The source is named so it can be rechecked.
- **[?]** not provable from this repository. Either confirm it before it goes on a slide, or
  leave it off. Do not let a **[?]** become an assertion by accident.
- **[TODO]** a placeholder for content only you have (the project proposal, the demo hardware).

Everything unmarked is presentation advice, not a factual claim.

## Corrections made after auditing the board

The first draft of this scaffold contained claims that the board does not support. They are
listed here so they do not creep back in:

- **There is no ground star point.** The netlist has exactly one `GND` net with 102 nodes, poured
  on both copper layers. The AD7718s' `AGND` (pins 4, 17), `DGND` (pin 25), `AINCOM` (12) and
  `REFIN1-` (5) all sit on it. Do not present a star ground. **[V]**
- **There is no analog/digital supply split.** One 3.3 V rail, confusingly named `+3.3VA`, feeds
  the AD7718s' `AVDD` *and* `DVDD`, all four 74HC595s and all eight TMUX1511s. **[V]**
- **The pull resistors are not shared between jacks.** Each jack has its own pull-up and
  pull-down resistor. What jacks do share is the 2.5 V rail, the ground, the 3.3 V rail and the
  ADC. This changes the mechanism behind the crosstalk measurement (M9). **[V]**
- **The 2.5 V rail is an LDO, not a precision voltage reference.** It is an ADP7142AUJZ-2.5. It
  does serve as the ADC reference and the pull-up rail at the same time, so the ratiometric
  argument holds, but calling it a reference oversells it. **[V]**
- **The Pico's own 3.3 V regulator is not connected to the board rail.** Milestone 1 said the
  3.3 V could be toggled between the LDO and the Pico's onboard regulator. In the final design
  Pico pins 36 (3V3) and 37 (3V3_EN) are unconnected. The solder jumpers do something else
  (see 3.3). If you show the milestone power slide again, this is now wrong on it. **[V]**
- **There are no buttons.** Milestone 1's outlook listed "hardware buttons and LEDs". Only the
  four LEDs exist. **[V]**

## Audience model

- Electronics-literate: voltage dividers, ADCs, SPI, RC settling, sigma-delta filtering and
  op-amp-level circuit reading need no explanation.
- Expression-pedal-naive: nobody knows what an expression pedal is, what a TRS jack carries,
  or why pedal pinouts are a problem. Every pedal-specific term has to be introduced once.
- Partially new: milestones 1 and 2 were given, but not everyone was there. Every concept the
  rest of the talk depends on gets repeated, but compressed hard. Rule of thumb: a returning
  listener should never feel a slide is only for them, and a new listener should never be lost.
- Wants evidence: the advisor asked for the evaluation as the main part, with measurement
  series and plots. Numbers on slides, not adjectives.

## Time budget

Assumed 20 minutes of talk plus about 5 minutes of live demo, then questions. **[TODO]** confirm
the actual slot. Section minutes below sum to 20. If the slot differs, scale sections 1, 2 and 7
last and section 4 first, since the evaluation is what the advisor asked to be the main part.

| # | Section | Slides | Minutes |
|---|---|---|---|
| 0 | Title | 1 | 0.5 |
| 1 | Recap: problem and goal | 3 | 3 |
| 2 | Recap: measurement principle | 2 | 2.5 |
| 3 | Implementation overview | 5 | 4 |
| 4 | Project evaluation (main part) | 8-9 | 7 |
| 5 | Problems and solutions | 3 | 2.5 |
| 6 | Live demonstration | 1 | 5 (outside the 20) |
| 7 | Summary and outlook | 2 | 1.5 |
| - | Backup | 5-8 | 0 |

## Narrative spine

One sentence the whole talk serves: *a passive pedal whose wiring nobody standardised can be
identified electrically in about a tenth of a second and then tracked fast enough to feel
instant, and here is the measured proof.*

Two numbers the audience should be able to repeat afterwards: **about 0.1 s to recognise a
pedal** and **about 190 position updates per second**, with jitter **well under one MIDI step**.
All three are from `docs/fast-tracking.md`, measured on this PCB. **[V]** Every evaluation slide
should visibly serve one of those, or the correctness claim behind them.

---

# Section 0: Title

**Purpose.** Frame the project in one line for people who missed both milestones.

**Content.** Title, name, course, date. Keep the milestone slides' design so returning
listeners recognise the project immediately. Add a subtitle that already states the result,
e.g. "four jacks, any pinout, about 190 updates per second".

**Takeaway.** This is the pedal adapter project, and it works.

---

# Section 1: Recap: problem and goal (3 slides, 3 min)

Reuse the milestone-1 motivation slides as they are, then cut them down. This is the only
part of the talk where photos beat schematics.

## 1.1 What an expression pedal is

**Content.** Photo of a keyboard player or guitarist with a pedal, plus a product photo of a
pedal. Inside it is a single potentiometer moved by a foot treadle. State the one contrast
that fixes the idea: a sustain pedal is a switch, an expression pedal is continuous. Name the
use case: hands stay on the instrument while a parameter sweeps.

**Takeaway.** An expression pedal is a foot-driven potentiometer, nothing more, and it carries
no electronics of its own.

**Notes.** 45 seconds, no more. Resist the urge to re-explain the musical context.

## 1.2 Why an adapter is not trivial

**Content.** The actual problem: the pedal is passive, and no pinout standard exists. Show a
small diagram of the same 6.35 mm TRS jack wired the two common ways (wiper on tip with ring
as reference, or wiper on ring with tip as reference), plus the fact that some pedals are
two-wire, some carry a switch, and resistance values vary widely between models. Mention that
some pedals have a physical switch to pick between conventions, which is exactly the workaround
this project removes.

The two common wirings and the mono-cable behaviour are documented in `README.md` and
`docs/fast-tracking.md`. **[V]** The claim "nearly all pedals use a 6.35 mm TRS jack" comes from
the milestone-1 notes and the Mission Engineering reference in your source list; keep the
citation on the slide. **[?]** for any specific market share figure, so do not state one.

**Takeaway.** The signal is easy; the unknown is the wiring. A pedal that is plugged in
correctly can still read backwards, read nothing, or read a curve that rises and falls again.

**Notes.** This slide is the reason the project exists. If the audience only remembers one
problem statement, it should be this one.

## 1.3 Goal and requirements

**Content.** The target as a short requirement list, phrased so the evaluation can later be
scored against it one line at a time. **[TODO]** The list below is reconstructed from
`README.md` and the milestone notes, not from your project proposal. Replace it with the
requirements you actually committed to, then keep it identical to the scorecard in 4.9.

Reconstructed draft:

- Identify automatically what is plugged into each of four jacks, without user input. **[V]**
  four jacks, `board::JACK_COUNT = 4`.
- Support potentiometer pedals of either pinout, sustain switches, and two-wire rheostat pedals.
  **[V]** the supported-pedal table in `README.md`.
- Report position continuously as USB MIDI control change, fast enough to feel immediate. **[V]**
  USB MIDI class device, `src/hal/usb/midi.rs`.
- Notice plugging and unplugging while running, without a reset. **[V]** the plug-check states in
  `topology/src/monitor.rs`.
- Configure per jack (MIDI channel, controller, range, inversion, response curve) live. **[V]**
  `src/web/interface.rs`.

**Takeaway.** A handful of checkable promises. Section 4 will come back and tick them off.

---

# Section 2: Recap: measurement principle (2 slides, 2.5 min)

This is the intellectual core of the project and the part a new listener cannot do without.
Milestones 1 and 2 both spent time here, so compress to the two slides that carry the idea.
The milestone-2 animated build-up of the star network is the right asset to reuse.

## 2.1 The three-terminal model

**Content.** Any pedal, whatever its wiring, is a three-terminal resistive network: three arms
meeting at a star point. A potentiometer is the special case where exactly one arm is near
0 Ohm, and that arm is the wiper. So identifying the pedal reduces to solving for three arm
resistances and asking which one is near zero. Two-wire pedals are the case where one arm is
open.

Note the vocabulary collision and defuse it on the slide: **"star point" here means the node
where the three resistor arms meet, which is a property of the pedal, not a grounding scheme on
the PCB.** The board has no star ground, so do not let the two meanings sit on adjacent slides
unexplained. **[V]**

The classification thresholds are real numbers you can quote: an arm counts as the wiper at up
to 10 % of the total (`max_wiper_relative = 0.1`), and as shorted to the star point below 2 %
(`end_stop_relative = 0.02`), both in `topology/src/monitor.rs`. **[V]**

**Takeaway.** Pedal identification is not pattern matching, it is solving a three-resistor
network. Which arm is zero tells you the pinout; the ratio of the other two tells you the
position.

## 2.2 How the network is measured electrically

**Content.** Each contact can be pulled up through a resistor, pulled down through a resistor,
or left floating, and each contact is read by an ADC. Drive two contacts against each other,
leave the third floating: the floating contact's voltage fixes the ratio of two arms, and the
voltage drops across the known pull resistors give the loop current and therefore the absolute
total. Repeat over a short sequence of pairs and the network is determined.

The accurate version of the ratiometric claim, which is worth making precisely because it is the
basis of every accuracy number later: **the same 2.5 V rail is both the ADC reference
(`REFIN1+` on both AD7718s) and the rail every pull-up switches to, so a shift in that rail
moves the measurement and the reference together and largely cancels.** Verified from the
netlist: net `+2.5V` reaches `U4.6`, `U5.6` (REFIN1+) and `R50`-`R53` (the four pull-ups).
**[V]** Say "rail", not "precision reference": it is an ADP7142 LDO. **[V]**

**Takeaway.** Three pull states and one ADC per contact are enough to measure an unknown
resistor network in place, with no calibration of the pedal.

**Notes.** The milestone-2 live simulator (topology-sim) is a good 20-second aside here if the
demo machine is already on screen, but do not let it eat the time budget; it is backup material
now that real hardware exists.

---

# Section 3: Implementation overview (5 slides, 4 min)

Goal of this section: show that the thing is built, and make the later evaluation
interpretable. Lead with the block diagram so every subsequent slide has a home. Explicitly
flag what changed since milestone 2, because the advisor remembers the old schematic.

## 3.1 System block diagram

**Content.** One diagram, left to right, with the verified counts:

- Four 6.35 mm jacks with switched tip contacts, J2 to J5. **[V]**
- Per jack: one 74HC595 driving two TMUX1511 quad analog switches, one to the jack's 1 kOhm
  pull-up, one to its 1 kOhm pull-down. The four 74HC595s are daisy-chained (U6 `QH'` into
  U8 `SER`, and so on), driven from SPI1. **[V]**
- Two AD7718 24-bit sigma-delta ADCs on SPI0, each reading two jacks: J2 and J3 on U4, J4 and
  J5 on U5. **[V]**
- A 10 kOhm / 10 nF low-pass into every ADC input, corner about 1.6 kHz. **[V]** values
  `board::INPUT_FILTER_*` and R15-R18 / C23-C26; the 1.6 kHz figure is the arithmetic.
- Raspberry Pi Pico 2 W module (RP2350), then two outputs: USB MIDI to the host, and a WiFi
  access point serving a web interface. **[V]**
- Four WS2812B LEDs on GPIO6 through a 74AHCT1G125 level shifter. **[V]**

Totals worth putting on the slide: 16 switched contacts (4 jacks x tip, ring, sleeve, tip
switch), 8 TMUX1511s, 4 shift registers, 2 ADCs, 149 footprints on a 2-layer board of roughly
89 mm x 70 mm. **[V]** footprint count and layer stack from `ExpressionController.kicad_pcb`;
the outline figure is from the ground-pour extent, so **[?]** on the exact millimetre, measure
it if you want to state it precisely.

**Takeaway.** A small, entirely passive-input signal chain: switches, two precision resistors
per jack, and a slow high-resolution ADC. All the intelligence is in firmware.

## 3.2 Analog frontend, and what changed since milestone 2

**Content.** The per-jack schematic slice, with one detail that is worth making explicit because
it constrains the whole measurement scheme:

**All four switches in a jack's pull-up TMUX1511 share that jack's single 1 kOhm resistor, and
likewise for the pull-down.** Verified: `U15` sources S1-S4 all tie to `R50` (1 kOhm 0.1 %) to
`+2.5V`, and `U19` sources S1-S4 all tie to `R54` (1 kOhm 0.1 %) to `GND`. **[V]** So only one
contact can be pulled up and one pulled down at a time without putting contacts in parallel,
which is exactly the "one high, one low" drive pattern the solver and the tracker use. This is a
nice point: the resistor sharing is not a compromise, it matches how the measurement works.

Then a short "changed since milestone 2" list:

- Tristate buffer ICs (SN74LVC126A) replaced by TMUX1511 analog switches plus a shared precision
  resistor per rail. **[V]** that the change happened (milestone slides vs. schematic, and commit
  `f89b08d` "replaced buffer ICs with analog switches"). The reason to give: a switch plus a
  0.1 % resistor makes the pull impedance a known quantity, which is what absolute resistance
  measurement needs, where a buffer's output impedance is not specified for that. **[?]** on
  whether that was your actual reason at the time; state your own.
- ADC changed from the AD7124-8 shown at milestone 1 to the **AD7718**, one per two jacks, each
  with its own 32.768 kHz crystal. **[V]** that the part changed, and the crystals (Y1, Y2 with
  18 pF loading). **[?] the reason is nowhere in this repository.** Give your real reason in one
  clause, or say "for availability and cost" only if that is true. Do not invent it.
- Pull rail moved to the 2.5 V rail that also feeds the ADC reference, making readings
  ratiometric. **[V]**
- Added since milestone 2 entirely: the four per-jack indicator LEDs, and the WiFi web interface.
  **[V]** neither appears in the milestone-2 slides.
- Dropped since milestone 1: the hardware buttons from the outlook slide. **[V]** Mention it in
  passing if the old slide is reused; otherwise leave it for 7.1.

**Takeaway.** The measurement accuracy is designed into the frontend: two known resistors and
one shared rail per jack, everything else is switching.

## 3.3 The board and its power architecture

**Content.** Photo of the assembled PCB with a pedal plugged in, plus the layout render. The
verified architecture, which is worth one accurate sentence each:

- **Input protection:** barrel jack, then a polyfuse (MF-MSMF050-2), then an SMBJ15CA
  bidirectional TVS, into a reverse-polarity P-channel MOSFET (DMP3099L) whose gate is pulled to
  ground through 100 kOhm and clamped by a 15 V zener (BZX84C15). **[V]** all from the netlist.
  **[?]** the fuse's hold current, and the nominal input voltage: the 15 V clamp parts suggest a
  12 V class supply, but nothing in the repository states it. Check your BOM before quoting a
  number.
- **Rails:** three ADP7142 LDOs. 3.3 V (U1) and 5 V (U2) from the protected input, and 2.5 V (U3)
  **cascaded from the 3.3 V rail**, not from the input. **[V]** That cascade is worth one clause:
  the reference rail sits behind two regulators, which is good for its noise.
- **One 3.3 V rail for everything:** `+3.3VA` feeds both AD7718s' `AVDD` and `DVDD`, all four
  74HC595s and all eight TMUX1511s. Despite the "A" in the net name it is not an analog-only
  rail. **[V]**
- **One ground:** a single `GND` net poured on both layers of the 2-layer board, with the power
  input section given its own higher-priority pour ("Power Ground Pour", priority 1) inside the
  board-wide pour (priority 0), both on the same net. No AGND/DGND split, no single-point star,
  and the Pico's `AGND` pad is left unconnected. **[V]**
  **If a milestone-1 slide claiming a star point gets reused, fix or drop it.** The decision to
  do it this way was deliberate; **[TODO]** supply the one-sentence rationale yourself, since it
  is not recorded in this repository, and it is a fair question to expect.
- **Solder jumpers JP1, JP2, JP3** sit between each LDO's output and the distributed rail, so any
  rail can be opened, and the test point is on the regulator side. **[V]** This is directly
  useful in the evaluation: it is how you measure per-rail current.
- **Test points:** 15 of them, and they are on exactly the right signals. TP1 `SR_OE`, TP2
  `SR_LAT`, TP3 `SR_CLK`, TP4 `SR_DATA`, TP5 `SPI0_RX`, TP6 `SPI0_NCS1`, TP7 `SPI0_CLK`, TP8
  `SPI0_TX`, TP9 `SPI0_NCS2`, TP10 `SPI0_NRDY1`, TP11 `SPI0_NRDY2`, TP12 `+5V_RAW`, TP13
  `+3.3VA_RAW`, TP14 `+2.5V_RAW`, TP15 `VEXT`. **[V]** Every scope measurement in section 4 has a
  named probe point because of this. Say so; it reads as design foresight, which it is.
- **The 5 V rail is a 200 mA LDO shared by the four LEDs, the level shifter and the Pico's VSYS
  through a Schottky (PMEG6010ELR).** **[V]** the topology; the 200 mA figure is the ADP7142's,
  per reference [7] in your own source list. That is why LED brightness is capped in firmware at
  51/255, about 20 %. **[V]** `MAX_CHANNEL_VALUE = 51` in `src/hal/led/ws2812.rs`.

**Takeaway.** It is a real, manufactured, fully assembled board, it is functionally working, and
it was laid out so that it can be measured.

**Notes.** One consequence to know before the demo: the analog frontend derives entirely from the
barrel-jack input, the Pico's `VBUS` pad is unconnected, and the Schottky blocks the Pico from
back-feeding the 5 V rail. **[V]** So USB alone should power only the Pico, and external power
looks mandatory for anything to be measured or demonstrated. **[?]** Confirm on the bench, then
put it on the demo checklist in section 6.

## 3.4 Firmware architecture and the jack state machine

**Content.** Two halves on one slide. Left: the crate split. A `no_std` async firmware crate
(Rust, embassy) on the device, and `expad-topology`, a hardware-independent crate holding the
solver arithmetic and the per-jack state machine, which runs and is unit-tested on the host:
**50 tests, 25 against a simulated resistor network and 25 driving the monitor through a
simulated jack in simulated time.** **[V]** counted in `topology/tests/`.

Right: the existing state diagram from `docs/diagrams/jack-monitor-states.pdf`, reduced to its
groups rather than every state: plug detection, identification, following (three-wire
potentiometer or two-wire element), and the re-identification edges back. **[V]**

**Takeaway.** Identification is a state machine per jack, not a one-shot measurement, and its
logic is testable without hardware, which is why it can be trusted.

**Notes.** Do not walk the full diagram. Name the four groups, point at the two re-identify
arrows, move on. The full diagram goes in backup for questions.

## 3.5 Host interfaces: MIDI and the web UI

**Content.** Screenshot of the web interface laid out as a mixer strip, with the topology panel
drawing the solved network of the selected jack. Note that the interface shows the measured
circuit and a plain-language explanation of what each jack is doing, which is both a user
feature and the debugging tool that made the evaluation possible.

On the MIDI side, the accurate defaults: USB MIDI class device on MIDI channel 1, with jacks 1
and 2 defaulting to **CC 11 (Expression)** and jacks 3 and 4 to **CC 1 (Modulation Wheel)**.
**[V]** `DEFAULT_JACK_CONTROLLERS` in `src/web/interface.rs`. Note that `README.md` still says
all four default to CC 11, which is stale since commit `9ace1bd`; do not copy the number off the
README.

**Takeaway.** The device explains itself, live. This screenshot is also what the live demo will
show, so it seeds section 6.

---

# Section 4: Project evaluation (main part, 8-9 slides, 7 min)

This is what the advisor wants weighted heaviest. Structure it as a top-down argument, not a
list of experiments: first the promise, then the evidence layer by layer from the signal chain
up to the user-visible behaviour, then an honest limits slide, then the scorecard.

Presentation rules for this whole section:
- Every slide gets a headline that is a claim, not a topic. "Position noise is 1/29 of a MIDI
  step" beats "Noise measurements".
- Every plot gets axis units and, where relevant, the relevant threshold drawn in as a line
  (one MIDI step, the 50 Hz working target, the 25 % rheostat tolerance).
- Say out loud once, early, what the reference standard is for each claim: precision resistors
  for accuracy, the scope for timing, the board's own 24-bit ADC for noise.
- Name the test point for every scope measurement. You have one for every signal you need.

## 4.1 The evaluation plan, and what became of it

**Content.** Reproduce the milestone-1 testing and evaluation plan verbatim and put a status
against each line. The plan, from the milestone-1 and 2 slides, was: **[V]**

1. Power supply quality (voltage and SNR on the rails)
2. SPI signal integrity
3. Functional self-test: SPI device ID inquiry, and tristate buffer pull-up/pull-down via the ADC
4. Detection algorithm for known static networks, including R = 0 and open-circuit edge cases
5. Accuracy and linearity, by sweeping a precision resistor against the measured ratio
6. End-to-end latency, from input resistance change to the value arriving at the USB host

Then mark each done, done differently, or dropped with a reason. Item 3's "tristate buffer" is
now the analog switches, so that line needs restating rather than dropping: the self-test that
exists does exactly the described job on the new parts. **[V]** `src/bin/detect_pin_mapping.rs`.

**Takeaway.** The evaluation was planned in advance and it was actually carried out. The advisor
set this plan; closing the loop on it explicitly is the single highest-value slide in the section.

**Notes.** This slide costs 30 seconds and buys credibility for everything after it. Be honest
about anything dropped: "dropped, because the self-test made it redundant" is a fine answer,
silence is not.

## 4.2 Bring-up and self-test: the board is wired as designed

**Content.** Results of the automated self-test in `src/bin/detect_pin_mapping.rs`, which
**[V]** does the following: checks each ADC's grounded input (AIN6) and reference input (AIN10),
then pulls every contact of every jack up and down, verifying against the `board::JACKS` table
that exactly the expected ADC input follows, including that the tip switch follows the tip while
no plug is inserted.

Present as a pass/fail matrix (4 jacks x 4 contacts x 2 polarities = 32 checks) plus the measured
value at each ADC's grounded and reference input. Both of those self-test channels are real
board features, not firmware conveniences: AIN6 is tied to `GND` and AIN10 to `+2.5V` on both
converters. **[V]** So the offset reading is a genuine measurement of the whole conversion path,
and the reference reading is a check of the rail the entire ratiometric argument rests on. Say
that: it makes this slide evidence rather than bookkeeping.

One honest footnote if anyone asks why the channel numbering looks odd: the second jack on each
chip reads its ring on AIN9, which the AD7718 also exposes as `REFIN2(+)`, which is why the
driver has to run in 10-channel mode. **[V]**

**Takeaway.** Wiring, switches, shift registers and both ADCs are verified by the device
itself, in a few seconds, repeatably. Everything later rests on this.

**Notes.** Mention that this test is a permanent part of the repository, not a one-off, and that
it is the first thing run after any change to the switch driver or the wiring table.

## 4.3 Signal chain quality: supply and SPI

**Content.** Two scope captures on one slide, each with the number that matters and each with its
test point named:

- **Rails (TP12 +5V, TP13 +3.3VA, TP14 +2.5V, TP15 VEXT).** AC-coupled ripple and noise at idle
  and with the LEDs and WiFi active. Note that these test points are on the regulator side of the
  solder jumpers, so they read the LDO output. **[V]** State the scope's noise floor honestly:
  the interesting numbers may sit under it, and the board's own ADC measures the part that
  actually matters (4.4).
- **SPI at 4 MHz (TP7 clock, TP8 MOSI, TP5 MISO, TP6/TP9 chip selects).** Edges, overshoot, and
  the chip-select guard. One thing to expect and explain rather than be surprised by: **every SPI
  and shift-register line carries a 47 Ohm series resistor at its driver** (R3-R6, R46-R49 at the
  MCU, R8/R10 on the ADCs' data outputs, R7/R9 on the ready lines). **[V]** So edges are
  deliberately slowed, and the measurement is a check that this was enough, not a hunt for
  ringing. Pair it with the firmware-side result already in hand: at 4 MHz with a 10 us guard,
  zero outlying codes in 300 readings, where the breadboard needed 50 us. **[V]**
  `docs/fast-tracking.md`.

**Takeaway.** The analog and digital plumbing is quiet enough and fast enough that the
remaining error is the ADC's own noise, not the board's.

**Notes.** Tap settling (M8) is left out of the talk and goes into the report. If someone asks
whether the firmware waits long enough after switching, answer with the backup slide on it.

## 4.4 ADC noise versus speed: why everything runs at 819 Hz

**Content.** The measured characterisation series across filter rates, already in
`docs/fast-tracking.md` **[V]**, turned into plots. Measured with a 10.85 kOhm pot pedal in
jack 1, tip as wiper, bipolar coding, calibrated at each rate; state those conditions on the
slide.

- Plot A: reading noise and output step versus filter word, log-log, with the predicted cube law
  drawn through it. The documented check is 0.97 mV continuous at word 3 against 12 uV at word
  13, and (13/3)^3 accounts for it. **[V]**
- Plot B: reading time versus filter word on the same x-axis, showing the trade-off directly.
  Mark the chosen operating point: **819 Hz, 4.36 ms per reading including the channel switch,
  0.39 mV reading sigma, 203 ppm position sigma.** **[V]**
- Plot C: the mains-hum comparison, 100 to 140 uV at 50 Hz at every rate, removable by a 20 ms
  moving average, and irrelevant at about 60 ppm of travel. **[V]**
- One line each on the two findings that constrained the design, both documented: continuous
  conversion is 2.5 to 3 times noisier than single conversions at fast rates and therefore not
  worth a separate code path; and calibration does not carry over between filter words (ground
  reads -2.9 to +1.75 mV across words 3 to 13, and running at word 3 on a word-13 calibration
  shifts the gain by -0.18 %), which is why the firmware uses one word throughout. **[V]**

The firmware runs at 819 Hz and assumes a single-reading noise of 0.5 mV. **[V]**
`ADC_UPDATE_RATE = 819`, `ADC_VOLTAGE_NOISE = 0.0005` in `src/bin/expression_controller.rs`. That
is slightly conservative against the measured 0.39 mV, which is worth one clause.

**Takeaway.** The operating point was chosen from measurements, not guessed: 819 Hz reads
2.3 times faster than the original 315 Hz while keeping the noise the solver's tolerances assume.

**Notes.** This data already exists. The work left is turning three table columns into three
plots. Highest value per hour of anything in this section.

## 4.5 Accuracy: solving known resistor networks

**Content.** The measurement that most directly tests the sensor, and the one the milestone-1
plan promised as items 4 and 5. Feed a jack from precision resistors instead of a pedal and
compare the solved network against the known one:

- Plot A: measured arm resistance against actual, over the useful range, log-log, with relative
  error in a panel below. Sweep one arm while the others stay fixed.
- Plot B: relative error of the **total** resistance against total resistance, for totals from
  about 1 kOhm to 500 kOhm. This experimentally tests the pull-resistor analysis already in
  `docs/fast-tracking.md` **[V]**: with 1 kOhm pulls, the drop that carries the current
  information falls from 208 mV at a 10 kOhm pedal to 24 mV at 100 kOhm and 5 mV at 500 kOhm, so
  the total's error should rise visibly at the top end while position accuracy stays flat. Draw
  the predicted curve next to the measured points. The documented claim to test is that with
  1 kOhm pulls and 0.4 mV noise, every pedal up to about 500 kOhm still resolves. **[V]**
- Table: edge cases. One arm at 0 Ohm (each of the three in turn), one arm open, all three
  shorted, nothing connected. The solver has explicit code paths for the all-shorted and
  two-shorted cases **[V]** (`topology/src/resolve.rs`, `from_shorted_pairs`), so these are
  testing real branches, not hypotheticals.
- Table: pinout coverage. Both common wirings, the wiper on each contact, plus a deliberately
  unusual wiring, with the classification the device reported.

**Takeaway.** The solver is accurate where accuracy matters, its degradation at very high total
resistance is predicted rather than discovered, and it classifies every edge case correctly.

**Notes.** **[TODO]** confirm you have precision resistors or a decade box to hand. This
measurement is the backbone of the section; if the parts are not available, that changes what
section 4 can claim and is worth knowing now rather than the night before.

## 4.6 Linearity and output resolution: what the host actually receives

**Content.** Two claims, two plots:

- **Linearity.** Sweep a precision divider through known ratios to synthesise a pedal at exactly
  known positions, and plot reported position against true ratio, with the residual in ppm
  underneath. This separates the device's linearity from any real pedal's own track error, which
  is the only way to get a defensible number. Then one real-pedal sweep plotted against time, to
  show monotonicity, no dropouts, and endpoint repeatability over several sweeps.
- **Resolution.** Hold a pedal still, log a few thousand positions, plot the histogram with one
  MIDI step drawn on the same axis for scale, and show the emitted MIDI value as a second panel.
  The documented figures: position sigma **274 ppm** after the conditioning fix, against
  **7874 ppm** per MIDI step (1/127), so jitter is about **1/29 of a step** and the emitted value
  should be constant. **[V]** both numbers from `docs/fast-tracking.md`.

**Takeaway.** The output is quieter than its own quantisation. The limiting factor for the user
is the MIDI protocol's 7 bits, not this device.

**Notes.** This is the most quotable result in the talk. Give it its own headline number. A flat
MIDI trace is the whole slide.

## 4.7 Speed: update rate, recognition time, and end-to-end latency

**Content.** The headline improvement of the project, three measurements:

- **Update rate against number of tracked pedals**, as a bar chart. Documented: **188 to 189 per
  second** for one pedal alone on its ADC after the SPI trim (168 before), and **about 95** when
  two jacks share an ADC or all four are tracked. **[V]** Add the original full-sweep figure,
  **2.6 per second** from a 382 ms sweep, as the reference bar. **[V]** Draw the 50 Hz working
  target as a line; note that 50 Hz is the threshold `docs/fast-tracking.md` works to, so present
  it as the project's own target rather than an external spec unless your proposal said
  otherwise. **[?]**
- **Recognition time.** Documented as **under about 0.1 s** to go from plug-in to tracking
  (a 50 ms plug poll plus about 45 ms to identify), and similarly fast to notice an unplug.
  **[V]** Measure it on the scope by probing one jack contact: the drive pattern changes as the
  state machine moves from plug detection through identification into following, so one annotated
  capture shows the phases and their durations. The relevant intervals are all real constants you
  can label on the capture: 50 ms plug check, 25 ms end-tap check, 100 ms ring check, 1000 ms
  open solve. **[V]** `topology/src/monitor.rs`.
- **End-to-end latency**, the last open item from the milestone-1 plan. Split it honestly into the
  part the scope can see and the part it cannot. Toggle a resistance step with a switch, scope
  that against a spare GPIO toggled when the firmware hands the packet to USB: that is
  sensor-to-packet. Then measure packet-to-host separately in software, with a timestamping MIDI
  monitor on the host or the existing `midi_loopback` firmware for a round trip. **[V]** that
  `midi_loopback` exists. Report both terms and the sum, and say which dominates.

  On the spare GPIO: **GPIO0-5, 7-10, 12 and 26-28 are unconnected in the netlist** **[V]**, so
  there is a free pin in the design. **[?]** whether any of them is physically probe-able once
  the Pico module is soldered down. Check that before planning around it. If none is reachable,
  the fallback is to scope the SPI chip select or `SR_LAT` at TP9 or TP2 as a proxy for firmware
  activity, which is less direct but needs no hardware change.

**Takeaway.** Fast enough to feel instant, by a wide margin, and the latency budget is
understood term by term rather than quoted as a single mystery number.

## 4.8 Robustness and honest limits

**Content.** The slide that makes the rest believable. Three parts:

- **Correctness matrix** over pedal type and cable: potentiometer with wiper on tip and on ring,
  sustain switch normally open and normally closed, two-wire rheostat, dual footswitch, and
  nothing at the far end, each on a mono and a stereo cable. Report what the device classified
  and whether that is right. Then a stress result: N plug and unplug cycles per type counting
  misdetections, plus a wiggle-during-insertion test, since contact bounce while sliding a plug
  in is the realistic failure mode. **[TODO]** confirm which pedal types you physically have; the
  matrix is only as good as the hardware on the table.
- **Known limits**, all documented in `README.md` and `docs/fast-tracking.md` **[V]**, stated
  plainly: a potentiometer pedal needs a stereo cable, because most pedals short their ring to
  the sleeve with a mono plug, which puts the two track halves in parallel, giving x(1-x) of the
  track at travel x, so position is genuinely unrecoverable. A rheostat on a mono cable first
  reads as a potentiometer at rest until its resistance has changed by about a fifth. A mono
  cable with nothing at the far end reads as a released switch. One ambiguous end-stop case
  (ring and sleeve shorted) needs the per-jack wiper setting; `Auto` handles the common wirings.
- **Two architecture-level measurements** worth adding if time allows:
  - **Crosstalk.** Four pedals plugged in, sweep one, record how far the others move. Note the
    corrected mechanism: the pull resistors are **per jack**, so the coupling paths are the
    shared 2.5 V rail (which is both the pull rail and the ADC reference, so loading it moves
    both), the shared ground, the shared 3.3 V rail, and the ADC multiplexer shared by two jacks.
    **[V]** Loading the 2.5 V rail is the interesting one, since a closed switch or a low-value
    rheostat draws the most current through a pull resistor.
  - **Warm-up drift.** A stationary pedal's position over 30 minutes from cold. The ratiometric
    design predicts a nearly flat line, and since the 2.5 V rail is an LDO rather than a
    reference, this is the measurement that shows the ratiometric cancellation actually working.
    **[V]** on the design, **[?]** on the result.

**Takeaway.** The limits are known, explained by the physics, and documented rather than
discovered by a user. Every one of them has a stated reason.

**Notes.** Presenting the mono-cable limit as a solved analysis rather than a bug is accurate and
also the strongest possible framing: the ambiguity is in the pedal's own switching jack, not in
this device.

## 4.9 Requirements scorecard

**Content.** The requirements from slide 1.3, repeated verbatim, each with the measured number
that settles it and a plain met or partly met. One row per requirement, one number per row, no
prose. **[TODO]** depends on 1.3 being replaced with your real requirement list.

**Takeaway.** The promises are kept, and the evidence for each was just shown.

---

# Section 5: Problems and solutions (3 slides, 2.5 min)

Pick problems that taught something, not every bug. The strongest ones are where a measurement
exposed a design flaw. Same shape every time: symptom, cause, fix, what it cost.

## 5.1 Hardware and bring-up

- **Pull impedance.** Tristate buffers could not provide a known pull resistance, so the frontend
  moved to analog switches with a precision resistor per rail. Cost: a schematic and layout redo
  mid-project (commit `f89b08d`). **[V]** the change and its timing; **[?]** the reasoning, state
  your own.
- **LED current budget.** The 5 V rail is a 200 mA LDO shared by the LEDs, the level shifter and
  the Pico's VSYS, so brightness is capped in firmware at 51/255. **[V]** the topology and the
  cap. **[?]** whether you found this by measuring current (commit `ffd5ff4` adds "power
  consumption measurements", which supports the story) or by calculation; say which.
- **USB.** Enumeration stalled because of a suspend event latched before a bus reset in
  embassy-rp 0.10, and a buffer overflow appeared on macOS. Fixes: clear the stale suspend, and
  pace bus events so a flood cannot starve the executor. **[V]** `src/hal/usb/driver.rs` and
  commit `c07f86d` "fixed USB overflow error on Mac OS".
- **SPI chip-select guard.** The breadboard needed 50 us and produced outlying ADC codes without
  it; the finished board runs 4 MHz with a 10 us guard and zero outliers in 300 readings. **[V]**

**Takeaway.** Most hardware problems here were found by instrumenting the board, and the board
was laid out with the test points that made instrumenting it possible.

## 5.2 Speed: from 2.6 to about 190 updates per second

The best story in the project. Tell it as a three-step diagnosis. All figures **[V]** from
`docs/fast-tracking.md`.

- **Symptom.** A full solve of all four jacks took 382 ms, so 2.6 updates per second per pedal.
  Unusable.
- **Diagnosis.** Every reading changes ADC channel, so the sinc-cubed filter settles from
  scratch: three conversion periods per reading, 10.2 ms at 315 Hz. Nine readings per jack (three
  pairs x three taps), four jacks. The cost is structural, not incidental: at a 50 Hz target,
  20 ms buys about two readings, so no full solve can fit.
- **Fix, in two independent parts.** Read faster: measurements chose 819 Hz over 315 Hz, 2.3
  times faster at noise the solver still tolerates. Read less: stop re-solving a pedal that is
  already identified. Once the wiring is known, drive the two track ends and read only the wiper,
  one reading per update, checking one track end every 25 ms to notice unplugging or a changed
  network. That is the whole reason the per-jack state machine exists.
- **Result.** 188 to 189 updates per second for one pedal, about 95 with jacks sharing an ADC, and
  recognition still within about 0.1 s.

**Takeaway.** The speed came from changing what is measured, not from faster hardware. State this
explicitly; it is the transferable lesson of the project.

## 5.3 When the solver was subtly wrong

Four cases where the maths was right and the situation was not. Each shows a measurement catching
something a functional test would have passed. All **[V]** from `docs/fast-tracking.md` and the
constants in `topology/src/`.

- **Ill-conditioned solve.** With the wiper on the tip, the two pairs sharing it are conditioned
  roughly t/((t+r)(t+s)), and the solver split the track through a few millivolts across the
  wiper, amplifying noise about 25-fold. Position sigma was 4880 ppm at 819 Hz where it should
  have been a few hundred. Fix: `min_ratio_conditioning = 0.5`, which forces a third measurement
  pair for such pedals. Result: 274 ppm, at about 15 ms more per sweep. Show the three-row
  before-and-after table (315 Hz/0.05: 2588 ppm, 382 ms; 819 Hz/0.05: 4880 ppm, 156 ms;
  819 Hz/0.5: 274 ppm, 171 ms). This is the clearest evidence in the talk that the evaluation
  found real defects, and note that going faster is what *exposed* it: the flaw was already
  costing accuracy at 315 Hz.
- **End-stop threshold.** The test pedal fully closed leaves 9 % of its track on the sleeve, which
  a 10 % end-stop threshold read as a shorted end stop, so the jack re-solved forever instead of
  tracking. Fix: a separate, tighter `end_stop_relative = 0.02`, kept well below the wiper's own
  10 % allowance.
- **Position convention.** Counting from the lower-numbered contact read the test pedal backwards,
  and at an ambiguous end stop the position could flip between 0 and 1. Fix: count from the sleeve
  whenever the sleeve is a track end, which matches both common wirings, and remember the wiper
  role for the jack until the plug comes out. That pedal now reads 0.09 closed and 1.0 open.
- **Plug insertion transient.** A stereo plug sliding in connects tip and sleeve through part of
  the track before the ring, so a potentiometer pedal briefly looked like a switch turning into a
  rheostat and stayed one. Fix: keep checking every 100 ms that the ring is still wired as
  identified, with a 1 % allowance on top of the noise because the two pull paths are not exactly
  equal.

**Takeaway.** Every one of these was invisible to a pass/fail functional test and visible in a
noise or repeatability measurement. That is the argument for the evaluation being the main part
of this talk.

---

# Section 6: Live demonstration (1 backdrop slide, about 5 min)

**Purpose.** Make the numbers physical. The audience should watch a pedal being recognised.

**Script**, building from the familiar to the specific:

1. Plug in a potentiometer pedal with a **stereo** cable. The LED lights in the jack colour and
   turns green while the pedal moves. **[V]** Show the MIDI monitor on the host reacting.
2. Show the web interface for that jack: the topology panel drawing the measured network with the
   wiper it found, and the plain-language explanation of what the jack is doing. This is where the
   measurement principle becomes concrete.
3. Unplug it live and plug a sustain pedal into the same jack, with no reset. Then a two-wire
   rheostat pedal, recognised with its resistance against the nearest standard value.
4. Plug a second pedal into another jack and move both at once. Note for yourself: jacks 1 and 2
   are on one ADC and 3 and 4 on the other **[V]**, so putting the two demo pedals in jacks 1 and
   3 keeps both at the full update rate, while jacks 1 and 2 share and run at about half. Choose
   deliberately, and mention it if you want to demonstrate the difference.
5. One settings change: set the range with the min and max buttons, or flip inversion, and show
   the MIDI output following.

**Takeaway.** It works, on stage, with pedals nobody prepared it for.

**Pre-flight checklist:**

- **External power.** The analog rails derive from the barrel jack, `VBUS` is unconnected, and the
  Schottky blocks back-feeding from the Pico. **[V]** So bring the barrel-jack supply; USB alone
  will very likely not run the frontend. **[?]** confirm on the bench, and pack a spare supply.
- USB cable, plus a spare. MIDI monitor already open and configured.
- WiFi access point joinable from the demo laptop. Note that a projector setup can fight the
  laptop's WiFi association; test it in the room if you can.
- All pedal types on the table, **and both a mono and a stereo cable**, clearly distinguishable
  from each other. If you demonstrate the mono-cable limitation deliberately, label the cables.
- Default controllers differ by jack: CC 11 on jacks 1 and 2, CC 1 on jacks 3 and 4. **[V]** Set
  the MIDI monitor up accordingly, or change the settings first, so nothing looks broken.
- LEDs visible from the room.

**Notes.** Plan about 3 minutes of content for a 5 minute slot; live demos overrun. Fallbacks in
order: a screen recording of the same script, then screenshots on the backdrop slide. Have the
recording cued, not "somewhere on the laptop". If the projector cannot show the web interface and
the MIDI monitor at once, put the web interface on screen and describe the MIDI side.

---

# Section 7: Summary and outlook (2 slides, 1.5 min)

## 7.1 Key takeaways and what I would do differently

**Content.** Three takeaways: an unknown passive network can be identified in place with nothing
but switchable pull resistors and a slow high-resolution ADC; the speed problem was solved by
measuring less rather than with faster hardware; and the accuracy claims rest on the frontend
being ratiometric to one rail.

What you would do differently, chosen to be specific and non-defensive. The first is fully
supported by the repository; the rest are judgement calls, so phrase them as yours:

- **Fit larger pull resistors, around 4.7 kOhm, if accurate absolute totals for 250 to 500 kOhm
  pedals ever matter.** The analysis in `docs/fast-tracking.md` names exactly that value and notes
  that the 1 kOhm choice is right for position and weak for high totals. **[V]** This is the
  strongest item because the repository already contains the reasoning.
- **Design the identify-then-track split in from the start** instead of retrofitting it. The
  one-shot-solve architecture was what made the firmware slow, and reworking it into a state
  machine was a large piece of work (commits `b4dc8e1`, `b757564`). **[V]** the sequence.
- **Build the host-testable solver crate first.** Once `expad-topology` existed the subtle solver
  bugs became reproducible in seconds; commit `922367b` moved the code into a library well after
  the solver was written. **[V]** the ordering.
- Optional, if you want a hardware one: the buttons from the milestone-1 outlook never made it
  onto the board, and the web interface took their place. **[V]** Whether that was the right call
  is your judgement.

## 7.2 Next steps

**Content.** Four concrete, ordered next steps, each with a reason:

- **Persist settings to flash**, so a configured device survives a power cycle. **[V]** nothing in
  the repository writes settings to flash today; `README.md`'s TODO list and the settings watch in
  `src/web/interface.rs` confirm it is runtime-only. The most obvious missing user feature.
- **Trim the remaining SPI overhead and schedule shared ADCs better.** About 0.35 ms of the
  4.36 ms reading is overhead beyond the three conversion periods. **[V]** Two jacks sharing an
  ADC currently alternate at about 95 updates each.
- **Case and panel**, so it can be used on stage.
- **Higher-resolution output.** The measurement is roughly 29 times finer than a 7-bit MIDI step
  **[V]**, and that resolution is thrown away at the output today. High-resolution CC pairs
  (MSB/LSB) are the standard way to use it, and MIDI 2.0 the longer-term one. **[?]** do not claim
  a specific implementation plan; present it as the obvious next gain. Best note to end on.

**Takeaway.** The project is finished as specified, the remaining work is known and scoped, and
the device is already more precise than its own output format.

---

# Backup slides

Prepare these but do not present them:

- The full jack state machine diagram (`docs/diagrams/jack-monitor-states.pdf`), for "how does it
  know it was unplugged?".
- The pull resistor analysis table, for "why 1 kOhm?".
- **The power and grounding slide**, for "why one ground net and no split?". Given that the
  milestone-1 slide said otherwise, expect this question. **[TODO]** write the one-sentence
  rationale; it is not in the repository.
- The solver arithmetic: how two voltage ratios through a shared arm determine three arms, and
  what conditioning means.
- The full ADC characterisation table.
- The mono-cable parallel-track-halves analysis with the x(1-x) curve plotted.
- The host test suite (50 tests), for "how do you know the solver is right?". **[V]**
- Tap settling, for "does the firmware wait long enough after switching?". The plug-in capture
  `RigolDS1.csv` (in `measurements/raw_data.zip`) shows the floating ring of a ~9.4 kOhm pedal
  charging with a time constant of 190.6 us after the tip-sleeve pair's latch, against
  (9.4 + 10 kOhm) x 10 nF = 194 us predicted; the 1 ms minimum wait is 5.2 time constants. **[V]**
  The full series with known resistors is in the report (M8).
- Bill of materials and cost. **[TODO]** `hardware/ExpressionController.xlsx` exists and was not
  opened for this scaffold; check whether it already has costs.

---

# Measurement program to run before the presentation

The board is functionally working and the ADC characterisation already exists. What is missing is
the evaluation series. Prioritised below by presentable result per hour of lab time.

**Equipment assumed:** a standard oscilloscope, precision resistors or a decade box **[TODO]**
confirm availability, a multimeter, the pedals on hand, and the board's own AD7718s, which are by
some margin the most precise instrument in this list and should be used as one. **[V]** Every
electrical probe point named below is an actual test point on the board.

## Must have

**M1. Plots from the existing ADC characterisation.** No lab time: the data is in
`docs/fast-tracking.md`, produced by `src/bin/adc_characterization.rs`. **[V]** Produce the
noise-versus-filter-word log-log plot with the cube law overlaid, the reading-time plot with the
operating point marked, and the mains-hum comparison. Feeds 4.4. Do this first, it is free.

**M2. Solver accuracy against known resistors.** A TRS breakout fed from precision resistors or a
decade box. Sweep one arm over decades with the others fixed; then repeat at several totals from
about 1 kOhm to 500 kOhm. Deliver the measured-versus-actual plot with a relative-error panel, and
the error-versus-total plot with the predicted curve from the pull-resistor analysis through it.
Run the edge cases: each arm at 0 Ohm in turn, one arm open, all shorted, nothing connected. Feeds
4.5. **The single most important missing measurement**, because it is what the milestone plan
promised and it tests the sensor rather than the product.

**M3. Position linearity and output noise.** Synthesise known positions with a precision divider,
sweep, plot reported position against true ratio with a ppm residual panel. Separately hold a real
pedal still, log a few thousand positions, plot the histogram with one MIDI step drawn for scale,
plus the emitted MIDI value as a flat trace. Feeds 4.6, and produces the most quotable number in
the talk. Needs no external instrument.

**M4. Update rate versus pedal count, and recognition time.** The firmware already logs position
updates per second every 5 s **[V]**, so the bar chart is a matter of logging with one, two, three
and four pedals tracked. Put the two pedals in jacks 1 and 3 for the independent case and jacks 1
and 2 for the shared-ADC case **[V]**, and say which is which. For recognition time, probe a jack
contact and capture a plug insertion: the drive pattern changes as the state machine advances, so
one annotated capture shows the phases. Feeds 4.7. Cheap and visually excellent.

**M5. End-to-end latency.** Add a GPIO toggle when a MIDI packet is handed to USB; scope channel 1
on a switched resistance step, channel 2 on that GPIO. Then measure packet-to-host separately with
a timestamping MIDI monitor or `midi_loopback` for a round trip. Report both terms and the sum.
Feeds 4.7 and closes the last open milestone-1 item. **[?]** Check first whether a free Pico pin is
physically reachable (GPIO0-5, 7-10, 12, 26-28 are unconnected in the netlist **[V]**, but that
says nothing about probe access after assembly). Fallback: scope `SR_LAT` at TP2 or a chip select
at TP6/TP9 as a firmware-activity proxy.

**M6. Classification correctness matrix and plug-cycle stress.** Every pedal type you have,
crossed with a mono and a stereo cable; record what was classified. Then N plug and unplug cycles
per type counting misdetections, plus a wiggle-during-insertion test. Feeds 4.8. No instruments,
only patience, and it is the evidence that the device is usable rather than merely correct.

## Should have

**M7. Supply and SPI captures.** Rails at TP12 (+5V), TP13 (+3.3VA), TP14 (+2.5V) and TP15 (VEXT),
AC-coupled, at idle and with LEDs and WiFi active, with the scope's bandwidth limit on and its
noise floor stated. SPI at TP7 (clock), TP8 (MOSI), TP5 (MISO), TP6/TP9 (chip selects). **[V]** all
test points. Expect deliberately slowed edges: every line has a 47 Ohm series resistor. **[V]**
Feeds 4.3, and closes two more milestone-1 items. Optional extra, cheap because the board supports
it: open JP1, JP2 or JP3 to measure each rail's current separately. **[V]**

## Nice to have

**M9. Crosstalk between jacks.** Four pedals plugged in; sweep one, record the other three. Note
the corrected mechanism: pull resistors are per jack **[V]**, so the coupling paths are the shared
2.5 V rail (both pull rail and ADC reference), the shared ground and 3.3 V rail, and the ADC shared
by two jacks. The worst case to try is a closed switch or a near-zero rheostat in one jack, which
draws the most current through a pull resistor, while a potentiometer is tracked in another. Feeds
4.8.

**M10. Warm-up drift.** A stationary pedal's position over 30 minutes from cold. Since the 2.5 V
rail is an LDO rather than a precision reference **[V]**, this is the measurement that shows the
ratiometric cancellation working, so it is more interesting here than on a board with a real
reference. Feeds 4.8.

**M11. Long cable.** Repeat recognition and tracking with a long instrument cable, since tap
capacitance sets the settle delay and cable capacitance adds to the 10 nF the model assumes.
**[V]** Feeds 4.8 if it shows anything, backup if not.

## Deferred to the report

**M8. Tap settling capture.** Not part of the talk; it goes into the report. It validates the
firmware's settle model, `2 x (network + 10 kOhm) x 10 nF` clamped to 1-400 ms and assuming
100 kOhm until a solve has measured the network, against the measured curve. **[V]**
`SettleConfig`, `MonitorConfig::initial_total`.

- **First attempt, not usable.** `settling_4k7.csv`, `settling_47k.csv` and `settling_220k.csv`
  (4.7, 47 and 220 kOhm between ring and tip and between tip and sleeve, wiper on the tip) were
  triggered on the ring rising through 1 V, which caught the plug sliding in rather than a switch
  change: the ring sits at the 1.25 V plug-check divider for 0.64-0.78 ms whatever the resistor,
  and SR_LAT stays low throughout, since an empty jack keeps its drives. **[V]** in
  `measurements/raw_data.zip`.
- **Redo.** CH1 on SR_LAT (TP2), trigger rising at 1.65 V, single; CH2 on J2.R; the other jacks
  empty, so the first latch is jack 1's. Plug in within 30 s of the previous unplug, so the rails
  are fresh and that latch is the tip-ring pair. 5 ms/div with the trigger at 10 %, at least 500k
  points. The step to fit is the tip-sleeve pair's latch 15-19 ms later, where the floating ring
  charges through R.
- **Prediction to test.** Time constant about (11 kOhm + R) x 10 nF: 156 us, 590 us and 2.3 ms.
  While identifying, the firmware waits 2.2 ms and reads the floating ring last, about 10.5 ms
  after the latch, so 220 kOhm is the case that tests the model: about 1 % of the step, some
  27 mV against 0.5 mV of noise, would still be missing. At 220 kOhm the 10X probe's 10 MOhm pulls
  the floating ring about 2 % low; correct for it.

## What is deliberately not measured, and why

Say this out loud in 4.1 rather than leaving gaps:

- **Absolute mechanical position accuracy of a real pedal.** Without a calibrated mechanical rig,
  the number would be dominated by the pedal's own track nonlinearity, not the device. M3's
  precision-divider sweep measures the device instead, which is the defensible choice.
- **Supply noise below the scope's floor.** A standard scope cannot reach it. Where it matters, the
  board's own ADC measures the part that reaches the conversion, which M1 already reports, and the
  grounded and reference self-test channels (4.2) give a continuous check. **[V]**
- **Temperature and long-term stability beyond warm-up.** Out of scope, and there is no climate
  chamber.
