// Runs the shared back-end suite through the WebGL 2 back end in Chrome, one page per test:
//   node suite.mjs <pkg-dir> <out-file> <list-file> [<extensions to switch off>]
// <pkg-dir> holds wasm-bindgen's output for examples/browser.rs; <list-file> is the native suite's
// `--list --format terse` output, so a test the browser host lacks fails instead of going unrun.
// WEBGL_SEED and WEBGL_CASES set the checks' pattern seed and how many seeds each runs (0 and 1).
// Writes one line per test (`<name>: ok` or `<name>: FAILED <message>`) to <out-file>; exit 1 on any
// failure.
import { readFile, writeFile } from 'node:fs/promises';
import { serve, launch } from './chrome.mjs';

const [pkg, out, list, off = ''] = process.argv.slice(2);
if (!list) throw new Error('usage: node suite.mjs <pkg-dir> <out-file> <list-file> [<extensions to switch off>]');
const tests = (await readFile(list, 'utf8')).split('\n').map((l) => l.trim().match(/^(\w+): test$/)?.[1]).filter(Boolean);
if (tests.length === 0) throw new Error(`${list} lists no tests`);
const server = await serve(pkg);
const browser = await launch();
const lines = [];
let passed = 0;
let notRun = 0;
try {
  for (const test of tests) {
    const page = await browser.newPage();
    page.on('console', (m) => console.log(`${test}: console: ${m.text()}`));
    let verdict;
    try {
      await page.goto(`http://127.0.0.1:${server.address().port}/suite.html?test=${test}&off=${encodeURIComponent(off)}&seed=${process.env.WEBGL_SEED ?? 0}&cases=${process.env.WEBGL_CASES ?? 1}`);
      await page.waitForFunction(() => window.done !== undefined, null, { timeout: 120000 });
      verdict = await page.evaluate(() => window.done);
    } catch (e) {
      verdict = { ok: false, message: `the page did not finish: ${e.message}` };
    }
    await page.close();
    const absent = verdict.ok && verdict.message;
    const line = !verdict.ok ? `${test}: FAILED ${verdict.message.replace(/\s+/g, ' ')}`
      : absent ? `${test}: not run: ${verdict.message}` : `${test}: ok`;
    if (absent) notRun++;
    else if (verdict.ok) passed++;
    console.log(line);
    lines.push(line);
  }
} finally {
  await browser.close();
  server.close();
}
const failed = tests.length - passed - notRun;
const summary = `suite in chrome${off ? ` without ${off}` : ''}: ${passed} passed; ${notRun} not run (a feature or format use absent); ${failed} failed`;
console.log(summary);
await writeFile(out, `${lines.join('\n')}\n${summary}\n`);
process.exitCode = failed === 0 ? 0 : 1;
