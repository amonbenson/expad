// Stand-in for the firmware's WebSocket server (src/web/server.rs) that replays the dummy data of
// the web_interface example, so the web interface can be developed without hardware. Unlike the
// example, the third jack holds a sustain pedal and the fourth runs through a scripted plug-in story,
// so every mode the firmware reports can be seen.
// Started together with the dev server by `npm run dev:mock`.

import { WebSocketServer } from "ws";

import type { JackStatus, Settings, Update } from "../src/interface.ts";
import { expressionValue } from "../src/interface.ts";

const PORT = 8765;
const JACK_COUNT = 4;
const STATUS_INTERVAL_MS = 100;
const SWEEP_PERIOD_MS = 4000;
const SWITCH_PERIOD_MS = 2000;
const SWITCH_JACK = 2;
const STORY_JACK = 3;

// Pretended circuit the dummy voltages are derived from, mirroring src/bin/web_interface.rs:
// resistances are in kΩ and currents in mA.
const TOTAL_RESISTANCE = 10;
const PULL_RESISTANCE = 1;
const HIGH_RAIL_VOLTAGE = 2.5;

/** Mirrors `DEFAULT_JACK_COLORS` in src/web/interface.rs. */
const DEFAULT_JACK_COLORS = ["#FF7E7E", "#FFA259", "#FFCB56", "#FFEDB9"];

let settings: Settings = {
  ledBrightness: 127,
  jacks: Array.from({ length: JACK_COUNT }, (_, jack) => ({
    midiChannel: 0,
    midiController: 11,
    inverted: false,
    minimum: 0,
    maximum: 1,
    wiper: "auto",
    drive: 0,
    color: DEFAULT_JACK_COLORS[jack],
  })),
};

const server = new WebSocketServer({ port: PORT, path: "/ws" });
const startedAt = Date.now();

function broadcast(update: Update): void {
  const message = JSON.stringify(update);
  server.clients.forEach(client => client.send(message));
}

/**
 * Mirrors `dummy_contact_voltages` in src/bin/web_interface.rs: arm 1 is pulled up and arm 2 pulled
 * down, so both drop the current across their pull resistors, while the floating arm 0 carries no
 * current and its tap sits at the center node voltage.
 */
function armVoltages(relative: [number, number, number]): [number, number, number] {
  const pulledUpResistance = relative[1] * TOTAL_RESISTANCE;
  const pulledDownResistance = relative[2] * TOTAL_RESISTANCE;
  const current
    = HIGH_RAIL_VOLTAGE / (2 * PULL_RESISTANCE + pulledUpResistance + pulledDownResistance);

  const pulledUpVoltage = HIGH_RAIL_VOLTAGE - current * PULL_RESISTANCE;
  const pulledDownVoltage = current * PULL_RESISTANCE;
  const centerVoltage = pulledDownVoltage + current * pulledDownResistance;

  return [centerVoltage, pulledUpVoltage, pulledDownVoltage];
}

/**
 * A stereo sustain pedal, pressed for half of every period, as the firmware follows it: the tip
 * pulled up, the sleeve down and only the tip read. Pressed, its contact reads 0 Ω and a few Ω in
 * turn, so the report switches between a short and a tiny resistance as on the real device.
 */
function switchStatus(uptimeMs: number): JackStatus {
  const pressed = uptimeMs % SWITCH_PERIOD_MS < SWITCH_PERIOD_MS / 2;
  const resistance = pressed ? (Math.random() < 0.5 ? 0 : 0.004) : Infinity;
  const current = HIGH_RAIL_VOLTAGE / (2 * PULL_RESISTANCE + resistance);
  const position = pressed ? 1 : 0;

  // Mirrors `element_resistances` in topology/src/monitor.rs; infinite values are sent as `null`.
  const relative: [number | null, number | null, number | null] = !pressed
    ? [null, null, null]
    : resistance === 0 ? [0, null, 0] : [1, null, 0];

  return {
    mode: "switch",
    position,
    value: expressionValue(position, settings.jacks[SWITCH_JACK]),
    resistances: { relative, total: pressed ? resistance : null },
    // The isolated ring is pulled down with the sleeve and carries no current.
    voltages: [
      HIGH_RAIL_VOLTAGE - current * PULL_RESISTANCE,
      0,
      current * PULL_RESISTANCE,
      HIGH_RAIL_VOLTAGE,
    ],
    pulls: ["up", "down", "down", "floating"],
  };
}

const UNSOLVED: JackStatus["resistances"] = { relative: [null, null, null], total: null };

/** Rheostat the story sweeps between these resistances, in kΩ, against its standard full scale. */
const RHEOSTAT_RANGE = [2, 24];
const RHEOSTAT_FULL_SCALE = 25;

/** A stereo rheostat of `resistance` kΩ, followed through the tip like a switch. */
function rheostatStatus(resistance: number): JackStatus {
  const current = HIGH_RAIL_VOLTAGE / (2 * PULL_RESISTANCE + resistance);
  const position = resistance / RHEOSTAT_FULL_SCALE;

  return {
    mode: "rheostat",
    position,
    value: expressionValue(position, settings.jacks[STORY_JACK]),
    resistances: { relative: [1, null, 0], total: resistance },
    voltages: [HIGH_RAIL_VOLTAGE - current * PULL_RESISTANCE, 0, current * PULL_RESISTANCE, HIGH_RAIL_VOLTAGE],
    pulls: ["up", "down", "down", "floating"],
  };
}

/** Steps of the story, each held for `durationMs`, with the status it shows at `elapsed` of it. */
const STORY: { durationMs: number; status: (elapsed: number) => JackStatus }[] = [
  // Plug checks: the tip switch pulled up against the tip pulled down, halfway between.
  {
    durationMs: 4000,
    status: () => ({
      mode: "empty",
      position: null,
      value: 0,
      resistances: UNSOLVED,
      voltages: [0, null, null, HIGH_RAIL_VOLTAGE / 2],
      pulls: ["down", "floating", "floating", "up"],
    }),
  },
  // Rails, then a solve's first pair; nothing conducts behind this plug.
  {
    durationMs: 1500,
    status: () => ({
      mode: "identifying",
      position: null,
      value: 0,
      resistances: UNSOLVED,
      voltages: [HIGH_RAIL_VOLTAGE, HIGH_RAIL_VOLTAGE, HIGH_RAIL_VOLTAGE, HIGH_RAIL_VOLTAGE],
      pulls: ["up", "up", "up", "floating"],
    }),
  },
  {
    durationMs: 1500,
    status: () => ({
      mode: "identifying",
      position: null,
      value: 0,
      resistances: UNSOLVED,
      voltages: [HIGH_RAIL_VOLTAGE, 0, 1, HIGH_RAIL_VOLTAGE],
      pulls: ["up", "down", "floating", "floating"],
    }),
  },
  {
    durationMs: 4000,
    status: () => ({
      mode: "open",
      position: null,
      value: 0,
      resistances: UNSOLVED,
      voltages: [HIGH_RAIL_VOLTAGE, 0, 0, HIGH_RAIL_VOLTAGE],
      pulls: ["up", "down", "down", "floating"],
    }),
  },
  {
    durationMs: 6000,
    status: (elapsed) => {
      const sweep = 1 - Math.abs((2 * elapsed) / 6000 - 1);
      return rheostatStatus(RHEOSTAT_RANGE[0]! + sweep * (RHEOSTAT_RANGE[1]! - RHEOSTAT_RANGE[0]!));
    },
  },
  // A dual footswitch: no arm at the star point.
  {
    durationMs: 4000,
    status: () => ({
      mode: "other",
      position: null,
      value: 0,
      resistances: { relative: [0.3, 0.45, 0.25], total: 20 },
      voltages: [2.1, 0.4, 1.3, HIGH_RAIL_VOLTAGE],
      pulls: ["up", "down", "floating", "floating"],
    }),
  },
];
const STORY_DURATION_MS = STORY.reduce((total, step) => total + step.durationMs, 0);

function storyStatus(uptimeMs: number): JackStatus {
  let elapsed = uptimeMs % STORY_DURATION_MS;
  for (const step of STORY) {
    if (elapsed < step.durationMs) {
      return step.status(elapsed);
    }

    elapsed -= step.durationMs;
  }

  return STORY[0]!.status(0);
}

/** Mirrors `dummy_jack_status` in src/bin/web_interface.rs for the pedals: a sweep per jack. */
function jackStatus(uptimeMs: number, jack: number): JackStatus {
  if (jack === STORY_JACK) {
    return storyStatus(uptimeMs);
  }

  if (jack === SWITCH_JACK) {
    return switchStatus(uptimeMs);
  }

  const phaseOffset = (jack * SWEEP_PERIOD_MS) / JACK_COUNT;
  const phase = (uptimeMs + phaseOffset) % SWEEP_PERIOD_MS;
  const position = 1 - Math.abs((2 * phase) / SWEEP_PERIOD_MS - 1);

  // Arm 0 is the wiper, so the two pot halves sit on arms 1 and 2, with the wiper `position`
  // of the way from arm 2's (the sleeve's) end.
  const relative: [number, number, number] = [0, 1 - position, position];

  return {
    mode: "tracking",
    position,
    value: expressionValue(position, settings.jacks[jack]),
    resistances: { relative, total: TOTAL_RESISTANCE },
    // The tip switch keeps the high rail from the plug check that found the plug.
    voltages: [...armVoltages(relative), HIGH_RAIL_VOLTAGE],
    pulls: ["floating", "up", "down", "floating"],
  };
}

server.on("connection", (socket) => {
  socket.send(JSON.stringify({ settings }));
  socket.on("message", (data) => {
    settings = JSON.parse(data.toString()) as Settings;
    console.log("settings changed:", JSON.stringify(settings));
    broadcast({ settings });
  });
});

setInterval(() => {
  const uptimeMs = Date.now() - startedAt;
  broadcast({
    status: {
      jacks: Array.from({ length: JACK_COUNT }, (_, jack) => jackStatus(uptimeMs, jack)),
    },
  });
}, STATUS_INTERVAL_MS);

console.log(`Mock device listening on ws://localhost:${PORT}/ws`);
