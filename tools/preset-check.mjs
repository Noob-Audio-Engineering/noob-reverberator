/**
 * Does saving a preset actually keep it?
 *
 *     node tools/preset-check.mjs http://127.0.0.1:4406/
 *
 * The interesting half of a preset system is the half that outlives the
 * window: `web/test` covers what a save writes and what a list contains, and
 * neither can tell whether the thing reached a file. This saves a preset with
 * a name nobody would type by accident, reloads the page, and looks for it.
 *
 * Needs a standalone running on the given port.
 */
import { chromium } from 'playwright';

const url = process.argv[2] ?? 'http://127.0.0.1:4406/';
const name = 'Probe ' + Date.now();
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 950 } });
const errors = [];
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
page.on('pageerror', (e) => errors.push(String(e)));

const openList = async () => {
  await page.locator('button.preset-button').first().click();
  await page.waitForTimeout(400);
};

await page.goto(url, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);

// Move a control to something recognisable, then save it under a name.
await page.locator('button', { hasText: /^Supermassive$/ }).first().click();
await page.waitForTimeout(700);
await openList();
await page.locator('button', { hasText: /Save these settings/ }).click();
await page.locator('input[placeholder="Name it"]').fill(name);
// The form's own submit, not a text match: the button's text carries the
// template's indentation and an anchored regex will not meet it.
await page.locator('form button[type="submit"]').click();
await page.waitForTimeout(1500);

// A second and a bit, so the plug-in's flusher has run.
await page.waitForTimeout(1500);

// Reload: a fresh page, hydrated from the engine's store.
await page.reload({ waitUntil: 'networkidle' });
await page.waitForTimeout(2500);
await openList();
const survived = await page.locator('span', { hasText: name }).count();

// And it must be deletable, unlike a factory one.
let deletable = 0;
if (survived) {
  const row = page.locator('div.group', { hasText: name }).first();
  deletable = await row.locator('button[title="Forget this preset"]').count();
}
// A factory preset must not offer that button.
const factoryRow = page.locator('div.group', { hasText: 'Vocal Plate' }).first();
const factoryDeletable = await factoryRow.locator('button[title="Forget this preset"]').count();

await browser.close();
let bad = 0;
if (!survived) {
  console.error(`"${name}" did not survive a reload, so nothing was kept`);
  bad++;
} else console.log(`kept across a reload: ${name}`);
if (survived && !deletable) {
  console.error('a saved preset offers no way to remove it');
  bad++;
} else if (survived) console.log('and it can be forgotten');
if (factoryDeletable) {
  console.error('a factory preset offers a delete button, which is a button that lies');
  bad++;
} else console.log('factory presets are not offered for deletion');
if (errors.length) {
  console.error('console errors:', errors.slice(0, 4));
  bad++;
}
console.log(bad ? `\n${bad} problem(s)` : '\npresets are kept, listed and removable');
process.exit(bad ? 1 : 0);
