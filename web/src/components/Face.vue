<script setup>
/**
 * Noob Reverberator's face.
 *
 * The panel is laid out around the one thing the plug-in is about: the decay
 * curve is the biggest object on it, and everything else is arranged under it.
 * A reverb whose selling point is that you draw its decay should not hide the
 * drawing behind a menu.
 *
 * This component is only mounted once the bridge is **ready**. Every
 * `useParam` here would throw before the manifest arrives --- the parameter
 * list comes from the engine, so until it has been received there is nothing
 * to bind to.
 */
import { computed } from 'vue';
import {
  Knob,
  LevelMeter,
  Segmented,
  Toggle,
  useNoobVstWebguiFramework,
  useParam,
} from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import DecayCurve from './DecayCurve.vue';
import BandStrip from './BandStrip.vue';
import Panel from './Panel.vue';
import { bandCount, meta, modes, seconds } from '../composables/useReverb.js';

const bridge = useNoobVstWebguiFramework();
const m = meta();

const mode = useParam('mode');
const bypass = useParam('bypass');
const freeze = useParam('freeze');
const mix = useParam('mix');
const output = useParam('output');

const decay = useParam('decay');
const size = useParam('size');
const density = useParam('density');
const attack = useParam('attack');
const predelay = useParam('predelay');
const width = useParam('width');
const lines = useParam('lines');

const modRate = useParam('mod_rate');
const modDepth = useParam('mod_depth');
const modRandom = useParam('mod_random');

const earlyLevel = useParam('early_level');
const earlySize = useParam('early_size');
const earlyAbsorb = useParam('early_absorb');
const earlyTaps = useParam('early_taps');

const shift = useParam('shift');
const shiftMix = useParam('shift_mix');
const shiftWindow = useParam('shift_window');

const shape = useParam('shape');
const shapeHold = useParam('shape_hold');

const era = useParam('era');
const eraAmount = useParam('era_amount');

const tension = useParam('tension');
const sections = useParam('sections');

const list = modes();
const modeNames = list.map((x) => x.name);
const current = computed(() => list[Math.round(mode.value ?? 0)] ?? {});
const arch = computed(() => current.value.arch ?? 'network');
const bands = Array.from({ length: bandCount() }, (_, i) => i + 1);
</script>

<template>
  <div class="flex h-full flex-col gap-2 overflow-y-auto p-3">
    <!-- Header -->
    <header class="flex shrink-0 items-center gap-3">
      <span
        class="h-2 w-2 shrink-0 rounded-full"
        :style="{ background: bridge.connected ? 'var(--accent)' : '#7f1d1d' }"
      />
      <h1 class="shrink-0 text-sm font-semibold tracking-wide">Noob Reverberator</h1>
      <p class="truncate text-[11px] text-[var(--dim)]">{{ current.blurb }}</p>
      <div class="ml-auto flex shrink-0 items-center gap-3">
        <!-- The meter sizes itself to its box, so the box is what sets it. -->
        <div class="h-4 w-32 overflow-hidden rounded border border-[var(--line)]">
          <LevelMeter stream="meter" orientation="horizontal" />
        </div>
        <Toggle :p="freeze" variant="button" :labels="['Freeze', 'Frozen']" />
        <Toggle :p="bypass" variant="button" :labels="['Bypass', 'Bypassed']" />
      </div>
    </header>

    <!-- The modes, given their own row: eighteen of them do not belong in a
         corner of another panel. -->
    <div class="shrink-0 rounded-xl border border-[var(--line)] bg-[var(--panel)] px-2 py-1.5">
      <Segmented :p="mode" :labels="modeNames" class="text-[10px]" />
    </div>

    <!-- The decay curve, given the room it deserves -->
    <section
      class="min-h-[240px] flex-1 shrink-0 rounded-xl border border-[var(--line)] bg-[var(--panel)] p-2"
    >
      <DecayCurve />
    </section>

    <!-- The bands that shape it -->
    <div class="grid shrink-0 grid-cols-6 gap-2">
      <BandStrip v-for="n in bands" :key="n" :n="n" />
    </div>

    <!-- Everything else -->
    <div class="grid shrink-0 grid-cols-4 gap-2">
      <Panel title="Space" :hint="arch">
        <div class="flex flex-wrap items-end gap-1">
          <Knob :p="decay" :size="44" label="Decay" />
          <Knob :p="size" :size="40" label="Size" />
          <Knob :p="density" :size="40" label="Density" />
          <Knob :p="attack" :size="40" label="Attack" />
          <Knob :p="predelay" :size="40" label="Pre" />
          <Knob :p="width" :size="40" label="Width" />
          <Knob v-if="arch === 'network'" :p="lines" :size="40" label="Lines" />
        </div>
      </Panel>

      <Panel title="Modulation" hint="sweep to wander">
        <div class="flex items-end gap-1">
          <Knob :p="modRate" :size="40" label="Rate" />
          <Knob :p="modDepth" :size="40" label="Depth" />
          <Knob :p="modRandom" :size="40" label="Character" />
        </div>
        <p class="mt-2 text-[10px] leading-snug text-[var(--faint)]">
          Character at nothing is a smooth sweep, which detunes the tail. At full it is a random
          walk, which loosens the same ringing without moving the pitch.
        </p>
      </Panel>

      <Panel title="Early" hint="where the walls are">
        <div class="flex flex-wrap items-end gap-1">
          <Knob :p="earlyLevel" :size="40" label="Level" />
          <Knob :p="earlySize" :size="40" label="Room" />
          <Knob :p="earlyAbsorb" :size="40" label="Absorb" />
          <Knob :p="earlyTaps" :size="40" label="Taps" />
        </div>
      </Panel>

      <Panel title="Shift" hint="into the feedback">
        <div class="flex items-end gap-1">
          <Knob :p="shift" :size="40" label="Interval" />
          <Knob :p="shiftMix" :size="40" label="Amount" />
          <Knob :p="shiftWindow" :size="40" label="Window" />
        </div>
        <p class="mt-2 text-[10px] leading-snug text-[var(--faint)]">
          A longer window transposes more accurately and smears more. Inside a tail the smearing
          costs nothing.
        </p>
      </Panel>
    </div>

    <div class="grid shrink-0 grid-cols-4 gap-2 pb-1">
      <Panel title="Shape" hint="what a room cannot do">
        <Segmented :p="shape" :labels="m.shapes ?? []" class="mb-2 text-[9px]" />
        <Knob :p="shapeHold" :size="40" label="Time" />
      </Panel>

      <Panel title="Era" hint="converters, not topology">
        <Segmented :p="era" :labels="m.eras ?? []" class="mb-2 text-[9px]" />
        <Knob :p="eraAmount" :size="40" label="Amount" />
      </Panel>

      <Panel title="Spring" :hint="arch === 'spring' ? 'dispersion, not density' : 'for the spring modes'">
        <div class="flex items-end gap-1" :class="arch === 'spring' ? '' : 'opacity-40'">
          <Knob :p="tension" :size="40" label="Tension" />
          <Knob :p="sections" :size="40" label="Sections" />
        </div>
      </Panel>

      <Panel title="Output">
        <div class="flex items-end gap-1">
          <Knob :p="mix" :size="44" label="Mix" />
          <Knob :p="output" :size="40" label="Output" />
        </div>
      </Panel>
    </div>
  </div>
</template>
