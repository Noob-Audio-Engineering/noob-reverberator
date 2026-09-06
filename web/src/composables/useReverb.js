import { ref } from 'vue';
/**
 * The one place the page talks to the engine.
 *
 * Everything the panel draws comes through here, and nothing here computes
 * anything the engine already knows. The decay curve on the panel is the
 * engine's `asked` stream and the line beside it is the engine's `realised`
 * stream --- if the page worked either of them out for itself, the panel could
 * agree with the arithmetic while disagreeing with the audio, which is exactly
 * the failure this plug-in is about not having.
 */
import {
  getClient,
  hasParam,
  useNoobVstWebguiFramework,
  useParam,
  useStream,
} from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import { logGrid } from '../grid.js';

export { getClient, hasParam, useNoobVstWebguiFramework, useParam, useStream };

/**
 * What the engine told us about itself at connect time.
 *
 * From the **manifest**, not from `client.meta`: the client only fills that
 * second field when the plug-in sends a sample-rate change, so reading it at
 * setup returns an empty object. Nothing looked broken --- the mode list
 * still rendered, because a labelled parameter carries its own labels and the
 * segmented control falls back to them --- so the panel was quietly running
 * on defaults for everything meta was supposed to supply, and the preset list
 * was empty with no error anywhere.
 */
export function meta() {
  const { manifest } = useNoobVstWebguiFramework();
  return manifest?.value?.meta ?? {};
}

/** The mode list, from the engine rather than repeated here. */
export function modes() {
  return meta().modes ?? [];
}

export function bandCount() {
  return meta().max_bands ?? 6;
}

/** The frequency of curve point `i`, on the same log grid the engine used. */
export function curveFreq(i, n) {
  const m = meta();
  return logGrid(i, n, m.fit_bottom ?? 20, m.fit_top ?? 20000);
}

/** One decay band's five parameters, as one object. */
export function useBand(n) {
  return {
    on: useParam(`band${n}_on`),
    shape: useParam(`band${n}_shape`),
    freq: useParam(`band${n}_freq`),
    mult: useParam(`band${n}_mult`),
    width: useParam(`band${n}_width`),
  };
}

/** Every band, for the curve editor. */
export function useBands() {
  const n = bandCount();
  return Array.from({ length: n }, (_, i) => useBand(i + 1));
}

/**
 * Which band the panel is pointing at, or `null`.
 *
 * A `ref` at module scope so the curve and the strips agree without passing
 * it through every component between them.
 */
export const selected = ref(null);

/**
 * The first band nobody is using, or `null` when they are all in use.
 *
 * **The parameters are still a fixed six.** A host stores automation against a
 * parameter's index and a saved project asks for it by number, so the list
 * cannot grow when somebody clicks. What "creating a band" means is claiming
 * the first slot whose `on` is false --- the same thing Noob-Q does with its
 * twenty-four.
 */
export function firstFreeBand() {
  for (let n = 1; n <= bandCount(); n += 1) {
    if (!useBand(n).on.on) return n;
  }
  return null;
}

/** How many bands are switched on. */
export function activeBands() {
  const out = [];
  for (let n = 1; n <= bandCount(); n += 1) if (useBand(n).on.on) out.push(n);
  return out;
}

/**
 * The shape a band should start as, given where on the curve it was made.
 *
 * Near the ends of the band, a shelf: that is what somebody reaching for the
 * bottom or the top of the spectrum almost always wants, and starting them
 * with a bell there means two corrections before the first useful one. In the
 * middle, a bell. Noob-Q guesses the same way for the same reason.
 */
export function shapeForFrequency(f) {
  const m = meta();
  const lo = m.fit_bottom ?? 20;
  const hi = m.fit_top ?? 20000;
  const t = Math.log2(f / lo) / Math.log2(hi / lo);
  if (t < 0.16) return 1; // low shelf
  if (t > 0.84) return 2; // high shelf
  return 0; // bell
}

/**
 * Claim a free slot and put a band at `freq` with `mult`.
 *
 * Everything the band has is set explicitly, including what is not being
 * asked for. A slot is **recycled**, not allocated, so anything left alone
 * here keeps whatever the last band in that slot had --- which is how a fresh
 * band comes out four octaves wide because the one deleted before it was.
 *
 * Returns the band number, or `null` when every slot is in use.
 */
export function createBandAt(freq, mult) {
  const n = firstFreeBand();
  if (n == null) return null;
  const b = useBand(n);
  const edits = [
    [b.shape, shapeForFrequency(freq)],
    [b.freq, freq],
    [b.mult, mult],
    [b.width, b.width.param.spec.default],
  ];
  for (const [h, v] of edits) {
    h.begin();
    if (h === b.shape) h.setIndex(v);
    else h.setPlain(v);
    h.end();
  }
  b.on.begin();
  b.on.setOn(true);
  b.on.end();
  selected.value = n;
  return n;
}

/** Switch a band off, which frees its slot for the next one. */
export function deleteBand(n) {
  const b = useBand(n);
  b.on.begin();
  b.on.setOn(false);
  b.on.end();
  if (selected.value === n) selected.value = null;
}

/** Seconds to a label a person reads without counting zeros. */
export function seconds(v) {
  if (!Number.isFinite(v)) return '--';
  if (v >= 10) return `${v.toFixed(1)} s`;
  if (v >= 1) return `${v.toFixed(2)} s`;
  return `${Math.round(v * 1000)} ms`;
}

/** Hertz, the same way. */
export function hertz(v) {
  if (!Number.isFinite(v)) return '--';
  return v >= 1000 ? `${(v / 1000).toFixed(v >= 10000 ? 1 : 2)} kHz` : `${Math.round(v)} Hz`;
}
