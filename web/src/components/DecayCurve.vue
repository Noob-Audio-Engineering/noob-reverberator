<script setup>
/**
 * The decay curve: how long the tail lasts at each frequency.
 *
 * Two lines are drawn. The **asked** line is the curve the bands describe; the
 * **realised** line is what the filters that are running actually produce,
 * read back out of them by the engine. Both arrive as streams. Neither is
 * computed here, and that is the point --- this plug-in's claim is that the
 * two agree, and a panel that drew the second one from the same arithmetic as
 * the first could not tell you when they did not.
 *
 * Bands are dragged directly: across for frequency, up and down for how much
 * longer or shorter the tail is there. That is the whole interaction.
 */
import { computed, onMounted, onBeforeUnmount, ref } from 'vue';
import { useStream } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import BandPanel from './BandPanel.vue';
import {
  createBandAt,
  curveFreq,
  deleteBand,
  hertz,
  meta,
  seconds,
  selected,
  useBands,
} from '../composables/useReverb.js';

const canvas = ref(null);
// A stream is an object with a `.data` frame on it, not a ref --- and it
// sends nothing until it is subscribed. Both of those bit me: the panel drew
// an empty grid while the engine published happily into a stream nobody had
// asked for.
const asked = useStream('asked');
const realised = useStream('realised');
const bands = useBands();
/**
 * Whether there is a realised curve to draw, decided by **whether the engine
 * published one**.
 *
 * The first version asked the mode table which architecture was running and
 * whether that architecture reads its decay back. That is the engine's rule,
 * written out a second time on the page, and a second copy of a rule is a
 * thing that can disagree with the first. Reading the stream cannot disagree
 * with the stream.
 *
 * A plain `ref` set from the draw loop rather than a `computed`: a stream's
 * `.data` is a field that is replaced, not a reactive source, so a computed
 * over it is evaluated once and never again.
 */
const hasRealised = ref(false);

const dragging = ref(-1);
const hovering = ref(-1);
/// Where a drag began, for the shapes that move relatively rather than to a
/// position.
const dragFrom = { y: 0, mult: 1 };
/// Set for a moment when every slot is in use, so the panel can say so
/// instead of a click doing nothing.
const full = ref(false);

const m = meta();
const F_LO = m.fit_bottom ?? 20;
const F_HI = m.fit_top ?? 20000;
const T_LO = m.min_t60 ?? 0.05;
const T_HI = m.max_t60 ?? 60;

/** The canvas's size in CSS pixels, kept so the axis mapping and the pointer
 *  arithmetic agree with what was last drawn. */
const size = ref({ w: 900, h: 300 });

const xOf = (f) => (Math.log2(f / F_LO) / Math.log2(F_HI / F_LO)) * size.value.w;
const fOf = (x) => F_LO * Math.pow(F_HI / F_LO, x / Math.max(1, size.value.w));
const yOf = (t) => {
  const c = Math.min(T_HI, Math.max(T_LO, t));
  return size.value.h * (1 - Math.log2(c / T_LO) / Math.log2(T_HI / T_LO));
};
const tOf = (y) => T_LO * Math.pow(T_HI / T_LO, 1 - y / Math.max(1, size.value.h));

/**
 * Where a band's handle sits, in frequency.
 *
 * For every shape but one it is the band's own corner or centre, which is
 * where its amount means something. A **tilt** has no centre: its shape is
 * zero at the corner and opposite at the two ends, so a handle there sits on
 * a part of the curve the control cannot move. Dragging it drove Decay from
 * 1.00x to its maximum of 10.00x in one gesture and the marker never moved,
 * because the drag divides by the curve's value at the handle and that value
 * was the base decay whatever the tilt was doing.
 *
 * So a tilt's handle sits two octaves above its corner, on the side the
 * control brightens, where the curve does move with the amount.
 */
function handleFreq(b) {
  return b.shape.index === TILT ? b.freq.plain * 4 : b.freq.plain;
}

/** The index of the tilt shape in the shared crate's table. */
const TILT = 4;

const GRID_F = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
const GRID_T = [0.1, 0.3, 1, 3, 10, 30];

/** The asked curve's value at a frequency, read from the stream rather than
 *  from a second copy of the engine's arithmetic. */
function valueAt(f) {
  const d = asked.data;
  if (!d || !d.length) return 1;
  const t = Math.log2(f / F_LO) / Math.log2(F_HI / F_LO);
  const i = Math.round(t * (d.length - 1));
  return d[Math.min(d.length - 1, Math.max(0, i))];
}

function draw() {
  const el = canvas.value;
  if (!el) return;
  const dpr = window.devicePixelRatio || 1;
  const w = el.clientWidth;
  const h = el.clientHeight;
  if (!w || !h) return;
  if (el.width !== Math.round(w * dpr) || el.height !== Math.round(h * dpr)) {
    el.width = Math.round(w * dpr);
    el.height = Math.round(h * dpr);
  }
  size.value = { w, h };
  const g = el.getContext('2d');
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);

  const accent =
    getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#a78bfa';

  g.strokeStyle = '#2a2440';
  g.fillStyle = '#5b5470';
  g.font = '10px ui-sans-serif, system-ui, sans-serif';
  g.lineWidth = 1;
  for (const f of GRID_F) {
    const x = xOf(f);
    g.beginPath();
    g.moveTo(x, 0);
    g.lineTo(x, h);
    g.stroke();
    g.fillText(hertz(f), x + 3, h - 4);
  }
  for (const t of GRID_T) {
    const y = yOf(t);
    g.beginPath();
    g.moveTo(0, y);
    g.lineTo(w, y);
    g.stroke();
    g.fillText(seconds(t), 3, y - 3);
  }

  const line = (data, colour, width) => {
    if (!data || !data.length) return;
    g.strokeStyle = colour;
    g.lineWidth = width;
    g.beginPath();
    let started = false;
    for (let i = 0; i < data.length; i++) {
      const v = data[i];
      if (!Number.isFinite(v)) {
        started = false;
        continue;
      }
      const x = xOf(curveFreq(i, data.length));
      const y = yOf(v);
      if (started) g.lineTo(x, y);
      else {
        g.moveTo(x, y);
        started = true;
      }
    }
    g.stroke();
  };

  const rd = realised.data;
  const ok = !!(rd && rd.length && Number.isFinite(rd[Math.floor(rd.length / 2)]));
  if (hasRealised.value !== ok) hasRealised.value = ok;

  // Realised underneath: it is the answer, and the asked line over it is the
  // question.
  if (ok) line(rd, '#4ade80', 3);
  line(asked.data, accent, 2);

  bands.forEach((b, i) => {
    // A parameter handle is a `reactive` object, so its computed fields are
    // already unwrapped: `b.on.on` is a boolean and `b.freq.plain` a number.
    // Writing goes through `setPlain`; there is no writable `.value` at all.
    // Reading `.value` on any of them gives `undefined` and writing it does
    // nothing, silently, which is how this canvas came to draw no band
    // markers and ignore every drag.
    if (!b.on.on) return;
    const hf = handleFreq(b);
    const x = xOf(hf);
    const y = yOf(valueAt(hf));
    const isSel = selected.value === i + 1;
    const r = i === dragging.value || i === hovering.value || isSel ? 9 : 6;
    g.beginPath();
    g.arc(x, y, r, 0, Math.PI * 2);
    g.fillStyle = accent;
    g.fill();
    g.strokeStyle = isSel ? '#ffffff' : '#0c0a14';
    g.lineWidth = 2;
    g.stroke();
    g.fillStyle = '#0c0a14';
    g.font = 'bold 9px ui-sans-serif, system-ui, sans-serif';
    g.fillText(String(i + 1), x - 2.5, y + 3);
  });
}

let raf = 0;
function loop() {
  draw();
  raf = requestAnimationFrame(loop);
}

onMounted(() => {
  // Ten frames a second is plenty: these curves move when a control does, and
  // a stream sends nothing at all until it is subscribed.
  asked.subscribe({ maxHz: 10 });
  realised.subscribe({ maxHz: 10 });
  loop();
});
onBeforeUnmount(() => cancelAnimationFrame(raf));

function pick(ev) {
  const rect = canvas.value.getBoundingClientRect();
  const x = ev.clientX - rect.left;
  const y = ev.clientY - rect.top;
  let best = -1;
  let bestD = 18;
  bands.forEach((b, i) => {
    if (!b.on.on) return;
    const hf = handleFreq(b);
    const d = Math.hypot(xOf(hf) - x, yOf(valueAt(hf)) - y);
    if (d < bestD) {
      bestD = d;
      best = i;
    }
  });
  return { x, y, i: best };
}

function onDown(ev) {
  const { x, y, i } = pick(ev);

  // Nothing under the pointer: make a band here.
  //
  // This is the whole interaction, and it is Noob-Q's: a band is not a strip
  // you fill in, it is a thing you put where you want it. Six strips sitting
  // there whether or not they are used ask somebody to think about slots; a
  // curve you click on asks them to think about the reverb.
  if (i < 0) {
    if (ev.button !== 0) return;
    const f = fOf(x);
    // The decay it would have here with no band, so the new band's multiplier
    // puts the curve exactly under the pointer.
    const base = valueAt(f);
    const n = createBandAt(f, tOf(y) / Math.max(0.01, base));
    if (n == null) {
      full.value = true;
      window.setTimeout(() => (full.value = false), 2200);
      return;
    }
    // Carry straight on into a drag, so putting a band down and shaping it
    // is one gesture rather than two.
    dragging.value = n - 1;
    dragFrom.y = ev.clientY;
    dragFrom.mult = bands[n - 1].mult.plain;
    bands[n - 1].freq.begin();
    bands[n - 1].mult.begin();
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return;
  }

  // A band under the pointer: select it and start dragging. Removing it is a
  // double click, which is its own handler --- `detail` is 0 on a pointer
  // event, so counting clicks there never fires, and the second click of a
  // double just made another band.
  selected.value = i + 1;
  dragging.value = i;
  dragFrom.y = ev.clientY;
  dragFrom.mult = bands[i].mult.plain;
  bands[i].freq.begin();
  bands[i].mult.begin();
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
}

function onMove(ev) {
  const i = dragging.value;
  if (i < 0) return;
  const b = bands[i];
  const rect = canvas.value.getBoundingClientRect();

  // Horizontal is always the band's own frequency. A tilt's handle is drawn
  // two octaves above it, so the drag puts that back.
  const under = fOf(ev.clientX - rect.left);
  b.freq.setPlain(b.shape.index === TILT ? under / 4 : under);

  if (b.shape.index === TILT) {
    // A tilt has no centre, so there is no frequency at which "the curve
    // reads `mult` times the base" and nothing to invert. The alternative
    // would be to work the band's shape out here, and that is the engine's
    // arithmetic written a second time on the page --- the one thing this
    // panel is built to avoid.
    //
    // So a tilt is dragged **relatively**: a fixed distance doubles the
    // amount. It needs to know nothing about the shape, and the handle still
    // follows the pointer, because the curve under it moves with the amount.
    const perDoubling = size.value.h / 6;
    const k = Math.pow(2, (dragFrom.y - ev.clientY) / perDoubling);
    b.mult.setPlain(dragFrom.mult * k);
    return;
  }

  // Everything else has a centre where the curve reads exactly `mult` times
  // what it would without this band, so the drag inverts exactly. Converted
  // through the curve without the band, so dragging means the same thing
  // wherever the Decay control happens to be.
  const withBand = valueAt(b.freq.plain);
  const without = withBand / Math.max(0.01, b.mult.plain);
  b.mult.setPlain(tOf(ev.clientY - rect.top) / Math.max(0.01, without));
}

function onUp() {
  const i = dragging.value;
  if (i >= 0) {
    bands[i].freq.end();
    bands[i].mult.end();
  }
  dragging.value = -1;
  window.removeEventListener('pointermove', onMove);
  window.removeEventListener('pointerup', onUp);
}

function onHover(ev) {
  hovering.value = pick(ev).i;
}

/// Right-click removes a band, for anybody who does not think to double-click.
function onRightClick(ev) {
  const { i } = pick(ev);
  if (i >= 0) deleteBand(i + 1);
}

/// Double-click removes one too.
function onDoubleClick(ev) {
  const { i } = pick(ev);
  if (i >= 0) deleteBand(i + 1);
}

const legend = computed(() =>
  hasRealised.value
    ? 'drawn against what the filters are doing'
    : 'this architecture carries one loss for a whole lap, so there is no per-frequency decay to read back',
);
</script>

<template>
  <div class="relative h-full w-full">
    <canvas
      ref="canvas"
      class="h-full w-full nodrag"
      @pointerdown="onDown"
      @pointermove="onHover"
      @contextmenu.prevent="onRightClick"
      @dblclick="onDoubleClick"
    />
    <BandPanel />
    <!-- What the canvas does, said once, where somebody looking at it is. -->
    <div class="pointer-events-none absolute left-3 top-2 text-[10px] text-[var(--faint)]">
      <span v-if="full" class="text-amber-400">
        every band is in use — remove one to add another
      </span>
      <span v-else>click to add a band · drag to shape it · double-click to remove</span>
    </div>
    <div class="pointer-events-none absolute right-3 top-2 flex items-center gap-3 text-[10px]">
      <span class="flex items-center gap-1" style="color: var(--accent)">
        <span class="inline-block h-[2px] w-4" style="background: var(--accent)" />asked for
      </span>
      <span v-if="hasRealised" class="flex items-center gap-1 text-emerald-400">
        <span class="inline-block h-[3px] w-4 bg-emerald-400" />realised
      </span>
      <span class="text-[var(--faint)]">{{ legend }}</span>
    </div>
  </div>
</template>
