/**
 * What choosing a mode writes.
 *
 * Kept out of the component and given a test, because the bug this exists to
 * prevent was not a wrong value --- it was **no values at all**. The mode
 * strip used to be the framework's plain `Segmented` bound to the `mode`
 * control, which set that one control, and the engine read it only to pick an
 * architecture. Measured, the plug-in made four distinct sounds from
 * thirty-two names.
 *
 * A component that renders is hard to test without a browser. A list of writes
 * is not.
 */

/**
 * The writes a mode implies, dropped to the controls this build actually has.
 *
 * @param {{set?: [string, number][]}} mode one entry of the manifest's `modes`
 * @param {(id: string) => boolean} hasParam
 * @returns {[string, number][]}
 */
export function modePlan(mode, hasParam) {
  const set = mode && Array.isArray(mode.set) ? mode.set : [];
  return set.filter(([id]) => hasParam(id));
}
