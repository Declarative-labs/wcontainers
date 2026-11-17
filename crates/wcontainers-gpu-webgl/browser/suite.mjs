// Runs the shared back-end suite through the WebGL 2 back end in Chrome, one page per test:
//   node suite.mjs <pkg-dir> <out-file> <list-file> [<extensions to switch off>]
// <pkg-dir> holds wasm-bindgen's output for examples/browser.rs; <list-file> is the native suite's
// `--list --format terse` output, so a test the browser host lacks fails instead of going unrun.
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
try {
  for (const test of tests) {
    const page = await browser.newPage();
    page.on('console', (m) => console.log(`${test}: console: ${m.text()}`));
    let verdict;
    try {
      await page.goto(`http://127.0.0.1:${server.address().port}/suite.html?test=${test}&off=${encodeURIComponent(off)}`);
      await page.waitForFunction(() => window.done !== undefined, null, { timeout: 120000 });
      verdict = await page.evaluate(() => window.done);
    } catch (e) {
      verdict = { ok: false, message: `the page did not finish: ${e.message}` };
    }
    await page.close();
    const line = verdict.ok ? `${test}: ok` : `${test}: FAILED ${verdict.message.replace(/\s+/g, ' ')}`;
    if (verdict.ok) passed++;
    console.log(line);
    lines.push(line);
  }
} finally {
  await browser.close();
  server.close();
}
const summary = `suite in chrome${off ? ` without ${off}` : ''}: ${passed} passed; ${tests.length - passed} failed`;
console.log(summary);
await writeFile(out, `${lines.join('\n')}\n${summary}\n`);
process.exitCode = passed === tests.length ? 0 : 1;
