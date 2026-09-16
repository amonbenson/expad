import { throttleFilter, useWebSocket, watchIgnorable } from "@vueuse/core";
import { ref, shallowRef } from "vue";

import type { Settings, Status, Update } from "@/interface";

const SETTINGS_SEND_INTERVAL_MS = 100;

/** Live connection to the firmware: `status` streams in, local edits to `settings` are sent back. */
export function useInterface() {
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

  const { ignoreUpdates } = watchIgnorable(settings, (value) => send(JSON.stringify(value)), {
    deep: true,
    eventFilter: throttleFilter(SETTINGS_SEND_INTERVAL_MS),
  });

  return { connection, status, settings };
}
