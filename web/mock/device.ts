// Stand-in for the firmware's WebSocket server (src/web/server.rs) that replays the dummy data of
// the web_interface example, so the web interface can be developed without hardware.
// Started together with the dev server by `npm run dev:mock`.

import { WebSocketServer } from "ws";

import type { JackStatus, Settings, Update } from "../src/interface.ts";

const PORT = 8765;
const JACK_COUNT = 4;
const STATUS_INTERVAL_MS = 100;
const SWEEP_PERIOD_MS = 4000;

let settings: Settings = {
  ledBrightness: 25,
  jacks: Array.from({ length: JACK_COUNT }, () => ({
    midiChannel: 0,
    midiController: 11,
    inverted: false,
  })),
};

const server = new WebSocketServer({ port: PORT, path: "/ws" });
const startedAt = Date.now();

function broadcast(update: Update) {
  const message = JSON.stringify(update);
  server.clients.forEach((client) => client.send(message));
}

/** Mirrors `dummy_jack_status` in src/bin/web_interface.rs: a sweep per jack, the last one unplugged. */
function jackStatus(uptimeMs: number, jack: number): JackStatus {
  if (jack === JACK_COUNT - 1) {
    return { value: 0, resistances: { relative: [null, null, null], total: null } };
  }

  const phaseOffset = (jack * SWEEP_PERIOD_MS) / JACK_COUNT;
  const phase = (uptimeMs + phaseOffset) % SWEEP_PERIOD_MS;
  const value = 1 - Math.abs((2 * phase) / SWEEP_PERIOD_MS - 1);

  return { value, resistances: { relative: [0, value, 1 - value], total: 10 } };
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
