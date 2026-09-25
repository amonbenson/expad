<script setup lang="ts">
import Button from "primevue/button";
import InputNumber from "primevue/inputnumber";
import Select from "primevue/select";
import Slider from "primevue/slider";
import ToggleSwitch from "primevue/toggleswitch";
import { computed, useId } from "vue";

import type { JackMode, JackSettings, JackStatus, WiperContact } from "@/interface";
import { JACK_COLORS } from "@/theme";

const { jack, status = undefined, selected } = defineProps<{
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

const wiperContacts: { label: string; value: WiperContact }[] = [
  { label: "Auto", value: "auto" },
  { label: "Tip", value: "tip" },
  { label: "Ring", value: "ring" },
  { label: "Sleeve", value: "sleeve" },
];

const modeLabels: Record<JackMode, string> = {
  empty: "Empty",
  identifying: "Identifying",
  tracking: "Tracking",
  other: "Other pedal",
  open: "Nothing connected",
};

const id = useId();
const color = computed(() => JACK_COLORS[jack]);
const mode = computed(() => status?.mode ?? "empty");
const valuePercent = computed(() => Math.round((status?.value ?? 0) * 100));
const position = computed(() => status?.position ?? null);

/** The pedal's range in percent of the wiper's travel, as the range slider edits it. */
const rangePercent = computed<number[]>({
  get: () => [settings.value.minimum * 100, settings.value.maximum * 100],
  set: ([minimum = 0, maximum = 100]) => {
    settings.value.minimum = minimum / 100;
    settings.value.maximum = maximum / 100;
  },
});

function setMinimum(): void {
  if (position.value !== null) {
    settings.value.minimum = position.value;
  }
}

function setMaximum(): void {
  if (position.value !== null) {
    settings.value.maximum = position.value;
  }
}
</script>

<template>
  <section
    class="jack-theme flex min-w-36 flex-1 basis-0 cursor-pointer flex-col"
    :class="selected ? 'bg-surface-800' : 'bg-surface-900'"
    :style="{
      '--jack-color': color,
      outline: `0.2rem solid ${selected ? color : 'transparent'}`
    }"
    @click="$emit('select')"
    @focusin="$emit('select')"
  >
    <div
      class="h-1.5 shrink-0"
      :style="{ background: color }"
    />

    <div class="flex min-h-0 flex-1 flex-col gap-2 p-3">
      <header class="flex items-center justify-center">
        <h2
          class="text-5xl font-bold"
          :style="{ color }"
          :title="`Jack ${jack + 1}: ${modeLabels[mode]}`"
        >
          {{ jack + 1 }}
        </h2>
      </header>
      <p class="-mt-2 text-center text-xs text-muted-color">
        {{ modeLabels[mode] }}
      </p>

      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1">
          <label
            :id="`${id}-channel`"
            class="text-sm text-muted-color"
          >
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
          <label
            :for="`${id}-controller`"
            class="text-sm text-muted-color"
          >
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
          <label
            :for="`${id}-inverted`"
            class="text-sm text-muted-color"
          >
            Invert
          </label>
          <ToggleSwitch
            v-model="settings.inverted"
            :input-id="`${id}-inverted`"
          />
        </div>

        <div class="flex flex-col gap-1">
          <span
            :id="`${id}-range`"
            class="text-sm text-muted-color"
          >
            Range
          </span>
          <div class="flex justify-between text-xs whitespace-nowrap text-muted-color tabular-nums">
            <span>{{ rangePercent[0]?.toFixed(1) }} %</span>
            <span>{{ rangePercent[1]?.toFixed(1) }} %</span>
          </div>
          <div class="relative mx-2 my-2">
            <Slider
              v-model="rangePercent"
              :aria-labelledby="`${id}-range`"
              :step="0.5"
              range
            />
            <!-- Where the wiper is now, to set the range from -->
            <div
              v-if="position !== null"
              class="pointer-events-none absolute top-1/2 h-3 w-0.5 -translate-1/2 bg-surface-0"
              :style="{ left: `${position * 100}%` }"
              :title="`Wiper at ${(position * 100).toFixed(1)} %`"
            />
          </div>
          <div class="flex gap-1">
            <Button
              label="Min"
              title="Set the range's start to the wiper's current position"
              size="small"
              severity="secondary"
              :disabled="position === null"
              fluid
              @click="setMinimum"
            />
            <Button
              label="Max"
              title="Set the range's end to the wiper's current position"
              size="small"
              severity="secondary"
              :disabled="position === null"
              fluid
              @click="setMaximum"
            />
          </div>
        </div>

        <div class="flex flex-col gap-1">
          <label
            :id="`${id}-wiper`"
            class="text-sm text-muted-color"
          >
            Wiper
          </label>
          <Select
            v-model="settings.wiper"
            :label-id="`${id}-wiper`"
            :options="wiperContacts"
            option-label="label"
            option-value="value"
            size="small"
            append-to="self"
          />
        </div>
      </div>

      <!-- Expression meter -->
      <div class="flex min-h-24 flex-1 justify-center">
        <div class="jack-meter relative w-8 bg-surface-950">
          <div
            class="absolute inset-x-0 bottom-0"
            :style="{ height: `${valuePercent}%`, background: color }"
          />
        </div>
      </div>

      <div
        class="text-center text-sm tabular-nums"
        :style="{ color }"
      >
        {{ valuePercent }} %
      </div>
    </div>
  </section>
</template>

<style scoped>
/* Scale marks at 25 %, 50 % and 75 % of the meter's height, leaving both ends unmarked */
.jack-meter {
  background-image: linear-gradient(
    to top,
    transparent 25%,
    var(--p-surface-900) 25% calc(25% + 0.1rem),
    transparent calc(25% + 0.1rem) 50%,
    var(--p-surface-900) 50% calc(50% + 0.1rem),
    transparent calc(50% + 0.1rem) 75%,
    var(--p-surface-900) 75% calc(75% + 0.1rem),
    transparent calc(75% + 0.1rem)
  );
}
</style>
