<script setup>
/**
 * The selected band's controls, floating over the bottom of the curve.
 *
 * There is no row of six strips. A slot nobody is using is not a control, and
 * six panels sitting there whether or not they are used ask somebody to think
 * about slots rather than about the reverb. A band is made by clicking the
 * curve and shaped here, and when nothing is selected this is not on the
 * screen at all.
 */
import { computed } from 'vue';
import { Knob, Segmented } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import { activeBands, deleteBand, meta, selected, useBand } from '../composables/useReverb.js';

const shapes = meta().band_shapes ?? ['Bell', 'Low Shelf', 'High Shelf', 'Notch', 'Tilt'];
const band = computed(() => (selected.value ? useBand(selected.value) : null));

/// Step to the next band that exists, so several can be shaped without going
/// back to the curve between each.
function step(by) {
  const list = activeBands();
  if (!list.length) {
    selected.value = null;
    return;
  }
  const at = list.indexOf(selected.value);
  const next = at < 0 ? 0 : (at + by + list.length) % list.length;
  selected.value = list[next];
}
</script>

<template>
  <div
    v-if="band"
    class="pointer-events-auto absolute bottom-2 left-1/2 flex -translate-x-1/2 items-end gap-3 rounded-xl border border-[var(--line)] bg-[var(--panel-2)]/95 px-3 py-2 shadow-xl backdrop-blur"
  >
    <div class="flex flex-col gap-1">
      <span class="text-[9px] font-semibold uppercase tracking-[0.08em] text-[var(--faint)]">
        Band {{ selected }}
      </span>
      <Segmented :p="band.shape" :labels="shapes" class="text-[9px]" />
    </div>
    <Knob :p="band.freq" :size="42" label="Freq" />
    <Knob :p="band.mult" :size="42" label="Decay" />
    <Knob :p="band.width" :size="42" label="Width" />
    <div class="flex flex-col gap-1 pb-1">
      <div class="flex gap-1">
        <button class="band-btn" type="button" title="Previous band" @click="step(-1)">‹</button>
        <button class="band-btn" type="button" title="Next band" @click="step(1)">›</button>
      </div>
      <div class="flex gap-1">
        <button class="band-btn" type="button" title="Deselect" @click="selected = null">✕</button>
        <button
          class="band-btn band-btn--warn"
          type="button"
          title="Remove this band"
          @click="deleteBand(selected)"
        >
          🗑
        </button>
      </div>
    </div>
  </div>
</template>
