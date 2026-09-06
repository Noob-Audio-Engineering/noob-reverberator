<script setup>
/**
 * The mode strip.
 *
 * **A mode is not one parameter, it is all of them.** This used to be the
 * framework's plain `Segmented` bound to the `mode` control, which set that
 * control and nothing else --- and the engine only ever read it to choose
 * between the four architectures. Measured, driving the engine the way the
 * plug-in drives it gave *four distinct sounds from thirty-two names*: all
 * twenty-five network modes were bit-for-bit identical, as were the four
 * plates and the two springs.
 *
 * So this applies the mode's whole parameter set, which the engine publishes
 * in the manifest alongside its name. It goes through the ordinary parameter
 * path, exactly as `Presets.vue` does and for the same reason: the host then
 * records it as it would a person turning the knobs, so undo works, the
 * automation lane sees it, and a saved session recalls what you could hear.
 * An engine that used a mode's values privately instead would leave every knob
 * on the panel disagreeing with the sound.
 *
 * It is deliberately a click handler rather than a watcher on the mode value.
 * A watcher would also fire when a *preset* sets the mode, and Vue would run
 * it after the preset's own writes --- so choosing a preset would land on its
 * mode's values instead of its own.
 */
import { getClient, meta, useParam } from '../composables/useReverb.js';

const mode = useParam('mode');
const list = meta().modes ?? [];

function pick(i) {
  const m = list[i];
  const client = getClient();
  mode.begin();
  mode.setIndex(i);
  mode.end();
  for (const [id, value] of m?.set ?? []) {
    // A build whose control list has moved on: skip rather than throw.
    if (!client.hasParam(id)) continue;
    const h = useParam(id);
    h.begin();
    h.setPlain(value);
    h.end();
  }
}
</script>

<template>
  <div class="noob-vst-webgui-framework-segmented" role="radiogroup">
    <button
      v-for="(m, i) in list"
      :key="i"
      type="button"
      class="noob-vst-webgui-framework-segment"
      :class="{ 'is-on': mode.index === i }"
      role="radio"
      :aria-checked="mode.index === i"
      :title="m.blurb"
      @click="pick(i)"
    >
      {{ m.name }}
    </button>
  </div>
</template>
