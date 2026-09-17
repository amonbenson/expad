import type { WebSocketStatus } from "@vueuse/core";
import { throttleFilter, useWebSocket, watchIgnorable } from "@vueuse/core";
import type { Ref, ShallowRef } from "vue";
import { ref, shallowRef } from "vue";

import type { Settings, Status, Update } from "@/interface";

const SETTINGS_SEND_INTERVAL_MS = 100;

export interface DeviceInterface {
  connection: Ref<WebSocketStatus>;
  /** Latest measurements, `undefined` while no status has arrived. */
  status: ShallowRef<Status | undefined>;
  /** Editable settings, `undefined` until the firmware sends them on connect. */
  settings: Ref<Settings | undefined>;
}

/** Live connection to the firmware: `status` streams in, local edits to `settings` are sent back. */
export function useInterface(): DeviceInterface {
  const status = shallowRef<Status>();
  const settings = ref<Settings>();

  const { status: connection, send } = useWebSocket(`ws://${location.host}/ws`, {
    autoReconnect: true,
    onDisconnected: () => (status.value = undefined),
    onMessage(_, event) {
      const update: Update = JSON.parse(event.data);
      if ("status" in update) {
        status.value = update.status;
      } else {
        ignoreUpdates(() => (settings.value = update.settings));
      }
    },
  });

  const { ignoreUpdates } = watchIgnorable(settings, value => send(JSON.stringify(value)), {
    deep: true,
    eventFilter: throttleFilter(SETTINGS_SEND_INTERVAL_MS),
  });

  return { connection, status, settings };
}
