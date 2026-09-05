<script setup>
/**
 * The preset list.
 *
 * Applying one writes every control it names through the ordinary parameter
 * path, so the host records it exactly as it would a person turning the knobs
 * --- which is what makes undo work on a preset and what puts it in
 * automation. Sending a "load this preset" message to the engine instead would
 * change the sound without the host ever knowing anything moved.
 */
import { ref } from 'vue';
import { getClient, meta, useParam } from '../composables/useReverb.js';

const list = meta().presets ?? [];
const mode = useParam('mode');
const open = ref(false);
const applied = ref('');

function apply(p) {
  const client = getClient();
  mode.setIndex(p.mode);
  for (const [id, value] of p.set) {
    if (!client.hasParam(id)) continue; // a build without that control
    const h = useParam(id);
    h.begin();
    h.setPlain(value);
    h.end();
  }
  applied.value = p.name;
  open.value = false;
}
</script>

<template>
  <div class="relative">
    <button
      class="noob-vst-webgui-framework-toggle button"
      :class="{ 'is-on': open }"
      type="button"
      @click="open = !open"
    >
      {{ applied || 'Presets' }}
    </button>
    <div
      v-if="open"
      class="absolute right-0 z-20 mt-1 max-h-[420px] w-[300px] overflow-y-auto rounded-lg border border-[var(--line)] bg-[var(--panel-2)] p-1 shadow-xl"
    >
      <button
        v-for="p in list"
        :key="p.name"
        type="button"
        class="block w-full rounded px-2 py-1.5 text-left hover:bg-[var(--panel)]"
        @click="apply(p)"
      >
        <span class="text-[11px] font-semibold">{{ p.name }}</span>
        <span class="block text-[10px] leading-snug text-[var(--faint)]">{{ p.blurb }}</span>
      </button>
      <p v-if="!list.length" class="px-2 py-2 text-[10px] text-[var(--faint)]">
        This build shipped no presets.
      </p>
    </div>
  </div>
</template>
