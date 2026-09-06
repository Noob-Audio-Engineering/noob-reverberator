<script setup>
/**
 * The preset list, the build's own and the listener's.
 *
 * Applying one writes every control it names through the ordinary parameter
 * path, so the host records it exactly as it would a person turning the knobs
 * --- which is what makes undo work on a preset and what puts it in
 * automation. Sending a "load this preset" message to the engine instead would
 * change the sound without the host ever knowing anything moved.
 *
 * **Saving keeps the preset out of the project.** A saved preset belongs to
 * whoever saved it, not to the song they happened to be working on, so it goes
 * in the page's store under one key that the plug-in keeps in a file of its
 * own rather than with the session. Everything else on this page --- window
 * size, which band is selected --- stays with the project, which is where
 * those belong.
 */
import { computed, ref } from 'vue';
import { getClient, meta, useParam } from '../composables/useReverb.js';
import { deletePreset, presetList, savePreset } from '../mode.js';

const factory = meta().presets ?? [];
const mode = useParam('mode');
const open = ref(false);
const applied = ref('');
const naming = ref(false);
const draft = ref('');

const client = getClient();
const mine = ref(client.store?.get?.('user_presets', []) ?? []);
// **Subscribed, not read once.** The store arrives from the engine after the
// page has already mounted --- `store.all` hydrates it a moment later --- so
// a single read at setup happens before there is anything to read, and every
// saved preset appears to have been forgotten. `'*'` catches the hydration
// too, because that emits with a null key.
client.store?.on?.('*', () => {
  mine.value = client.store.get('user_presets', []) ?? [];
});
const list = computed(() => presetList(factory, mine.value));

function remember(next) {
  mine.value = next;
  // One key, which the plug-in mirrors to a file beside the discovery
  // records. See `PresetStore` in `src/plugin.rs`.
  client.store?.set?.('user_presets', next);
}

function apply(p) {
  mode.begin();
  mode.setIndex(typeof p.mode === 'number' ? p.mode : mode.index);
  mode.end();
  for (const [id, value] of p.set ?? []) {
    if (!client.hasParam(id)) continue; // a build without that control
    const h = useParam(id);
    h.begin();
    h.setPlain(value);
    h.end();
  }
  applied.value = p.name;
  open.value = false;
}

function save() {
  // Every control this build has, at what it reads now. Straight off the
  // client's own parameter objects: they already carry `id` and `plain`, and
  // going back through `useParam` for each would build a reactive wrapper per
  // control to read one number off it.
  const values = (client.params ?? []).map((p) => [p.id, p.plain]);
  const { presets, saved } = savePreset(draft.value, mode.index ?? 0, values, mine.value);
  if (saved) {
    remember(presets);
    applied.value = saved.name;
  }
  draft.value = '';
  naming.value = false;
}

function forget(p) {
  remember(deletePreset(p.name, mine.value));
  if (applied.value === p.name) applied.value = '';
}
</script>

<template>
  <div class="relative">
    <!-- Its own class, not the framework's toggle. It is not a toggle: it
         opens a list. Borrowing the toggle's class made it indistinguishable
         from Freeze and Bypass to anything selecting by class, which is a
         small lie that cost me an afternoon of off-by-one selectors. -->
    <button class="preset-button" :class="{ 'is-on': open }" type="button" @click="open = !open">
      {{ applied || 'Presets' }}
    </button>
    <div
      v-if="open"
      class="absolute right-0 z-20 mt-1 max-h-[420px] w-[300px] overflow-y-auto rounded-lg border border-[var(--line)] bg-[var(--panel-2)] p-1 shadow-xl"
    >
      <div
        v-for="p in list"
        :key="(p.user ? 'u:' : 'f:') + p.name"
        class="group flex items-start gap-1 rounded hover:bg-[var(--panel)]"
      >
        <button type="button" class="block flex-1 px-2 py-1.5 text-left" @click="apply(p)">
          <span class="text-[11px] font-semibold">{{ p.name }}</span>
          <span class="block text-[10px] leading-snug text-[var(--faint)]">
            {{ p.blurb || (p.user ? 'Yours.' : '') }}
          </span>
        </button>
        <!-- Only the listener's own. A preset that came with the build is not
             theirs to remove, and offering it would be a button that lies. -->
        <button
          v-if="p.user"
          type="button"
          title="Forget this preset"
          class="px-2 py-1.5 text-[10px] text-[var(--faint)] opacity-0 group-hover:opacity-100 hover:text-red-400"
          @click.stop="forget(p)"
        >
          ×
        </button>
      </div>
      <p v-if="!list.length" class="px-2 py-2 text-[10px] text-[var(--faint)]">
        This build shipped no presets.
      </p>

      <div class="mt-1 border-t border-[var(--line)] pt-1">
        <button
          v-if="!naming"
          type="button"
          class="block w-full rounded px-2 py-1.5 text-left text-[11px] hover:bg-[var(--panel)]"
          @click="naming = true"
        >
          Save these settings…
        </button>
        <form v-else class="flex gap-1 px-1 py-1" @submit.prevent="save">
          <input
            v-model="draft"
            autofocus
            placeholder="Name it"
            class="min-w-0 flex-1 rounded border border-[var(--line)] bg-[var(--panel)] px-1.5 py-1 text-[11px] outline-none focus:border-[var(--accent)]"
          />
          <button type="submit" class="rounded px-2 py-1 text-[11px] hover:bg-[var(--panel)]">
            Save
          </button>
        </form>
      </div>
    </div>
  </div>
</template>
