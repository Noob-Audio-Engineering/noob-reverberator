<script setup>
/**
 * The shell.
 *
 * It holds nothing but the connection, because everything else needs the
 * parameter list, and the parameter list comes from the engine. `useParam`
 * throws before the manifest has arrived, so the face is mounted only once the
 * bridge says it is ready --- which is also the honest thing to show a person:
 * a panel of controls bound to nothing is worse than a line saying what is
 * being waited for.
 */
import { useNoobVstWebguiFramework } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import Face from './components/Face.vue';

const { ready, connected } = useNoobVstWebguiFramework();
</script>

<template>
  <Face v-if="ready" />
  <div v-else class="flex h-full items-center justify-center gap-3 text-sm text-[var(--dim)]">
    <span
      class="h-2 w-2 rounded-full"
      :style="{ background: connected ? 'var(--accent)' : '#7f1d1d' }"
    />
    {{ connected ? 'reading the parameter list' : 'connecting to the plug-in' }}
  </div>
</template>
