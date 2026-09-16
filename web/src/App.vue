<script setup lang="ts">
import Wifi from "@primeicons/vue/wifi";
import Card from "primevue/card";
import Slider from "primevue/slider";
import Tag from "primevue/tag";
import { computed } from "vue";

import JackCard from "@/components/JackCard.vue";
import { useInterface } from "@/composables/useInterface";

const { connection, status, settings } = useInterface();

const connectionTag = computed(
  () =>
    ({
      OPEN: { severity: "success", value: "Connected" },
      CONNECTING: { severity: "warn", value: "Connecting" },
      CLOSED: { severity: "danger", value: "Disconnected" },
    })[connection.value],
);
</script>

<template>
  <main class="mx-auto flex max-w-5xl flex-col gap-4 p-4">
    <header class="flex flex-wrap items-center justify-between gap-2">
      <h1 class="flex items-center gap-2 text-2xl font-semibold">
        <Wifi class="text-primary" :size="28" />
        Expression Adapter
      </h1>
      <div class="flex items-center gap-3">
        <span v-if="status" class="text-muted-color">Uptime {{ status.uptimeSeconds }} s</span>
        <Tag v-bind="connectionTag" />
      </div>
    </header>

    <template v-if="settings">
      <Card>
        <template #title>LEDs</template>
        <template #content>
          <div class="flex items-center gap-4">
            <span id="led-brightness">Brightness</span>
            <Slider
              v-model="settings.ledBrightness"
              aria-labelledby="led-brightness"
              :max="255"
              class="grow"
            />
            <span class="w-10 text-right">{{ settings.ledBrightness }}</span>
          </div>
        </template>
      </Card>

      <div class="grid gap-4 md:grid-cols-2">
        <JackCard
          v-for="(_, index) in settings.jacks"
          :key="index"
          v-model="settings.jacks[index]"
          :index="index"
          :status="status?.jacks[index]"
        />
      </div>
    </template>
  </main>
</template>
