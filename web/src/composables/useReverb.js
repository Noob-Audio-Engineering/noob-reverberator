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

export { getClient, hasParam, useNoobVstWebguiFramework, useParam, useStream };

/** What the engine told us about itself at connect time. */
export function meta() {
  return getClient()?.meta ?? {};
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
  const lo = m.fit_bottom ?? 20;
  const hi = m.fit_top ?? 20000;
  return lo * Math.pow(hi / lo, i / Math.max(1, n - 1));
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
