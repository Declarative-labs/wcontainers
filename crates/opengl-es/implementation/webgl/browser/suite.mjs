// Runs the OpenGL ES conformance suite's tests that need no EGL on WebGL 2 in headless Chrome, one page per test, and
// measures a query read as soon as it ended:
//   node suite.mjs <pkg-dir> <out-file> [<queries to measure>]
// <pkg-dir> holds wasm-bindgen's output for examples/browser.rs. CHROME names a Chrome or Chromium executable and
// PLAYWRIGHT_PACKAGE the package.json of playwright-core. Writes one line per test (`<name>: ok` or `<name>: FAILED
// <message>`), the measurements, and a summary to <out-file>; exit 1 on any failure.
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const [pkg, out, queries = '200'] = process.argv.slice(2);
if (!out) throw new Error('usage: node suite.mjs <pkg-dir> <out-file> [<queries to measure>]');
const { chromium } = createRequire(process.env.PLAYWRIGHT_PACKAGE)('playwright-core');

// Serves suite.html from this folder and /pkg/ from <pkg-dir>.
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const server = createServer(async (request, response) => {
  const path = new URL(request.url, 'http://localhost').pathname;
  const file = path === '/suite.html' ? join(here, 'suite.html') : path.startsWith('/pkg/') ? join(pkg, path.slice(5)) : null;
  if (!file) return response.writeHead(404).end();
  try {
    response.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' }).end(await readFile(file));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
// No GPU on the build machines: ANGLE on SwiftShader, Chrome's own CPU renderer, which it takes for WebGL only when
// asked.
const browser = await chromium.launch({
  executablePath: process.env.CHROME, headless: true,
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--no-sandbox'],
});
console.log(`chrome ${browser.version()}`);
const lines = [];
async function page(query, label) {
  const tab = await browser.newPage();
  tab.on('console', (m) => console.log(`${label}: console: ${m.text()}`));
  let verdict;
  try {
    await tab.goto(`http://127.0.0.1:${server.address().port}/suite.html?${query}`);
    await tab.waitForFunction(() => window.done !== undefined, null, { timeout: 300000 });
    verdict = await tab.evaluate(() => window.done);
  } catch (e) {
    verdict = { ok: false, message: `the page did not finish: ${e.message}` };
  }
  await tab.close();
  return verdict;
}
let passed = 0;
let failed = 0;
try {
  const listed = await page('list=1', 'list');
  if (!listed.ok) throw new Error(`no list: ${listed.message}`);
  for (const test of listed.message.split(' ')) {
    const verdict = await page(`test=${test}`, test);
    const line = verdict.ok ? `${test}: ok` : `${test}: FAILED ${verdict.message.replace(/\s+/g, ' ')}`;
    if (verdict.ok) passed++; else failed++;
    console.log(line);
    lines.push(line);
  }
  // The query wait's cost, counted as a test (it asserts that every read waited and every query passed); and a host
  // without JSPI, which cannot wait: query_results_wait must fail there at its first read, as implementation.rs says.
  for (const [query, label, expected, because] of [[`measure=${queries}`, 'measure', true, ''],
    ['test=query_results_wait&nojspi=1', 'query_results_wait without JSPI', false, 'the triangle on the pixel buffer passed samples']]) {
    const verdict = await page(query, label);
    const message = verdict.message.replace(/\s+/g, ' ');
    const held = verdict.ok === expected && (expected || message.includes(because));
    const line = `${label}: ${held ? 'ok' : 'FAILED'} (${verdict.ok ? 'passed' : 'failed'}, ${expected ? 'must pass' : `must fail at "${because}"`}) ${message}`;
    if (held) passed++; else failed++;
    console.log(line);
    lines.push(line);
  }
} finally {
  await browser.close();
  server.close();
}
const summary = `suite in chrome: ${passed} passed; ${failed} failed`;
console.log(summary);
await writeFile(out, `${lines.join('\n')}\n${summary}\n`);
process.exitCode = failed === 0 ? 0 : 1;
