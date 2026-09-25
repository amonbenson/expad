<script setup lang="ts">
import { computed } from "vue";

import { driveCurve } from "@/interface";

/** Points the curve is drawn through, enough for the tightest bend to look smooth. */
const SAMPLES = 32;
const SIZE = 100;
const MARKER_RADIUS = 6;

const { drive, travel = null } = defineProps<{
  drive: number;
  /** Where the pedal is in its travel, marked on the curve; `null` without a pedal. */
  travel?: number | null;
}>();

/** Maps a point of the unit square to the SVG, with the value rising upward. */
function toSvg(input: number, output: number): { x: number; y: number } {
  return { x: input * SIZE, y: (1 - output) * SIZE };
}

const curvePoints = computed(() =>
  Array.from({ length: SAMPLES + 1 }, (_, sample) => {
    const input = sample / SAMPLES;
    const { x, y } = toSvg(input, driveCurve(input, drive));
    return `${x},${y}`;
  }).join(" "),
);

const marker = computed(() =>
  travel === null ? null : toSvg(travel, driveCurve(travel, drive)),
);
</script>

<template>
  <svg
    :viewBox="`${-MARKER_RADIUS} ${-MARKER_RADIUS} ${SIZE + 2 * MARKER_RADIUS} ${SIZE + 2 * MARKER_RADIUS}`"
    fill="none"
    aria-hidden="true"
  >
    <rect
      :width="SIZE"
      :height="SIZE"
      class="fill-surface-950"
    />
    <!-- Linear response for reference -->
    <line
      :x1="0"
      :y1="SIZE"
      :x2="SIZE"
      :y2="0"
      class="stroke-surface-700"
      stroke-width="2"
      stroke-dasharray="6 6"
    />
    <polyline
      :points="curvePoints"
      stroke="var(--jack-color)"
      stroke-width="6"
      stroke-linejoin="round"
    />
    <circle
      v-if="marker"
      :cx="marker.x"
      :cy="marker.y"
      :r="MARKER_RADIUS"
      class="fill-surface-0"
    />
  </svg>
</template>
