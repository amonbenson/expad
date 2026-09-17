<script setup lang="ts">
import ChevronLeft from "@primeicons/vue/chevron-left";
import ChevronRight from "@primeicons/vue/chevron-right";
import { computed } from "vue";

import ResistorCircuit, { type CircuitArm } from "@/components/ResistorCircuit.vue";
import type { JackStatus } from "@/interface";
import { JACK_COLORS } from "@/theme";

const ARM_COUNT = 3;

const { jack, status = undefined } = defineProps<{ jack: number; status?: JackStatus }>();
const collapsed = defineModel<boolean>("collapsed", { required: true });

const color = computed(() => JACK_COLORS[jack]);

const circuitArms = computed<CircuitArm[]>(() =>
  Array.from({ length: ARM_COUNT }, (_, arm) => ({
    pull: status?.pulls[arm] ?? "floating",
    voltage: status?.voltages[arm] ?? null,
    resistance: armResistance(arm),
  })),
);

/** Absolute resistance of `arm` in kΩ, `null` while it is isolated or the total is unresolved. */
function armResistance(arm: number): number | null {
  const relative = status?.resistances.relative[arm];
  const total = status?.resistances.total;

  return relative == null || total == null ? null : relative * total;
}

function formatResistance(kiloOhms: number | null | undefined): string {
  return kiloOhms == null ? "–" : `${kiloOhms.toFixed(1)} kΩ`;
}
</script>

<template>
  <aside
    class="jack-theme flex shrink-0 border-l border-surface-800 bg-surface-900"
    :style="{ '--jack-color': color }"
  >
    <button
      class="flex w-10 shrink-0 cursor-pointer flex-col items-center gap-3 border-r border-surface-800 py-3 hover:bg-surface-800"
      :aria-expanded="!collapsed"
      :title="collapsed ? 'Show topology' : 'Hide topology'"
      @click="collapsed = !collapsed"
    >
      <component
        :is="collapsed ? ChevronLeft : ChevronRight"
        :size="16"
        class="text-muted-color"
      />
      <span class="text-sm text-muted-color [writing-mode:vertical-rl]">
        Topology
      </span>
    </button>

    <div
      class="overflow-hidden transition-[width] duration-200"
      :class="collapsed ? 'w-0' : 'w-180'"
    >
      <div class="flex h-full w-180 flex-col gap-4 p-4">
        <header class="flex items-baseline justify-between">
          <h2
            class="text-xl font-semibold"
            :style="{ color }"
          >
            Jack {{ jack + 1 }}
          </h2>
          <span class="text-sm text-muted-color">
            Total {{ formatResistance(status?.resistances.total) }}
          </span>
        </header>

        <div class="flex flex-1 items-center justify-center">
          <ResistorCircuit :arms="circuitArms" />
        </div>
      </div>
    </div>
  </aside>
</template>
