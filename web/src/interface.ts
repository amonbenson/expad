// Mirrors the JSON protocol of src/web/interface.rs. Non-finite floats are sent as `null`.

export interface ArmResistances {
  /** Relative resistance of each arm, summing to 1. */
  relative: [number | null, number | null, number | null];
  /** Total resistance in kΩ. */
  total: number | null;
}

/** Rail an arm is driven to while it is measured. */
export type ArmPull = "up" | "down" | "floating";

/**
 * What the firmware makes of a jack: `identifying` runs full solves (just plugged in, or tracking
 * lost the pedal), `tracking` follows a potentiometer's wiper, `switch` and `rheostat` follow
 * what connects tip and sleeve (sustain pedals, two-wire expression pedals, pedals on a mono
 * cable), `other` is any other network and `open` a plug with nothing conducting behind it.
 */
export type JackMode = "empty" | "identifying" | "tracking" | "switch" | "rheostat" | "other" | "open";

/** Contact a potentiometer's wiper is on, deciding the one ambiguous end stop (ring and sleeve shorted). */
export type WiperContact = "auto" | "tip" | "ring" | "sleeve";

export interface JackStatus {
  mode: JackMode;
  /** Wiper position in 0..1 as measured, before range and inversion; `null` without a potentiometer. */
  position: number | null;
  /** Expression value in 0..1 that is sent to the host. */
  value: number;
  resistances: ArmResistances;
  /** Voltage last measured at each contact's tap - tip, ring, sleeve, tip switch - in V. */
  voltages: [number | null, number | null, number | null, number | null];
  /** Rail each contact is switched to while the jack is measured. */
  pulls: [ArmPull, ArmPull, ArmPull, ArmPull];
}

export interface Status {
  jacks: JackStatus[];
}

export interface JackSettings {
  /** Zero-based MIDI channel. */
  midiChannel: number;
  midiController: number;
  inverted: boolean;
  /** Wiper positions in 0..1 the pedal's travel starts and ends at, stretched to the full expression range. */
  minimum: number;
  maximum: number;
  wiper: WiperContact;
  /**
   * Bend of the response curve in -1..1 against a pedal's nonlinear track: 0 is linear, positive
   * values make the value rise early in the travel and negative ones late (see `driveCurve`).
   */
  drive: number;
  /** Accent color of the jack as `#RRGGBB`, shown by its LED and throughout the interface. */
  color: string;
}

export interface Settings {
  ledBrightness: number;
  jacks: JackSettings[];
}

export type Update = { status: Status } | { settings: Settings };

/** How far the drive curve bends at full drive, mirroring `MAX_DRIVE_BEND` in src/web/interface.rs. */
const MAX_DRIVE_BEND = 0.9;

/**
 * Mirrors `drive_curve` in src/web/interface.rs: bends `travel` in 0..1 by `drive` in -1..1,
 * keeping both ends in place and moving half travel to `(1 + bend) / 2`.
 */
export function driveCurve(travel: number, drive: number): number {
  const bend = Math.min(Math.max(drive, -1), 1) * MAX_DRIVE_BEND;
  const clamped = Math.min(Math.max(travel, 0), 1);

  return ((1 + bend) * clamped) / (1 - bend + 2 * bend * clamped);
}

/**
 * Mirrors `JackSettings::value` in src/web/interface.rs up to the drive curve: where a wiper
 * `position` is in the pedal's travel, the jack's range stretched to 0..1 and inverted if it is.
 */
export function pedalTravel(position: number, jackSettings: JackSettings): number {
  const range = jackSettings.maximum - jackSettings.minimum;
  const stretched = range > 0 ? (position - jackSettings.minimum) / range : position;
  const value = Math.min(Math.max(stretched, 0), 1);

  return jackSettings.inverted ? 1 - value : value;
}

/** Mirrors `JackSettings::value` in src/web/interface.rs: the expression value sent for a wiper `position`. */
export function expressionValue(position: number, jackSettings: JackSettings): number {
  return driveCurve(pedalTravel(position, jackSettings), jackSettings.drive);
}
