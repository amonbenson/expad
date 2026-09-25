// Explains a jack's live status in words: what the firmware is doing with it, why it decided on
// that, and what it is waiting for. Every threshold and interval mirrors `MonitorConfig::new` in
// topology/src/monitor.rs and the board's pulls in src/board.rs.

import type { ArmPull, JackStatus } from "@/interface";

/** A piece of a sentence: plain text, or a live value to highlight. */
export type Segment = string | { value: string };

export interface NarrativeLine {
  /** What the line explains: the current activity, the reason for it, or what comes next. */
  label: "Now" | "Why" | "Next";
  segments: Segment[];
}

const HIGH_RAIL_VOLTAGE = 2.5;
const PULL_RESISTANCE = 1;
/** A plug check's tip switch reading above this fraction of the rail means a plug. */
const PLUG_THRESHOLD_FRACTION = 0.75;
/** Largest tip-sleeve resistance that counts as a closed switch, in kΩ. */
const CLOSED_RESISTANCE = 0.2;
/** Relative arm resistance up to which two arms both touch the star point: an end stop. */
const END_STOP_RELATIVE = 0.02;

const PLUG_CHECK_INTERVAL_MS = 50;
const RING_CHECK_INTERVAL_MS = 100;
const END_TAP_INTERVAL_MS = 25;
const OPEN_SOLVE_INTERVAL_MS = 1000;
const RAIL_REFRESH_INTERVAL_S = 30;
const RHEOSTAT_READINGS = 3;

const TIP = 0;
const RING = 1;
const SLEEVE = 2;
const TIP_SWITCH = 3;
const CONTACT_NAMES = ["tip", "ring", "sleeve", "tip switch"];

/** Sentence pieces with every interpolated value highlighted. */
function say(strings: TemplateStringsArray, ...values: string[]): Segment[] {
  return strings.flatMap((text, index) =>
    index < values.length ? [text, { value: values[index] ?? "" }] : [text],
  );
}

function volts(voltage: number | null | undefined): string {
  return voltage == null ? "? V" : `${voltage.toFixed(2)} V`;
}

function kiloOhms(resistance: number | null | undefined): string {
  return resistance == null ? "? kΩ" : `${resistance.toFixed(1)} kΩ`;
}

function percent(fraction: number): string {
  return `${Math.round(fraction * 100)} %`;
}

/** Absolute resistance of an arm in kΩ, `null` while it is isolated or unresolved. */
function armResistance(status: JackStatus, arm: number): number | null {
  const relative = status.resistances.relative[arm];
  const total = status.resistances.total;
  return relative == null || total == null ? null : relative * total;
}

/** The arm driven to `pull`, `undefined` if none or several are. */
function armPulled(status: JackStatus, pull: ArmPull): number | undefined {
  const arms = [TIP, RING, SLEEVE].filter(arm => status.pulls[arm] === pull);
  return arms.length === 1 ? arms[0] : undefined;
}

/** Behind a mono plug the sleeve shorts the ring to the star point; behind a stereo one it stays isolated. */
function monoPlug(status: JackStatus): boolean {
  return status.resistances.relative[RING] === 0;
}

function describeEmpty(status: JackStatus): NarrativeLine[] {
  const threshold = PLUG_THRESHOLD_FRACTION * HIGH_RAIL_VOLTAGE;

  return [
    {
      label: "Now",
      segments: say`Waiting for a plug. Every ${`${PLUG_CHECK_INTERVAL_MS} ms`} the tip is pulled down, the tip switch up, and the tip switch is read: ${volts(status.voltages[TIP_SWITCH])}.`,
    },
    {
      label: "Why",
      segments: say`Without a plug the tip switch rests on the tip, so the two ${`${PULL_RESISTANCE} kΩ`} pulls split the rail in half: ${volts(HIGH_RAIL_VOLTAGE / 2)}.`,
    },
    {
      label: "Next",
      segments: say`A plug lifts the tip switch off the tip, leaving it to its own pull-up at ${volts(HIGH_RAIL_VOLTAGE)}. Anything above ${volts(threshold)}, three quarters of the rail, counts as a plug and starts identifying what is behind it.`,
    },
  ];
}

function describeRails(status: JackStatus): NarrativeLine[] {
  const rail = status.pulls[TIP] === "up" ? "high" : "low";

  return [
    {
      label: "Now",
      segments: say`Measuring the ${rail} rail: tip, ring and sleeve are all pulled ${status.pulls[TIP] ?? ""} together, so no current flows and every tap reads the rail itself, like the tip at ${volts(status.voltages[TIP])}.`,
    },
    {
      label: "Why",
      segments: say`Every solve compares the tap voltages against both rails, so they are measured at start-up and again before a solve once they are ${`${RAIL_REFRESH_INTERVAL_S} s`} old.`,
    },
    {
      label: "Next",
      segments: say`With both rails known, the network behind the plug is solved.`,
    },
  ];
}

function describeIdentifying(status: JackStatus): NarrativeLine[] {
  const high = armPulled(status, "up");
  const low = armPulled(status, "down");
  const floating = armPulled(status, "floating");
  if (high === undefined || low === undefined || floating === undefined) {
    return describeRails(status);
  }

  return [
    {
      label: "Now",
      segments: say`Identifying: the ${CONTACT_NAMES[high] ?? ""} is pulled up, the ${CONTACT_NAMES[low] ?? ""} down and the ${CONTACT_NAMES[floating] ?? ""} left floating. No current flows through its arm, so it reads the star point: ${volts(status.voltages[floating])}.`,
    },
    {
      label: "Why",
      segments: say`Where the star point sits between ${volts(status.voltages[high])} and ${volts(status.voltages[low])} gives the ratio of the two driven arms, and the current through the pulls gives their sum. Up to three such pairs resolve all three arms.`,
    },
    {
      label: "Next",
      segments: say`One arm at the star point with a track across the other two makes an expression pedal, a single element between tip and sleeve a switch or rheostat, and no current anywhere an open plug.`,
    },
  ];
}

function describeTracking(status: JackStatus): NarrativeLine[] {
  const wiper = armPulled(status, "floating");
  const high = armPulled(status, "up");
  const low = armPulled(status, "down");
  if (wiper === undefined || high === undefined || low === undefined) {
    return describeIdentifying(status);
  }

  const [wiperName, highName, lowName] = [wiper, high, low].map(arm => CONTACT_NAMES[arm] ?? "");
  const lowVoltage = status.voltages[low] ?? null;
  const highVoltage = status.voltages[high] ?? null;
  const position = status.position ?? 0;
  const endDrop = lowVoltage === null ? null : lowVoltage;

  const endStopArm = [low, high].find(
    arm => (status.resistances.relative[arm] ?? Infinity) <= END_STOP_RELATIVE,
  );
  const why: Segment[] = endStopArm === undefined
    ? say`The ${wiperName}'s arm is ${kiloOhms(armResistance(status, wiper))}, next to nothing, so the ${wiperName} sits at the star point: a wiper. The ${lowName} and ${highName} carry the track, ${kiloOhms(status.resistances.total)} end to end - together a potentiometer, an expression pedal.`
    : say`The pedal rests on an end stop, where the ${wiperName} and the ${CONTACT_NAMES[endStopArm] ?? ""} both touch the star point. Either could be the wiper; until the pedal moves, the Wiper setting or the last pedal seen in this jack decides.`;

  return [
    {
      label: "Now",
      segments: say`Following an expression pedal: the track ends are driven, the ${highName} up and the ${lowName} down, and only the wiper on the ${wiperName} is read. At ${volts(status.voltages[wiper])} it lies ${percent(position)} of the way from ${volts(lowVoltage)} to ${volts(highVoltage)}; the ends sit ${volts(endDrop)} inside the rails, dropped across the pulls by the track current.`,
    },
    { label: "Why", segments: why },
    {
      label: "Next",
      segments: say`The ${wiperName} rising toward the ${highName}'s voltage moves the position up. Every ${`${END_TAP_INTERVAL_MS} ms`} a track end is read again: if its drop vanishes or shifts, the pedal was unplugged, rewired or its wiper misjudged, and it is identified again.`,
    },
  ];
}

function describePlugChecks(mono: boolean): Segment[] {
  return mono
    ? say`every ${`${PLUG_CHECK_INTERVAL_MS} ms`} the tip switch checks the plug is still in, and every ${`${RING_CHECK_INTERVAL_MS} ms`} the ring that it still matches the sleeve`
    : say`every ${`${PLUG_CHECK_INTERVAL_MS} ms`} the tip switch checks the plug is still in`;
}

function describeSwitch(status: JackStatus): NarrativeLine[] {
  const mono = monoPlug(status);
  const pressed = status.position === 1;
  const tipVoltage = volts(status.voltages[TIP]);
  const plug = mono
    ? say`and the ring reads like the sleeve: a mono plug, whose sleeve shorts the jack's ring to it.`
    : say`and the ring was isolated: a stereo plug with nothing on its ring.`;

  return [
    {
      label: "Now",
      segments: [
        ...say`Following a switch: the tip pulled up, the sleeve down, only the tip read. `,
        ...(pressed
          ? say`It is closed: the tip dropped to ${tipVoltage}, where the two pulls split the rail, so its position is ${percent(1)}.`
          : say`It is open: no current flows, so the tip stays at the rail, ${tipVoltage}, and its position is ${percent(0)}.`),
      ],
    },
    {
      label: "Why",
      segments: [
        ...say`Identifying found a single element between tip and sleeve, `,
        ...plug,
        ...say` Every reading was either closed, below ${kiloOhms(CLOSED_RESISTANCE)}, or open - never in between - so the element is a switch.`,
      ],
    },
    {
      label: "Next",
      segments: [
        ...say`The tip falling toward ${volts(HIGH_RAIL_VOLTAGE / 2)} means pressed, rising to ${volts(HIGH_RAIL_VOLTAGE)} released; ${`${RHEOSTAT_READINGS}`} readings in a row in between would make it a rheostat. Meanwhile, `,
        ...describePlugChecks(mono),
        ".",
      ],
    },
  ];
}

function describeRheostat(status: JackStatus): NarrativeLine[] {
  const mono = monoPlug(status);
  const resistance = status.resistances.total;
  const position = status.position ?? 0;
  const tipVoltage = status.voltages[TIP] ?? null;
  const fullScale = resistance !== null && position > 0.01 ? resistance / position : null;
  const drop = tipVoltage === null ? null : HIGH_RAIL_VOLTAGE - tipVoltage;

  return [
    {
      label: "Now",
      segments: [
        ...say`Following a rheostat: the tip pulled up, the sleeve down, only the tip read. The tip sits ${volts(drop)} below the rail, the current through its pull-up, which puts ${kiloOhms(resistance)} between tip and sleeve`,
        ...(fullScale === null ? say`.` : say` - ${percent(position)} of its ${kiloOhms(fullScale)} full scale.`),
      ],
    },
    {
      label: "Why",
      segments: mono
        ? say`Tip and sleeve are joined by a resistance that changes with the pedal: a two-wire expression pedal on a mono plug. It first looked like a potentiometer on its end stop, until its total changed by more than a fifth.`
        : say`The resistance between tip and sleeve read between closed and open ${`${RHEOSTAT_READINGS}`} times in a row, so it is a variable resistor - a two-wire expression pedal - rather than a switch caught mid-bounce.`,
    },
    {
      label: "Next",
      segments: [
        ...say`The tip rising toward ${volts(HIGH_RAIL_VOLTAGE)} means more resistance and a higher position; reading beyond the full scale moves it to the next standard value. Meanwhile, `,
        ...describePlugChecks(mono),
        ".",
      ],
    },
  ];
}

function describeOpen(status: JackStatus): NarrativeLine[] {
  return [
    {
      label: "Now",
      segments: say`A plug is in - the tip switch reads ${volts(status.voltages[TIP_SWITCH])} - but nothing conducts: with the tip pulled up and the sleeve down, the tip stays at the rail, ${volts(status.voltages[TIP])}, as no current crosses its pull-up.`,
    },
    {
      label: "Why",
      segments: say`A full solve found no current between any two contacts: a cable with nothing at its end, or a released stereo sustain pedal, which keeps tip and sleeve apart.`,
    },
    {
      label: "Next",
      segments: say`Any drop of the tip below the rail means tip and sleeve connected, and the jack is identified again; a full solve every ${`${OPEN_SOLVE_INTERVAL_MS / 1000} s`} catches networks elsewhere. The tip switch still checks for the plug every ${`${PLUG_CHECK_INTERVAL_MS} ms`}.`,
    },
  ];
}

function describeOther(status: JackStatus): NarrativeLine[] {
  const [tip, ring, sleeve] = [TIP, RING, SLEEVE].map(arm => kiloOhms(armResistance(status, arm)));

  return [
    {
      label: "Now",
      segments: say`Solving the network in full, again and again: tip ${tip ?? ""}, ring ${ring ?? ""}, sleeve ${sleeve ?? ""} to the star point. No MIDI is sent.`,
    },
    {
      label: "Why",
      segments: say`It is neither a potentiometer - one arm at the star point with the track across the other two - nor a single element between tip and sleeve; a dual footswitch looks like this.`,
    },
    {
      label: "Next",
      segments: say`As soon as a solve shows a pedal the adapter knows, it is followed.`,
    },
  ];
}

/** Explains `status` in up to three lines; a single line while nothing is known. */
export function describeJack(status: JackStatus | undefined): NarrativeLine[] {
  if (status === undefined) {
    return [{ label: "Now", segments: say`Waiting for the device to report.` }];
  }

  const allArmsAlike = status.pulls[TIP] !== "floating"
    && status.pulls[RING] === status.pulls[TIP]
    && status.pulls[SLEEVE] === status.pulls[TIP];
  if (allArmsAlike && (status.mode === "identifying" || status.mode === "empty")) {
    return describeRails(status);
  }

  switch (status.mode) {
    case "empty": return describeEmpty(status);
    case "identifying": return describeIdentifying(status);
    case "tracking": return describeTracking(status);
    case "switch": return describeSwitch(status);
    case "rheostat": return describeRheostat(status);
    case "open": return describeOpen(status);
    case "other": return describeOther(status);
  }
}
