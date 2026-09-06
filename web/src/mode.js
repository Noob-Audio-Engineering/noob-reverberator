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

/**
 * The preset list the panel shows: the build's own, then the user's.
 *
 * Kept here, beside `modePlan`, and for the same reason — the interesting
 * parts of a preset panel are which presets exist and what applying one
 * writes, and neither of those needs a browser to check.
 *
 * A user preset is stored as `{ name, mode, set }`, the same shape the engine
 * publishes for a factory one, so applying either is one code path. `user` is
 * what tells the panel which ones it may delete: a factory preset is part of
 * the build and is not the listener's to remove.
 *
 * @param {Array} factory the manifest's `presets`
 * @param {Array} user what the page saved, from the store
 */
export function presetList(factory, user) {
  const own = Array.isArray(factory) ? factory : [];
  const mine = Array.isArray(user) ? user : [];
  return [
    ...own.map((p) => ({ ...p, user: false })),
    ...mine.filter((p) => p && typeof p.name === 'string').map((p) => ({ ...p, user: true })),
  ];
}

/**
 * What to store when somebody saves the panel as a preset.
 *
 * Every control the build has, at its current value, minus the ones that are
 * not part of a sound: bypassing and freezing are things you do to a reverb,
 * not descriptions of one, and restoring them would surprise somebody who
 * saved a preset while auditioning it bypassed.
 *
 * A name already in the list replaces it rather than adding a second entry
 * with the same name, which is what people expect from a save dialog.
 *
 * @param {string} name
 * @param {number} modeIndex
 * @param {[string, number][]} values every id and plain value
 * @param {Array} existing the user's current presets
 */
export const NOT_A_SOUND = ['bypass', 'freeze', 'mode'];

export function savePreset(name, modeIndex, values, existing) {
  const trimmed = String(name ?? '').trim();
  if (!trimmed) return { presets: Array.isArray(existing) ? existing : [], saved: null };
  const entry = {
    name: trimmed,
    mode: modeIndex,
    set: values.filter(([id]) => !NOT_A_SOUND.includes(id)),
  };
  const rest = (Array.isArray(existing) ? existing : []).filter((p) => p?.name !== trimmed);
  return { presets: [...rest, entry], saved: entry };
}

/** Remove one of the listener's own presets by name. */
export function deletePreset(name, existing) {
  return (Array.isArray(existing) ? existing : []).filter((p) => p?.name !== name);
}
