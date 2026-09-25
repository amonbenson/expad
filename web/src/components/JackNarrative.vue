<script setup lang="ts">
import { computed } from "vue";

import type { JackStatus } from "@/interface";
import { describeJack } from "@/narrative";

const { status = undefined } = defineProps<{ status?: JackStatus }>();

const lines = computed(() => describeJack(status));
</script>

<template>
  <dl
    class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-sm leading-snug"
    aria-live="off"
  >
    <template
      v-for="line in lines"
      :key="line.label"
    >
      <dt class="pt-0.5 text-xs font-semibold tracking-wide text-muted-color uppercase">
        {{ line.label }}
      </dt>
      <dd class="text-muted-color">
        <template
          v-for="(segment, index) in line.segments"
          :key="index"
        >
          <!-- v-text keeps the spacing between segments exact -->
          <strong
            v-if="typeof segment === 'object'"
            class="font-semibold whitespace-nowrap text-(--jack-color) tabular-nums"
            v-text="segment.value"
          />
          <span
            v-else
            v-text="segment"
          />
        </template>
      </dd>
    </template>
  </dl>
</template>
