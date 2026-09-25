<script lang="ts">
import type { ArmPull } from "@/interface";

/** One contact of a jack as the firmware reports it. */
export interface CircuitContact {
  /** Rail the contact is switched to. */
  pull: ArmPull;
  /** Voltage last measured at the contact's tap in V, `null` when unknown. */
  voltage: number | null;
}

/** One arm of the measured star network, between its star point and a contact. */
export interface CircuitArm extends CircuitContact {
  /** Absolute arm resistance in kΩ, `null` when the arm is isolated or unresolved. */
  resistance: number | null;
}
</script>

<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";

import { INDICATOR_COLORS } from "@/theme";

/*
 * The circuit of one jack, as on the PCB: every contact - tip, ring, sleeve and the tip's normalling
 * contact, the tip switch - can be switched to a pull-up and a pull-down resistor shared by the
 * jack, and is read by the ADC through an RC filter that carries no current, so the ADC reads the
 * contact's own voltage. The pedal's network is drawn as the star the firmware solves for. Rows
 * hop over the pull buses they cross without connecting to them.
 */

const GRID = 12;
const SPACING = GRID * 4;
const STROKE = 2;
const JUNCTION_RADIUS = STROKE * 2;
const TERMINAL_RADIUS = 3;
const HOP_RADIUS = 6;
const RESISTOR_STROKE = 20;
const FONT_SIZE = 16;
const LABEL_GAP = 16;

const WIRE_COLOR = "#888";
const HIGH_RAIL_COLOR = INDICATOR_COLORS.red;
const LOW_RAIL_COLOR = INDICATOR_COLORS.blue;

/** Voltage of the high rail, the ADCs' reference, and resistance of each shared pull in kΩ. */
const HIGH_RAIL_VOLTAGE = 2.5;
const PULL_RESISTANCE = 1;

/**
 * An arm is drawn as a plain wire below `SHORT_BELOW` kΩ and as a resistor again only above
 * `RESISTOR_ABOVE`, the firmware's `closed_resistance`, so a closed switch whose reading hovers
 * around 0 Ω does not flicker between the two.
 */
const SHORT_BELOW = 0.1;
const RESISTOR_ABOVE = 0.2;

/** Fractions along an arm, between the star point and its tap, that the resistor body spans. */
const RESISTOR_BODY_START = 0.33;
const RESISTOR_BODY_END = 0.66;

/** Length of a switch between its two ends, how far its pivot and contact sit inside them, and how far an open blade swings off. */
const SWITCH_LENGTH = SPACING;
const SWITCH_TERMINAL_INSET = 0.25;
const SWITCH_SWING = SPACING * 0.35;

const STAR_ARM_X = GRID * 7;
const STAR_ARM_Y = GRID * 6;
const STAR_X = SPACING * 3;
const TIP_SWITCH_Y = SPACING * 3.5;
const TIP_Y = TIP_SWITCH_Y + SPACING * 1.5;
const STAR_Y = TIP_Y + STAR_ARM_X;
const BOTTOM_TAP_Y = STAR_Y + STAR_ARM_Y;
const SLEEVE_ROW_Y = BOTTOM_TAP_Y + SPACING * 1.5;

/** Pull buses, each switch sitting a little left of its bus, and where every row ends at the ADC. */
const UP_BUS_X = STAR_X + STAR_ARM_X + SPACING * 2.5;
const DOWN_BUS_X = UP_BUS_X + SPACING * 2.5;
const SWITCH_OFFSET = SPACING / 2;
const ROW_END_X = DOWN_BUS_X + SPACING * 1.5;

/** The pull-up resistor hangs from the high rail at the top, the pull-down from ground at the bottom. */
const SUPPLY_Y = SPACING;
const UP_RESISTOR_Y = [SPACING * 1.5, SPACING * 2.25];
const DOWN_RESISTOR_Y = [SLEEVE_ROW_Y + SPACING * 1.75, SLEEVE_ROW_Y + SPACING * 2.5];
const GROUND_Y = SLEEVE_ROW_Y + SPACING * 3.25;

const LEFT_MARGIN = SPACING / 2;
const WIDTH = ROW_END_X + SPACING * 3.5;
const HEIGHT = GROUND_Y + SPACING / 2;

/** Where each contact's tap sits, the height its row runs right on, and where its label goes. */
const CONTACTS = [
  { name: "Tip", tapX: STAR_X, tapY: TIP_Y, rowY: TIP_Y, labelAnchor: "end", labelDx: -LABEL_GAP, labelDy: 0 },
  { name: "Ring", tapX: STAR_X + STAR_ARM_X, tapY: BOTTOM_TAP_Y, rowY: BOTTOM_TAP_Y, labelAnchor: "middle", labelDx: 0, labelDy: LABEL_GAP * 1.5 },
  { name: "Sleeve", tapX: STAR_X - STAR_ARM_X, tapY: BOTTOM_TAP_Y, rowY: SLEEVE_ROW_Y, labelAnchor: "end", labelDx: -LABEL_GAP, labelDy: 0 },
  { name: "Tip switch", tapX: STAR_X, tapY: TIP_SWITCH_Y, rowY: TIP_SWITCH_Y, labelAnchor: "end", labelDx: -LABEL_GAP, labelDy: 0 },
] as const;

/** Where each arm's resistance label sits, relative to the middle of the arm. */
const ARM_LABELS = [
  { anchor: "start", dx: LABEL_GAP, dy: 0 },
  { anchor: "start", dx: 12, dy: -16 },
  { anchor: "end", dx: -12, dy: -16 },
] as const;

const UP_BUS_TOP = SUPPLY_Y;
const UP_BUS_BOTTOM = Math.max(...CONTACTS.map(contact => contact.rowY)) - SWITCH_LENGTH;
const DOWN_BUS_TOP = Math.min(...CONTACTS.map(contact => contact.rowY)) + SWITCH_LENGTH;
const DOWN_BUS_BOTTOM = DOWN_RESISTOR_Y[1];

const { arms, tipSwitch, plugged } = defineProps<{
  /** Tip, ring and sleeve. */
  arms: CircuitArm[];
  tipSwitch: CircuitContact;
  /** Whether a plug lifts the tip switch off the tip. */
  plugged: boolean;
}>();

/** Which arms are drawn as a plain wire, kept until the resistance clearly rises again. */
const shorted = shallowRef<boolean[]>([]);
watch(
  () => arms.map(arm => arm.resistance),
  (resistances) => {
    shorted.value = resistances.map((resistance, arm) =>
      resistance !== null
      && resistance < (shorted.value[arm] ? RESISTOR_ABOVE : SHORT_BELOW),
    );
  },
  { immediate: true },
);

/** Each arm's tap voltage between the lowest and the highest one, `null` while they are indistinguishable. */
const voltageRatios = computed(() => {
  const measured = arms.map(arm => arm.voltage).filter(voltage => voltage !== null);
  const minimum = Math.min(...measured);
  const span = Math.max(...measured) - minimum;

  return arms.map(arm =>
    arm.voltage === null || span < 1e-6 ? null : (arm.voltage - minimum) / span,
  );
});

interface Point {
  x: number;
  y: number;
}

/** A switch between `from` and `to`, drawn along the line between them with its blade pivoting at the `from` end. */
function switchParts(from: Point, to: Point, closed: boolean, swing: number): {
  pivot: Point;
  contact: Point;
  bladeEnd: Point;
} {
  const along = (fraction: number): Point => ({
    x: from.x + (to.x - from.x) * fraction,
    y: from.y + (to.y - from.y) * fraction,
  });
  const pivot = along(SWITCH_TERMINAL_INSET);
  const contact = along(1 - SWITCH_TERMINAL_INSET);

  return { pivot, contact, bladeEnd: closed ? contact : { x: contact.x + swing, y: contact.y } };
}

/** A row's path from its tap to the ADC, hopping over every bus it crosses. */
function rowPath(tapX: number, tapY: number, rowY: number): string {
  const crossings = [
    { x: UP_BUS_X, top: UP_BUS_TOP, bottom: UP_BUS_BOTTOM },
    { x: DOWN_BUS_X, top: DOWN_BUS_TOP, bottom: DOWN_BUS_BOTTOM },
  ].filter(bus => bus.x > tapX && rowY > bus.top && rowY < bus.bottom);
  const hops = crossings
    .map(bus => `H ${bus.x - HOP_RADIUS} a ${HOP_RADIUS} ${HOP_RADIUS} 0 0 1 ${HOP_RADIUS * 2} 0`)
    .join(" ");

  return `M ${tapX} ${tapY} V ${rowY} ${hops} H ${ROW_END_X}`;
}

const contactViews = computed(() =>
  [...arms, tipSwitch].map((contact, index) => {
    const geometry = CONTACTS[index];
    const ratio = index < arms.length ? voltageRatios.value[index] : null;
    const upSwitchX = UP_BUS_X - SWITCH_OFFSET;
    const downSwitchX = DOWN_BUS_X - SWITCH_OFFSET;

    return {
      ...contact,
      ...geometry,
      path: rowPath(geometry.tapX, geometry.tapY, geometry.rowY),
      voltageLabel: contact.voltage === null
        ? "? V"
        : `${contact.voltage.toFixed(3)} V${ratio === null ? "" : ` (${Math.round(ratio * 100)} %)`}`,
      upSwitch: {
        rowX: upSwitchX,
        busY: geometry.rowY - SWITCH_LENGTH,
        ...switchParts(
          { x: upSwitchX, y: geometry.rowY },
          { x: upSwitchX, y: geometry.rowY - SWITCH_LENGTH },
          contact.pull === "up",
          -SWITCH_SWING,
        ),
      },
      downSwitch: {
        rowX: downSwitchX,
        busY: geometry.rowY + SWITCH_LENGTH,
        ...switchParts(
          { x: downSwitchX, y: geometry.rowY },
          { x: downSwitchX, y: geometry.rowY + SWITCH_LENGTH },
          contact.pull === "down",
          -SWITCH_SWING,
        ),
      },
    };
  }),
);

/** The tip switch rests on the tip until a plug lifts it off. */
const normallingSwitch = computed(() =>
  switchParts(
    { x: STAR_X, y: TIP_SWITCH_Y },
    { x: STAR_X, y: TIP_Y },
    !plugged,
    SWITCH_SWING,
  ),
);

const pulledUp = computed(() => contactViews.value.some(contact => contact.pull === "up"));
const pulledDown = computed(() => contactViews.value.some(contact => contact.pull === "down"));

const armViews = computed(() =>
  arms.map((arm, index) => {
    const { tapX, tapY } = CONTACTS[index];
    const towardTap = (fraction: number): Point => ({
      x: STAR_X + (tapX - STAR_X) * fraction,
      y: STAR_Y + (tapY - STAR_Y) * fraction,
    });

    return {
      ...arm,
      tapX,
      tapY,
      name: `R${index + 1}`,
      label: ARM_LABELS[index],
      isolated: arm.resistance === null,
      shorted: shorted.value[index] ?? false,
      bodyStart: towardTap(RESISTOR_BODY_START),
      bodyEnd: towardTap(RESISTOR_BODY_END),
      labelPosition: towardTap(0.5),
    };
  }),
);

function formatResistance(kiloOhms: number | null): string {
  return kiloOhms === null ? "∞" : kiloOhms.toFixed(1);
}
</script>

<template>
  <svg
    :viewBox="`${-LEFT_MARGIN} 0 ${LEFT_MARGIN + WIDTH} ${HEIGHT}`"
    :style="{ maxWidth: `${LEFT_MARGIN + WIDTH}px` }"
    class="size-full"
    xmlns="http://www.w3.org/2000/svg"
    fill="none"
    stroke="currentColor"
    :stroke-width="STROKE"
    :font-size="FONT_SIZE"
  >
    <!-- Pull-up: the high rail, the shared resistor and the bus every contact's up switch closes onto -->
    <g :stroke="pulledUp ? HIGH_RAIL_COLOR : WIRE_COLOR">
      <text
        :x="UP_BUS_X"
        :y="SUPPLY_Y - LABEL_GAP"
        text-anchor="middle"
        stroke="none"
        fill="currentColor"
      >
        {{ HIGH_RAIL_VOLTAGE }} V
      </text>
      <line
        :x1="UP_BUS_X - SPACING / 4"
        :y1="SUPPLY_Y"
        :x2="UP_BUS_X + SPACING / 4"
        :y2="SUPPLY_Y"
      />
      <line
        :x1="UP_BUS_X"
        :y1="UP_BUS_TOP"
        :x2="UP_BUS_X"
        :y2="UP_BUS_BOTTOM"
      />
      <line
        :x1="UP_BUS_X"
        :y1="UP_RESISTOR_Y[0]"
        :x2="UP_BUS_X"
        :y2="UP_RESISTOR_Y[1]"
        :stroke-width="RESISTOR_STROKE"
      />
      <text
        :x="UP_BUS_X + RESISTOR_STROKE / 2 + GRID"
        :y="(UP_RESISTOR_Y[0] + UP_RESISTOR_Y[1]) / 2"
        dominant-baseline="middle"
        stroke="none"
        fill="currentColor"
      >
        {{ PULL_RESISTANCE }} k&#x3A9;
      </text>
    </g>

    <!-- Pull-down: the bus every down switch closes onto, the shared resistor and ground -->
    <g :stroke="pulledDown ? LOW_RAIL_COLOR : WIRE_COLOR">
      <line
        :x1="DOWN_BUS_X"
        :y1="DOWN_BUS_TOP"
        :x2="DOWN_BUS_X"
        :y2="GROUND_Y"
      />
      <line
        :x1="DOWN_BUS_X"
        :y1="DOWN_RESISTOR_Y[0]"
        :x2="DOWN_BUS_X"
        :y2="DOWN_RESISTOR_Y[1]"
        :stroke-width="RESISTOR_STROKE"
      />
      <text
        :x="DOWN_BUS_X + RESISTOR_STROKE / 2 + GRID"
        :y="(DOWN_RESISTOR_Y[0] + DOWN_RESISTOR_Y[1]) / 2"
        dominant-baseline="middle"
        stroke="none"
        fill="currentColor"
      >
        {{ PULL_RESISTANCE }} k&#x3A9;
      </text>
      <line
        v-for="step in 3"
        :key="step"
        :x1="DOWN_BUS_X - (SPACING / 4) * (1 - (step - 1) / 3)"
        :y1="GROUND_Y + (step - 1) * STROKE * 3"
        :x2="DOWN_BUS_X + (SPACING / 4) * (1 - (step - 1) / 3)"
        :y2="GROUND_Y + (step - 1) * STROKE * 3"
      />
    </g>

    <template
      v-for="contact in contactViews"
      :key="contact.name"
    >
      <!-- Row from the tap to the ADC -->
      <path
        :d="contact.path"
        :stroke="WIRE_COLOR"
      />
      <circle
        :cx="ROW_END_X"
        :cy="contact.rowY"
        :r="JUNCTION_RADIUS"
        :fill="WIRE_COLOR"
        stroke="none"
      />
      <text
        :x="ROW_END_X + LABEL_GAP"
        :y="contact.rowY"
        dominant-baseline="middle"
        stroke="none"
        fill="currentColor"
        class="tabular-nums"
      >
        {{ contact.voltageLabel }}
      </text>

      <!-- Up and down switches, each closing onto its bus -->
      <g
        v-for="{ part, color, closed } in [
          { part: contact.upSwitch, color: HIGH_RAIL_COLOR, closed: contact.pull === 'up' },
          { part: contact.downSwitch, color: LOW_RAIL_COLOR, closed: contact.pull === 'down' },
        ]"
        :key="color"
        :stroke="closed ? color : WIRE_COLOR"
      >
        <title>{{ contact.name }} {{ closed ? "switched to" : "off" }} the {{ color === HIGH_RAIL_COLOR ? "pull-up" : "pull-down" }}</title>
        <line
          :x1="part.rowX"
          :y1="contact.rowY"
          :x2="part.pivot.x"
          :y2="part.pivot.y"
        />
        <line
          :x1="part.pivot.x"
          :y1="part.pivot.y"
          :x2="part.bladeEnd.x"
          :y2="part.bladeEnd.y"
        />
        <polyline :points="`${part.contact.x},${part.contact.y} ${part.rowX},${part.busY} ${part.rowX + SWITCH_OFFSET},${part.busY}`" />
        <circle
          :cx="part.rowX"
          :cy="contact.rowY"
          :r="JUNCTION_RADIUS"
          :fill="closed ? color : WIRE_COLOR"
          stroke="none"
        />
        <circle
          :cx="part.rowX + SWITCH_OFFSET"
          :cy="part.busY"
          :r="JUNCTION_RADIUS"
          :fill="closed ? color : WIRE_COLOR"
          stroke="none"
        />
      </g>

      <!-- Tap junction and contact name -->
      <circle
        :cx="contact.tapX"
        :cy="contact.tapY"
        :r="JUNCTION_RADIUS"
        fill="currentColor"
        stroke="none"
      />
      <text
        :x="contact.tapX + contact.labelDx"
        :y="contact.tapY + contact.labelDy"
        :text-anchor="contact.labelAnchor"
        dominant-baseline="middle"
        font-weight="600"
        stroke="none"
        fill="currentColor"
      >
        {{ contact.name }}
      </text>
    </template>

    <!-- The tip switch touching the tip while no plug is inserted -->
    <g>
      <title>{{ plugged ? "Plug inserted: tip switch lifted off the tip" : "No plug: tip switch resting on the tip" }}</title>
      <line
        :x1="STAR_X"
        :y1="TIP_SWITCH_Y"
        :x2="normallingSwitch.pivot.x"
        :y2="normallingSwitch.pivot.y"
      />
      <line
        :x1="normallingSwitch.pivot.x"
        :y1="normallingSwitch.pivot.y"
        :x2="normallingSwitch.bladeEnd.x"
        :y2="normallingSwitch.bladeEnd.y"
      />
      <line
        :x1="normallingSwitch.contact.x"
        :y1="normallingSwitch.contact.y"
        :x2="STAR_X"
        :y2="TIP_Y"
      />
    </g>

    <!-- Switch terminals, drawn over the blades -->
    <template
      v-for="terminal in [
        normallingSwitch.pivot,
        normallingSwitch.contact,
        ...contactViews.flatMap(contact => [
          contact.upSwitch.pivot,
          contact.upSwitch.contact,
          contact.downSwitch.pivot,
          contact.downSwitch.contact,
        ]),
      ]"
      :key="`${terminal.x},${terminal.y}`"
    >
      <circle
        :cx="terminal.x"
        :cy="terminal.y"
        :r="TERMINAL_RADIUS"
        class="fill-surface-900"
        stroke="currentColor"
        :stroke-width="STROKE / 2"
      />
    </template>

    <!-- Star network: a plain wire while an arm is shorted, left out while it is isolated -->
    <template
      v-for="arm in armViews"
      :key="arm.name"
    >
      <line
        v-if="!arm.isolated"
        :x1="STAR_X"
        :y1="STAR_Y"
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
        :x="arm.labelPosition.x + arm.label.dx"
        :y="arm.labelPosition.y + arm.label.dy"
        :text-anchor="arm.label.anchor"
        dominant-baseline="middle"
        stroke="none"
        fill="currentColor"
      >
        {{ arm.name }}: {{ formatResistance(arm.resistance) }} k&#x3A9;
      </text>
    </template>
    <circle
      :cx="STAR_X"
      :cy="STAR_Y"
      :r="JUNCTION_RADIUS"
      fill="currentColor"
      stroke="none"
    />
  </svg>
</template>
