// Mirrors the JSON protocol of src/web/interface.rs. Non-finite floats are sent as `null`.

export interface ArmResistances {
  /** Relative resistance of each arm, summing to 1. */
  relative: [number | null, number | null, number | null];
  /** Total resistance in kΩ. */
  total: number | null;
}

export interface JackStatus {
  value: number;
  resistances: ArmResistances;
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
