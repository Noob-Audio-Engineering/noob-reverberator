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
import Panel from './Panel.vue';
import ModeStrip from './ModeStrip.vue';
import Presets from './Presets.vue';
import { meta, modes } from '../composables/useReverb.js';

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
const modChaos = useParam('mod_chaos');
const predelaySync = useParam('predelay_sync');
const predelayOffset = useParam('predelay_offset');

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

const tapeMix = useParam('tape_mix');
const tapeTime = useParam('tape_time');
const tapeHeads = useParam('tape_heads');
const tapeFeedback = useParam('tape_feedback');
const tapeWobble = useParam('tape_wobble');
const tapeDrive = useParam('tape_drive');

const choirAmount = useParam('choir_amount');
const choirVowel = useParam('choir_vowel');
const vowels = m.vowels ?? [];
// What the sweep is between, in words. A knob at 2.31 is somewhere between
// "Ee" and "Oh" and the panel should say so rather than show a number whose
// units are the index of a table the listener cannot see.
const vowelNow = computed(() => {
  const v = choirVowel.plain ?? 0;
  const lo = Math.floor(v);
  const hi = Math.min(lo + 1, vowels.length - 1);
  if (!vowels.length) return '';
  if (lo === hi || Math.abs(v - lo) < 0.02) return vowels[lo];
  if (Math.abs(v - hi) < 0.02) return vowels[hi];
  return `${vowels[lo]} to ${vowels[hi]}`;
});
function pickVowel(i) {
  choirVowel.begin();
  choirVowel.setPlain(i);
  choirVowel.end();
}
const choirSpread = useParam('choir_spread');
const choirResonance = useParam('choir_resonance');

const distance = useParam('distance');
const thickness = useParam('thickness');
const duck = useParam('duck');
const duckRelease = useParam('duck_release');
const gate = useParam('gate');
const gateHold = useParam('gate_hold');
const bloom = useParam('bloom');
const bloomTime = useParam('bloom_time');
const bloomSwell = useParam('bloom_swell');

/**
 * What the running architecture's code never reads.
 *
 * Taken from the engine, not from what a mode happens to set: the spring's
 * `set` takes tension, sections, size and the curve and nothing else, so
 * density and modulation reach it nowhere; the early-only architecture
 * produces silence from the tank, so the decay curve, the shift, the choir
 * and the era all act on nothing.
 *
 * Bloom, Tape, Shape and Dynamics are **not** here. They work on every
 * architecture --- Concert Hall simply has them at zero --- and dimming a
 * control that works is worse than leaving it plain.
 */
const inert = computed(() => {
  const a = arch.value;
  const early = a === 'early';
  const spring = a === 'spring';
  return {
    curve: early,
    lines: a !== 'network',
    density: spring || early,
    size: early,
    modulation: spring || early,
    spring: !spring,
    shift: early,
    choir: early,
    era: early,
  };
});

const list = modes();
// `mode.index` and not `mode.value`: a parameter handle is a `reactive`
// object whose computed fields are already unwrapped, so `.value` on one is
// `undefined`. It read as index nought, so the architecture was always
// "network" --- the mode strip changed the sound and the panel never noticed,
// which meant the Spring controls stayed dimmed while the spring was running.
const current = computed(() => list[mode.index ?? 0] ?? {});
const arch = computed(() => current.value.arch ?? 'network');
// Only the bands that exist. There are still thirty-two parameter slots
// underneath --- a host stores automation by index, so the list cannot grow ---
// but a
// slot nobody is using is not a control, and a wall of empty strips asks
// somebody to think about slots instead of about the reverb.
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
        <Presets />
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
      <ModeStrip class="text-[10px]" />
    </div>

    <!-- The decay curve, given the room it deserves -->
    <section
      class="min-h-[240px] flex-1 shrink-0 rounded-xl border border-[var(--line)] bg-[var(--panel)] p-2"
    >
      <DecayCurve />
    </section>

    <!-- Everything else -->
    <div class="grid shrink-0 grid-cols-4 gap-2">
      <Panel title="Space" :hint="arch">
        <div class="mb-2 flex items-center gap-2">
          <span class="text-[9px] uppercase tracking-[0.08em] text-[var(--faint)]">Pre sync</span>
          <Segmented :p="predelaySync" :labels="m.sync ?? []" class="text-[9px]" />
        </div>
        <div class="flex flex-wrap items-end gap-1">
          <Knob :p="decay" :size="44" label="Decay" />
          <Knob
            :p="size"
            :size="40"
            label="Size"
            :disabled="inert.size"
            :class="inert.size ? 'pointer-events-none opacity-45' : ''"
          />
          <Knob
            :p="density"
            :size="40"
            label="Density"
            :disabled="inert.density"
            :class="inert.density ? 'pointer-events-none opacity-45' : ''"
          />
          <Knob :p="attack" :size="40" label="Attack" />
          <Knob :p="predelay" :size="40" label="Pre" />
          <Knob :p="predelayOffset" :size="40" label="Pre Offset" />
          <Knob :p="width" :size="40" label="Width" />
          <Knob v-if="arch === 'network'" :p="lines" :size="40" label="Lines" />
          <Knob :p="distance" :size="40" label="Distance" />
          <Knob :p="thickness" :size="40" label="Thickness" />
        </div>
      </Panel>

      <Panel
        title="Modulation"
        hint="sweep to wander"
        :inert="inert.modulation"
        inert-reason="this architecture has no modulator"
      >
        <div class="flex items-end gap-1">
          <Knob :p="modRate" :size="40" label="Rate" />
          <Knob :p="modDepth" :size="40" label="Depth" />
          <Knob :p="modRandom" :size="40" label="Character" />
          <Knob :p="modChaos" :size="40" label="Chaos" />
        </div>
        <p class="mt-2 text-[10px] leading-snug text-[var(--faint)]">
          Character at nothing is a smooth sweep, which detunes the tail. At full it is a random
          walk, which loosens the same ringing without moving the pitch. Chaos is neither: an
          unsteady transport, drifting well below the rate and fluttering well above it.
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

      <Panel
        title="Shift"
        hint="into the feedback"
        :inert="inert.shift"
        inert-reason="there is no tail to transpose"
      >
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

      <Panel
        title="Era"
        hint="converters, not topology"
        :inert="inert.era"
        inert-reason="it colours the tank, and this one is silent"
      >
        <Segmented :p="era" :labels="m.eras ?? []" class="mb-2 text-[9px]" />
        <Knob :p="eraAmount" :size="40" label="Amount" />
      </Panel>

      <Panel
        title="Spring"
        hint="dispersion, not density"
        :inert="inert.spring"
        inert-reason="only the spring architecture reads these"
      >
        <div class="flex items-end gap-1">
          <Knob :p="tension" :size="40" label="Tension" />
          <Knob :p="sections" :size="40" label="Sections" />
        </div>
      </Panel>

      <Panel title="Tape" hint="in front of the tank">
        <div class="flex flex-wrap items-end gap-1">
          <Knob :p="tapeMix" :size="40" label="Amount" />
          <Knob :p="tapeTime" :size="40" label="Time" />
          <Knob :p="tapeHeads" :size="40" label="Heads" />
          <Knob :p="tapeFeedback" :size="40" label="Repeats" />
          <Knob :p="tapeWobble" :size="40" label="Wobble" />
          <Knob :p="tapeDrive" :size="40" label="Drive" />
        </div>
      </Panel>

      <Panel
        title="Choir"
        hint="vowels on the tail"
        :inert="inert.choir"
        inert-reason="there is no tail to sing"
      >
        <!-- The vowel is a sweep, not a switch: Chorale's whole idea is
             moving from one to the next, so the knob stays and these only
             jump it to a whole one. Without them the control read "2.31" and
             nothing on the panel said what a 2.31 sounds like. -->
        <div class="mb-2 flex items-center gap-2">
          <span class="text-[9px] uppercase tracking-[0.08em] text-[var(--faint)]">Vowel</span>
          <div class="noob-vst-webgui-framework-segmented text-[9px]">
            <button
              v-for="(v, i) in vowels"
              :key="v"
              type="button"
              class="noob-vst-webgui-framework-segment"
              :class="{ 'is-on': Math.round(choirVowel.plain ?? 0) === i }"
              @click="pickVowel(i)"
            >
              {{ v }}
            </button>
          </div>
          <span class="text-[10px] text-[var(--faint)]">{{ vowelNow }}</span>
        </div>
        <div class="flex flex-wrap items-end gap-1">
          <Knob :p="choirAmount" :size="40" label="Amount" />
          <Knob :p="choirVowel" :size="40" label="Sweep" />
          <Knob :p="choirSpread" :size="40" label="Size" />
          <Knob :p="choirResonance" :size="40" label="Resonance" />
        </div>
        <p class="mt-2 text-[10px] leading-snug text-[var(--faint)]">
          A vowel is two or three resonances at fixed frequencies. They do not move with the
          pitch, which is why a sung note stays the same vowel across an octave.
        </p>
      </Panel>

      <Panel title="Bloom" hint="it swells before the tank hears it">
        <div class="flex items-end gap-1">
          <Knob :p="bloom" :size="40" label="Amount" />
          <Knob :p="bloomTime" :size="40" label="Time" />
          <Knob :p="bloomSwell" :size="40" label="Swell" />
        </div>
        <p class="mt-2 text-[10px] leading-snug text-[var(--faint)]">
          A note starts it and it grows on its own, so the reverb arrives after the note rather
          than being faded in behind it. That is what Attack does instead.
        </p>
      </Panel>

      <Panel title="Dynamics" hint="keeping the tail out of the way">
        <div class="flex flex-wrap items-end gap-1">
          <Knob :p="duck" :size="40" label="Duck" />
          <Knob :p="duckRelease" :size="40" label="Release" />
          <Knob :p="gate" :size="40" label="Gate" />
          <Knob :p="gateHold" :size="40" label="Hold" />
        </div>
        <p class="mt-2 text-[10px] leading-snug text-[var(--faint)]">
          Both watch the dry signal, not the wet. The gate's threshold follows the level, so
          the same performance printed quieter gates the same way.
        </p>
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
