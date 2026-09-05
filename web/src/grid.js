/**
 * The frequency grid the decay curves are drawn on.
 *
 * Its own module, with no imports, for two reasons. It has to be **the same
 * grid the engine published on** --- the engine fills its curve arrays by
 * walking `lo · (hi/lo)^(i/(n−1))`, and a page walking a different one would
 * draw both lines at the wrong frequencies and still look like a reverb. And
 * a thing worth testing should be importable without dragging a whole
 * component library in behind it.
 */

/** Point `i` of `n`, log-spaced from `lo` to `hi`. */
export function logGrid(i, n, lo, hi) {
  return lo * Math.pow(hi / lo, i / Math.max(1, n - 1));
}
