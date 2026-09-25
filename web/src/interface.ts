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
  /** Wiper positions in 0..1 the pedal's travel starts and ends at, stretched to the full expression range. */
  minimum: number;
  maximum: number;
  wiper: WiperContact;
}

export interface Settings {
  ledBrightness: number;
  jacks: JackSettings[];
}

export type Update = { status: Status } | { settings: Settings };
