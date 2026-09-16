<script setup lang="ts">
import Card from "primevue/card";
import InputNumber from "primevue/inputnumber";
import ProgressBar from "primevue/progressbar";
import Select from "primevue/select";
import Tag from "primevue/tag";
import ToggleSwitch from "primevue/toggleswitch";
import { computed, useId } from "vue";

import type { JackSettings, JackStatus } from "@/interface";

const { index, status } = defineProps<{ index: number; status?: JackStatus }>();
const settings = defineModel<JackSettings>({ required: true });

const midiChannels = Array.from({ length: 16 }, (_, channel) => ({
  label: `Channel ${channel + 1}`,
  value: channel,
}));

const id = useId();
const connected = computed(() => status?.resistances.relative.some((arm) => arm !== null) ?? false);
const valuePercent = computed(() => Math.round((status?.value ?? 0) * 100));

function formatPercent(fraction: number | null) {
  return fraction === null ? "–" : `${Math.round(fraction * 100)} %`;
}

function formatResistance(kiloOhms: number | null | undefined) {
  return kiloOhms == null ? "–" : `${kiloOhms.toFixed(1)} kΩ`;
}
</script>

<template>
  <Card>
    <template #title>
      <div class="flex items-center justify-between">
        Jack {{ index + 1 }}
        <Tag
          :severity="connected ? 'success' : 'secondary'"
          :value="connected ? 'Connected' : 'Disconnected'"
        />
      </div>
    </template>

    <template #content>
      <div class="flex flex-col gap-4">
        <!-- The default width transition lags behind the status update rate -->
        <ProgressBar :value="valuePercent" :pt="{ value: { class: 'transition-none' } }" />

        <dl class="grid grid-cols-4 text-center text-sm">
          <div v-for="(fraction, arm) in status?.resistances.relative" :key="arm">
            <dt class="text-muted-color">Arm {{ arm + 1 }}</dt>
            <dd>{{ formatPercent(fraction) }}</dd>
          </div>
          <div>
            <dt class="text-muted-color">Total</dt>
            <dd>{{ formatResistance(status?.resistances.total) }}</dd>
          </div>
        </dl>

        <div class="grid grid-cols-[auto_1fr] items-center gap-3">
          <label :id="`${id}-channel`">MIDI channel</label>
          <Select
            v-model="settings.midiChannel"
            :label-id="`${id}-channel`"
            :options="midiChannels"
            option-label="label"
            option-value="value"
          />

          <label :for="`${id}-controller`">Control change</label>
          <InputNumber
            v-model="settings.midiController"
            :input-id="`${id}-controller`"
            :min="0"
            :max="127"
            show-buttons
          />

          <label :for="`${id}-inverted`">Inverted</label>
          <ToggleSwitch v-model="settings.inverted" :input-id="`${id}-inverted`" />
        </div>
      </div>
    </template>
  </Card>
</template>
