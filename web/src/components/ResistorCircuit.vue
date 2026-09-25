<script lang="ts">
import type { ArmPull } from "@/interface";

/** One arm of the measured delta network, as reported by the firmware. */
export interface CircuitArm {
  /** Rail the arm is driven to while measuring. */
  pull: ArmPull;
  /** Voltage at the arm's tap in V, `null` when unknown. */
  voltage: number | null;
  /** Absolute arm resistance in kΩ, `null` when the arm is isolated or unresolved. */
  resistance: number | null;
}
</script>

<script setup lang="ts">
import { computed } from "vue";

import { INDICATOR_COLORS } from "@/theme";

const GRID = 12;
const STROKE = 2;
const JUNCTION_RADIUS = STROKE * 2;
const RESISTOR_STROKE = 20;
const FONT_SIZE = 16;
const LABEL_GAP = 16;

const SPACING = GRID * 4;
const DELTA_X = GRID * 7;
const DELTA_Y = GRID * 6;

const WIRE_COLOR = "#888";
const HIGH_RAIL_COLOR = INDICATOR_COLORS.red;
const LOW_RAIL_COLOR = INDICATOR_COLORS.blue;

/** Fractions along an arm, between the center junction and its tap, that the resistor body spans. */
const RESISTOR_BODY_START = 0.33;
const RESISTOR_BODY_END = 0.66;

const DELTA_CENTER_X = SPACING + DELTA_X;
const DELTA_CENTER_Y = SPACING * 2 + DELTA_X;
const DELTA_ARMS = [
  {
    tapX: SPACING + DELTA_X,
    tapY: SPACING * 2,
    labelAlign: "start",
    labelOffsetX: 16,
    labelOffsetY: 0,
    contact: "Tip",
    contactAlign: "middle",
    contactOffsetX: 0,
    contactOffsetY: -LABEL_GAP * 1.5,
  },
  {
    tapX: SPACING + DELTA_X + DELTA_X,
    tapY: SPACING * 2 + DELTA_X + DELTA_Y,
    labelAlign: "start",
    labelOffsetX: 12,
    labelOffsetY: -16,
    contact: "Ring",
    contactAlign: "middle",
    contactOffsetX: 0,
    contactOffsetY: LABEL_GAP * 1.5,
  },
  {
    tapX: SPACING,
    tapY: SPACING * 2 + DELTA_X + DELTA_Y,
    labelAlign: "end",
    labelOffsetX: -12,
    labelOffsetY: -16,
    contact: "Sleeve",
    contactAlign: "end",
    contactOffsetX: -LABEL_GAP,
    contactOffsetY: 0,
  },
];

/** Height each arm's tap wire runs right on, so that the three never overlap. */
const TAP_WIRE_YS = [DELTA_ARMS[0].tapY, DELTA_ARMS[1].tapY, DELTA_ARMS[2].tapY + SPACING * 2];

const TAP_WIRE_EX = DELTA_ARMS[1].tapX + SPACING * 5;
const GND_X = TAP_WIRE_EX - SPACING * 2;
const VCC_X = TAP_WIRE_EX - SPACING * 3;

/** Room left of the circuit for the sleeve's contact label. */
const LEFT_MARGIN = SPACING;
const WIDTH = SPACING + DELTA_X * 2 + SPACING * 8;
const HEIGHT = SPACING + DELTA_X + DELTA_Y + SPACING * 5;

const { arms } = defineProps<{ arms: CircuitArm[] }>();

const measuredVoltages = computed(() =>
  arms.map(arm => arm.voltage).filter(voltage => voltage !== null),
);

/** Each tap voltage between the lowest and the highest one, `null` while they are indistinguishable. */
const voltageRatios = computed(() => {
  const minimum = Math.min(...measuredVoltages.value);
  const span = Math.max(...measuredVoltages.value) - minimum;

  return arms.map(arm =>
    arm.voltage === null || span < 1e-6 ? null : (arm.voltage - minimum) / span,
  );
});

const armViews = computed(() =>
  arms.map((arm, index) => {
    const geometry = DELTA_ARMS[index];
    const towardTap = (fraction: number): { x: number; y: number } => ({
      x: DELTA_CENTER_X + (geometry.tapX - DELTA_CENTER_X) * fraction,
      y: DELTA_CENTER_Y + (geometry.tapY - DELTA_CENTER_Y) * fraction,
    });

    return {
      ...arm,
      ...geometry,
      name: `R${index + 1}`,
      wireY: TAP_WIRE_YS[index],
      voltageRatio: voltageRatios.value[index],
      isolated: arm.resistance === null,
      shorted: arm.resistance === 0,
      bodyStart: towardTap(RESISTOR_BODY_START),
      bodyEnd: towardTap(RESISTOR_BODY_END),
      labelPosition: towardTap(0.5),
    };
  }),
);

function formatResistance(kiloOhms: number | null): string {
  return kiloOhms === null ? "∞" : kiloOhms.toFixed(1);
}

function formatVoltage(volts: number | null): string {
  return volts === null ? "?" : volts.toFixed(2);
}
</script>

<template>
  <svg
    :viewBox="`${-LEFT_MARGIN} 0 ${LEFT_MARGIN + WIDTH} ${HEIGHT}`"
    :style="{ maxWidth: `${LEFT_MARGIN + WIDTH}px` }"
    class="h-auto w-full"
    xmlns="http://www.w3.org/2000/svg"
    fill="none"
    stroke="currentColor"
    :stroke-width="STROKE"
  >
    <template
      v-for="arm in armViews"
      :key="arm.name"
    >
      <!-- Tap wire to the measurement point -->
      <line
        :x1="arm.tapX"
        :y1="arm.tapY"
        :x2="arm.tapX"
        :y2="arm.wireY"
        :stroke="WIRE_COLOR"
      />
      <line
        :x1="arm.tapX"
        :y1="arm.wireY"
        :x2="TAP_WIRE_EX"
        :y2="arm.wireY"
        :stroke="WIRE_COLOR"
      />
      <circle
        :cx="TAP_WIRE_EX"
        :cy="arm.wireY"
        :r="JUNCTION_RADIUS"
        :fill="WIRE_COLOR"
        stroke="none"
      />
      <text
        :x="TAP_WIRE_EX + LABEL_GAP"
        :y="arm.wireY"
        dominant-baseline="middle"
        :font-size="FONT_SIZE"
        stroke="none"
        fill="currentColor"
      >
        {{ formatVoltage(arm.voltage) }} V
        <template v-if="arm.voltageRatio !== null">
          ({{ Math.round(arm.voltageRatio * 100) }} %)
        </template>
      </text>

      <!-- Pull-up rail -->
      <template v-if="arm.pull === 'up'">
        <line
          :x1="VCC_X"
          :y1="SPACING / 2"
          :x2="VCC_X"
          :y2="arm.wireY"
          :stroke="HIGH_RAIL_COLOR"
        />
        <line
          :x1="VCC_X - SPACING / 4"
          :y1="SPACING / 2"
          :x2="VCC_X + SPACING / 4"
          :y2="SPACING / 2"
          :stroke="HIGH_RAIL_COLOR"
        />
        <line
          :x1="VCC_X"
          :y1="SPACING"
          :x2="VCC_X"
          :y2="SPACING * 1.5"
          :stroke="HIGH_RAIL_COLOR"
          :stroke-width="RESISTOR_STROKE"
        />
        <circle
          :cx="VCC_X"
          :cy="arm.wireY"
          :r="JUNCTION_RADIUS"
          :fill="HIGH_RAIL_COLOR"
          stroke="none"
        />
      </template>

      <!-- Pull-down rail -->
      <template v-if="arm.pull === 'down'">
        <line
          :x1="GND_X"
          :y1="HEIGHT - SPACING / 2"
          :x2="GND_X"
          :y2="arm.wireY"
          :stroke="LOW_RAIL_COLOR"
        />
        <line
          :x1="GND_X - SPACING / 4"
          :y1="HEIGHT - SPACING / 2"
          :x2="GND_X + SPACING / 4"
          :y2="HEIGHT - SPACING / 2"
          :stroke="LOW_RAIL_COLOR"
        />
        <line
          :x1="GND_X"
          :y1="HEIGHT - SPACING"
          :x2="GND_X"
          :y2="HEIGHT - SPACING * 1.5"
          :stroke="LOW_RAIL_COLOR"
          :stroke-width="RESISTOR_STROKE"
        />
        <circle
          :cx="GND_X"
          :cy="arm.wireY"
          :r="JUNCTION_RADIUS"
          :fill="LOW_RAIL_COLOR"
          stroke="none"
        />
      </template>

      <!-- Delta arm: a plain wire while it is shorted, left out while it is isolated -->
      <line
        v-if="!arm.isolated"
        :x1="DELTA_CENTER_X"
        :y1="DELTA_CENTER_Y"
        :x2="arm.tapX"
        :y2="arm.tapY"
      />
      <line
        v-if="!arm.isolated && !arm.shorted"
        :x1="arm.bodyStart.x"
        :y1="arm.bodyStart.y"
        :x2="arm.bodyEnd.x"
        :y2="arm.bodyEnd.y"
        :stroke-width="RESISTOR_STROKE"
      />
      <text
        :x="arm.labelPosition.x + arm.labelOffsetX"
        :y="arm.labelPosition.y + arm.labelOffsetY"
        :text-anchor="arm.labelAlign"
        dominant-baseline="middle"
        :font-size="FONT_SIZE"
        stroke="none"
        fill="currentColor"
      >
        {{ arm.name }}: {{ formatResistance(arm.resistance) }} k&#x3A9;
      </text>

      <!-- Tap junction -->
      <circle
        :cx="arm.tapX"
        :cy="arm.tapY"
        :r="JUNCTION_RADIUS"
        fill="currentColor"
        stroke="none"
      />
      <text
        :x="arm.tapX + arm.contactOffsetX"
        :y="arm.tapY + arm.contactOffsetY"
        :text-anchor="arm.contactAlign"
        dominant-baseline="middle"
        :font-size="FONT_SIZE"
        font-weight="600"
        stroke="none"
        fill="currentColor"
      >
        {{ arm.contact }}
      </text>
    </template>

    <!-- Delta center junction -->
    <circle
      :cx="DELTA_CENTER_X"
      :cy="DELTA_CENTER_Y"
      :r="JUNCTION_RADIUS"
      fill="currentColor"
      stroke="none"
    />
  </svg>
</template>
