/**
 * The page's curve grid has to be the engine's curve grid.
 *
 * The engine fills its two curve arrays by walking a log-spaced grid from
 * `fit_bottom` to `fit_top`, and the page draws point `i` at whatever
 * frequency it works out for `i`. If the two disagree the lines are drawn at
 * the wrong frequencies --- and they would still look like a reverb, which is
 * why this is worth a test rather than a glance.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { logGrid } from '../src/grid.js';
import { modePlan } from '../src/mode.js';

const LO = 20;
const HI = 20000;
const N = 160;

test('the ends of the grid are exactly the ends of the band', () => {
  assert.ok(Math.abs(logGrid(0, N, LO, HI) - LO) < 1e-9);
  assert.ok(Math.abs(logGrid(N - 1, N, LO, HI) - HI) / HI < 1e-9);
});

test('the grid rises, and by a constant ratio', () => {
  let prev = logGrid(0, N, LO, HI);
  const ratio = logGrid(1, N, LO, HI) / prev;
  for (let i = 1; i < N; i++) {
    const f = logGrid(i, N, LO, HI);
    assert.ok(f > prev, `point ${i} did not rise`);
    assert.ok(Math.abs(f / prev / ratio - 1) < 1e-9, `point ${i} broke the spacing`);
    prev = f;
  }
});

test('the middle point is the geometric centre, which is what a log axis means', () => {
  const mid = logGrid((N - 1) / 2, N, LO, HI);
  assert.ok(Math.abs(mid - Math.sqrt(LO * HI)) / mid < 1e-9);
});

test('a single point does not divide by zero', () => {
  assert.equal(logGrid(0, 1, LO, HI), LO);
});

/**
 * A mode has to write something.
 *
 * The reverb shipped with a mode strip that set the `mode` control and
 * nothing else, and the engine read that only to choose one of four
 * architectures --- so twenty-five of the thirty-two modes were bit-for-bit
 * the same reverb. Nothing failed, because nothing checked that choosing a
 * mode *does* anything.
 */
test('a mode plans every write the manifest gives it', () => {
  const mode = {
    name: 'Concert Hall',
    set: [
      ['decay', 3.2],
      ['size', 160],
      ['band1_mult', 0.45],
    ],
  };
  const plan = modePlan(mode, () => true);
  assert.equal(plan.length, 3);
  assert.deepEqual(plan[0], ['decay', 3.2]);
});

test('a control this build does not have is dropped, not thrown', () => {
  const mode = { set: [['decay', 3.2], ['gone', 1]] };
  const plan = modePlan(mode, (id) => id !== 'gone');
  assert.deepEqual(plan, [['decay', 3.2]]);
});

test('a mode with no set at all plans nothing rather than exploding', () => {
  assert.deepEqual(modePlan(undefined, () => true), []);
  assert.deepEqual(modePlan({}, () => true), []);
  assert.deepEqual(modePlan({ set: null }, () => true), []);
});
