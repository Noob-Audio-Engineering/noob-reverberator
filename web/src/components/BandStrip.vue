<script setup>
/** One decay band's controls, for the row under the curve. */
import { Knob, Segmented, Toggle } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import { useBand, meta } from '../composables/useReverb.js';

const props = defineProps({ n: { type: Number, required: true } });
const b = useBand(props.n);
const shapes = meta().band_shapes ?? ['Bell', 'Low Shelf', 'High Shelf', 'Notch'];
</script>

<template>
  <div
    class="rounded-lg border p-2"
    :class="b.on.value ? 'border-[var(--accent-dim)] bg-[var(--panel-2)]' : 'border-[var(--line)] bg-[var(--panel)]'"
  >
    <div class="mb-1 flex items-center justify-between">
      <span class="text-[10px] font-semibold tracking-wide text-[var(--dim)]">BAND {{ n }}</span>
      <Toggle :p="b.on" variant="button" :labels="['off', 'on']" />
    </div>
    <Segmented :p="b.shape" :labels="shapes" class="mb-2 text-[9px]" />
    <div class="flex items-end justify-between gap-1">
      <Knob :p="b.freq" :size="38" label="Freq" />
      <Knob :p="b.mult" :size="38" label="Decay" />
      <Knob :p="b.width" :size="38" label="Width" />
    </div>
  </div>
</template>
