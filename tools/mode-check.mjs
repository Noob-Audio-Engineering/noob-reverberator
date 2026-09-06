/**
 * Does choosing a mode on the real page actually move the controls?
 *
 *     node tools/mode-check.mjs http://127.0.0.1:4406/
 *
 * Every other check of this runs in Rust or in node, and neither can see the
 * one thing that was wrong: the *page* did not apply a mode. The strip was the
 * framework's plain `Segmented` bound to the `mode` control, so it set that
 * control and nothing else, the engine read it only to pick an architecture,
 * and the plug-in made four distinct sounds from thirty-two names.
 *
 * `mode::param_set` is tested against `apply_mode` in Rust, and `modePlan` is
 * tested in `web/test`. This is the link between them --- that the strip is
 * wired to the page at all --- and it needs a browser, so it is a script to
 * run rather than a test in CI. Needs a standalone running on the given port
 * and `npx playwright install chromium` once.
 */
import { chromium } from 'playwright';

const url = process.argv[2] ?? 'http://127.0.0.1:4406/';
// Four modes far enough apart that a strip which did nothing could not fake it.
const WANT = [
  ['Concert Hall', { DECAY: '3.20 s', SIZE: '160 %' }],
  ['Small Plate', { DECAY: '1.20 s', SIZE: '55.0 %' }],
  ['Supermassive', { DECAY: '30.00 s', SIZE: '360 %' }],
  ['Tight Ambience', { DECAY: '0.45 s', SIZE: '35.0 %' }],
];

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 950 } });
const errors = [];
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto(url, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);

const read = () =>
  page.evaluate(() => {
    // Line-based rather than a regular expression: the value sits on the
    // line above its label, and an escaped pattern written through a shell
    // heredoc lost a backslash and quietly matched nothing at all.
    const lines = document.body.innerText.split('\n').map((s) => s.trim());
    const out = {};
    for (const key of ['DECAY', 'SIZE', 'DENSITY']) {
      const at = lines.indexOf(key);
      out[key] = at > 0 ? lines[at - 1] : null;
    }
    return out;
  });

let bad = 0;
const seen = [];
for (const [name, want] of WANT) {
  await page.locator('button', { hasText: new RegExp(`^${name}$`) }).first().click();
  await page.waitForTimeout(900);
  const got = await read();
  seen.push(JSON.stringify(got));
  for (const [k, v] of Object.entries(want)) {
    if (got[k] !== v) {
      console.error(`${name}: ${k} reads ${got[k]} and should read ${v}`);
      bad++;
    }
  }
  console.log(`${name.padEnd(16)} ${JSON.stringify(got)}`);
}

// Two modes showing the same thing means the strip is not applying anything,
// which is exactly how this shipped.
if (new Set(seen).size !== seen.length) {
  console.error('two modes left the controls identical, so the strip applied nothing');
  bad++;
}
if (errors.length) {
  console.error('console errors:', errors.slice(0, 5));
  bad++;
}
await browser.close();
console.log(bad ? `\n${bad} problem(s)` : '\nthe strip applies its modes');
process.exit(bad ? 1 : 0);
