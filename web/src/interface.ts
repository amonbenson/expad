// Mirrors the JSON protocol of src/web/interface.rs. Non-finite floats are sent as `null`.

export interface ArmResistances {
  /** Relative resistance of each arm, summing to 1. */
  relative: [number | null, number | null, number | null];
  /** Total resistance in kΩ. */
  total: number | null;
}

/** Rail an arm is driven to while it is measured. */
export type ArmPull = "up" | "down" | "floating";

export interface JackStatus {
  value: number;
  resistances: ArmResistances;
  /** Voltage measured at each arm's tap, in V. */
  voltages: [number | null, number | null, number | null];
  /** Rail each arm is driven to while measuring. */
  pulls: [ArmPull, ArmPull, ArmPull];
}

export interface Status {
  uptimeSeconds: number;
  jacks: JackStatus[];
}

export interface JackSettings {
  /** Zero-based MIDI channel. */
  midiChannel: number;
  midiController: number;
  inverted: boolean;
}

export interface Settings {
  ledBrightness: number;
  jacks: JackSettings[];
}

export type Update = { status: Status } | { settings: Settings };
