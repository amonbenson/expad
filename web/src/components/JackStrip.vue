<script setup lang="ts">
import InputNumber from "primevue/inputnumber";
import Select from "primevue/select";
import ToggleSwitch from "primevue/toggleswitch";
import { computed, useId } from "vue";

import type { JackSettings, JackStatus } from "@/interface";
import { JACK_COLORS } from "@/theme";

const { jack, status, selected } = defineProps<{
  jack: number;
  status?: JackStatus;
  selected: boolean;
}>();
const settings = defineModel<JackSettings>({ required: true });
defineEmits<{ select: [] }>();

const midiChannels = Array.from({ length: 16 }, (_, channel) => ({
  label: `${channel + 1}`,
  value: channel,
}));

const id = useId();
const color = computed(() => JACK_COLORS[jack]);
const connected = computed(() => status?.resistances.relative.some((arm) => arm !== null) ?? false);
const valuePercent = computed(() => Math.round((status?.value ?? 0) * 100));
</script>

<template>
  <section
    class="jack-theme flex min-w-36 flex-1 basis-0 cursor-pointer flex-col"
    :class="selected ? 'bg-surface-800' : 'bg-surface-900'"
    :style="{
      '--jack-color': color,
      outline: `2px solid ${selected ? color : 'transparent'}`,
      outlineOffset: '-2px',
    }"
    @click="$emit('select')"
    @focusin="$emit('select')"
  >
    <div class="h-1.5 shrink-0" :style="{ background: color }"></div>

    <div class="flex min-h-0 flex-1 flex-col gap-2 p-3">
      <header class="flex items-center justify-center">
        <h2
          class="text-5xl font-bold"
          :style="{ color }"
          :title="`Jack ${jack + 1}: ${connected ? 'Connected' : 'Disconnected'}`"
        >
          {{ jack + 1 }}
        </h2>
      </header>

      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1">
          <label :id="`${id}-channel`" class="text-muted-color text-sm">
            Midi Channel
          </label>
          <Select
            v-model="settings.midiChannel"
            :label-id="`${id}-channel`"
            :options="midiChannels"
            option-label="label"
            option-value="value"
            size="small"
            append-to="self"
          />
        </div>

        <div class="flex flex-col gap-1">
          <label :for="`${id}-controller`" class="text-muted-color text-sm">
            CC
          </label>
          <InputNumber
            v-model="settings.midiController"
            :input-id="`${id}-controller`"
            :min="0"
            :max="127"
            size="small"
            show-buttons
            fluid
          />
        </div>

        <div class="flex flex-col gap-1">
          <label :for="`${id}-inverted`" class="text-muted-color text-sm">
            Invert
          </label>
          <ToggleSwitch v-model="settings.inverted" :input-id="`${id}-inverted`" />
        </div>
      </div>

      <!-- Expression meter, read like a console's level meter: it fills from the bottom -->
      <div class="flex min-h-24 flex-1 justify-center">
        <div class="jack-meter bg-surface-950 relative w-8">
          <div
            class="absolute inset-x-0 bottom-0"
            :style="{ height: `${valuePercent}%`, background: color }"
          ></div>
        </div>
      </div>

      <div class="text-center text-sm tabular-nums" :style="{ color }">{{ valuePercent }} %</div>
    </div>
  </section>
</template>

<style scoped>
/* Scale marks every 25 % of the meter's height */
.jack-meter {
  background-image: repeating-linear-gradient(
    to top,
    var(--p-surface-800) 0 1px,
    transparent 1px 25%
  );
}
</style>
