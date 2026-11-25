// Runs the comparisons with the WebGPU back end in Chrome, one page per comparison:
//   node against.mjs <pkg-dir> <out-file>
// <pkg-dir> holds wasm-bindgen's output for examples/browser.rs. Writes each comparison's routes
// (`<name>, <route>: ok` or `<name>, <route>: FAILED <message>`) to <out-file>; exit 1 on any failure.
import { writeFile } from 'node:fs/promises';
import { serve, launch } from './chrome.mjs';

const [pkg, out] = process.argv.slice(2);
if (!out) throw new Error('usage: node against.mjs <pkg-dir> <out-file>');
const server = await serve(pkg);
const browser = await launch();
const lines = [`chrome ${browser.version()}`];
let failed = 0;
let passed = 0;
async function open(query) {
  const page = await browser.newPage();
  page.on('console', (m) => console.log(`console: ${m.text()}`));
  try {
    await page.goto(`http://127.0.0.1:${server.address().port}/against.html${query}`);
    await page.waitForFunction(() => window.done !== undefined, null, { timeout: 120000 });
    return await page.evaluate(() => window.done);
  } catch (e) {
    return { ok: false, lines: [`the page did not finish: ${e.message}`] };
  } finally {
    await page.close();
  }
}
try {
  const list = await open('');
  if (!list.ok || list.lines.length === 0) throw new Error(`no list of comparisons: ${list.lines.join(' ')}`);
  for (const name of list.lines) {
    const verdict = await open(`?name=${name}`);
    const [adapter, ...routes] = verdict.ok || verdict.lines.length > 1 ? verdict.lines : ['', `every route: FAILED ${verdict.lines[0]}`];
    if (adapter) lines.push(`${name}, ${adapter}`);
    for (const r of routes) {
      const line = `${name}, ${r.replace(/\s+/g, ' ')}`;
      if (r.includes(': FAILED ')) failed++;
      else passed++;
      console.log(line);
      lines.push(line);
    }
  }
} finally {
  await browser.close();
  server.close();
}
const summary = `against webgpu in chrome: ${passed} routes equal; ${failed} failed`;
console.log(summary);
await writeFile(out, `${lines.join('\n')}\n${summary}\n`);
process.exitCode = failed === 0 ? 0 : 1;
