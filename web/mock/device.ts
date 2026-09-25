// Stand-in for the firmware's WebSocket server (src/web/server.rs) that replays the dummy data of
// the web_interface example, so the web interface can be developed without hardware.
// Started together with the dev server by `npm run dev:mock`.

import { WebSocketServer } from "ws";

import type { JackSettings, JackStatus, Settings, Update } from "../src/interface.ts";

const PORT = 8765;
const JACK_COUNT = 4;
const STATUS_INTERVAL_MS = 100;
const SWEEP_PERIOD_MS = 4000;

// Pretended circuit the dummy voltages are derived from, mirroring src/bin/web_interface.rs:
// resistances are in kΩ and currents in mA.
const TOTAL_RESISTANCE = 10;
const PULL_RESISTANCE = 1;
const HIGH_RAIL_VOLTAGE = 2.5;

let settings: Settings = {
  ledBrightness: 127,
  jacks: Array.from({ length: JACK_COUNT }, () => ({
    midiChannel: 0,
    midiController: 11,
    inverted: false,
    minimum: 0,
    maximum: 1,
    wiper: "auto",
  })),
};

const server = new WebSocketServer({ port: PORT, path: "/ws" });
const startedAt = Date.now();

function broadcast(update: Update): void {
  const message = JSON.stringify(update);
  server.clients.forEach(client => client.send(message));
}

/**
 * Mirrors `dummy_arm_voltages` in src/bin/web_interface.rs: arm 1 is pulled up and arm 2 pulled
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

/** Mirrors `JackSettings::value` in src/web/interface.rs: the jack's range stretched to 0..1, then inverted. */
function expressionValue(position: number, jackSettings: JackSettings): number {
  const range = jackSettings.maximum - jackSettings.minimum;
  const stretched = range > 0 ? (position - jackSettings.minimum) / range : position;
  const value = Math.min(Math.max(stretched, 0), 1);

  return jackSettings.inverted ? 1 - value : value;
}

/** Mirrors `dummy_jack_status` in src/bin/web_interface.rs: a sweep per jack, the last one unplugged. */
function jackStatus(uptimeMs: number, jack: number): JackStatus {
  if (jack === JACK_COUNT - 1) {
    return {
      mode: "empty",
      position: null,
      value: 0,
      resistances: { relative: [null, null, null], total: null },
      voltages: [null, null, null],
      pulls: ["floating", "floating", "floating"],
    };
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
    voltages: armVoltages(relative),
    pulls: ["floating", "up", "down"],
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
      uptimeSeconds: Math.floor(uptimeMs / 1000),
      jacks: Array.from({ length: JACK_COUNT }, (_, jack) => jackStatus(uptimeMs, jack)),
    },
  });
}, STATUS_INTERVAL_MS);

console.log(`Mock device listening on ws://localhost:${PORT}/ws`);
