<script setup lang="ts">
import Wifi from "@primeicons/vue/wifi";
import { useMediaQuery } from "@vueuse/core";
import Slider from "primevue/slider";
import Tag from "primevue/tag";
import { computed, ref } from "vue";

import JackStrip from "@/components/JackStrip.vue";
import TopologyPanel from "@/components/TopologyPanel.vue";
import { useInterface } from "@/composables/useInterface";

const { connection, status, settings } = useInterface();

const selectedJack = ref(0);
const topologyCollapsed = ref(!useMediaQuery("(min-width: 64rem)").value);

const connectionTag = computed(
  () =>
    ({
      OPEN: { severity: "secondary", value: "Connected" },
      CONNECTING: { severity: "warn", value: "Connecting" },
      CLOSED: { severity: "danger", value: "Disconnected" },
    })[connection.value],
);
</script>

<template>
  <div class="flex h-dvh flex-col">
    <header class="border-surface-800 flex items-center justify-between gap-3 border-b px-4 py-3">
      <h1 class="flex items-center gap-2 text-xl font-semibold">
        <Wifi class="text-primary" :size="24" />
        Expression Adapter
      </h1>
      <div class="flex items-center gap-3">
        <span v-if="status" class="text-muted-color text-sm">
          Uptime {{ status.uptimeSeconds }} s
        </span>
        <Tag v-bind="connectionTag" />
      </div>
    </header>

    <div v-if="settings" class="flex min-h-0 flex-1">
      <!-- Expression channel strips -->
      <div class="flex min-w-0 flex-1 gap-3 overflow-x-auto p-3">
        <JackStrip
          v-for="(_, jack) in settings.jacks"
          :key="jack"
          v-model="settings.jacks[jack]"
          :jack="jack"
          :status="status?.jacks[jack]"
          :selected="jack === selectedJack"
          @select="selectedJack = jack"
        />

        <!-- LED brightness strip -->
        <section class="bg-surface-900 flex w-24 shrink-0 flex-col">
          <div class="bg-surface-700 h-1.5 shrink-0"></div>
          <div class="flex min-h-0 flex-1 flex-col items-center gap-4 p-3">
            <h2 class="text-muted-color self-start text-sm font-semibold">LEDs</h2>
            <Slider
              v-model="settings.ledBrightness"
              aria-label="LED brightness"
              orientation="vertical"
              :max="255"
              class="min-h-24 flex-1"
            />
            <span class="text-sm tabular-nums">{{ settings.ledBrightness }}</span>
          </div>
        </section>
      </div>

      <TopologyPanel
        v-model:collapsed="topologyCollapsed"
        :jack="selectedJack"
        :status="status?.jacks[selectedJack]"
      />
    </div>
  </div>
</template>
