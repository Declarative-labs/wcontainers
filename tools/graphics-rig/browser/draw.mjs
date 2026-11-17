// Draws the triangle in Chrome through WebGPU and WebGL 2 and saves the pictures:
//   node draw.mjs <scene.json> <out-dir>
// with scene.json from `gt-scene triangle`. Writes chrome-webgpu-triangle.png and
// chrome-webgl2-triangle.png, prints the adapter each API reports and the adapters chrome://gpu lists,
// and fails unless WebGPU drew on a hardware adapter (a GPU, not a fallback or CPU one) and WebGL 2 on
// the same GPU, so a fall back to software drawing fails the run. Chrome is the newest one in
// C:\wc\chrome.
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getInstalledBrowsers } from '@puppeteer/browsers';
import { chromium } from 'playwright-core';

const here = dirname(fileURLToPath(import.meta.url));
const [sceneFile, out] = process.argv.slice(2);
if (!out) throw new Error('usage: node draw.mjs <scene.json> <out-dir>');
const files = {
  '/triangle.html': [join(here, 'triangle.html'), 'text/html'],
  '/triangle.wgsl': [join(here, '../../../crates/wcontainers-graphics-test/src/triangle.wgsl'), 'text/plain'],
  '/scene.json': [sceneFile, 'application/json'],
};
await mkdir(out, { recursive: true });

const server = createServer(async (request, response) => {
  const file = files[new URL(request.url, 'http://localhost').pathname];
  if (!file) return response.writeHead(404).end();
  response.writeHead(200, { 'content-type': file[1] }).end(await readFile(file[0]));
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

const installed = (await getInstalledBrowsers({ cacheDir: 'C:\\wc\\chrome' })).filter((b) => b.browser === 'chrome');
installed.sort((a, b) => a.buildId.localeCompare(b.buildId, undefined, { numeric: true }));
const chrome = installed.at(-1);
if (!chrome) throw new Error('no Chrome in C:\\wc\\chrome');
// The developer features flag only adds the backend, adapter type and full name to GPUAdapterInfo.
const browser = await chromium.launch({
  executablePath: chrome.executablePath,
  headless: true,
  args: ['--enable-webgpu-developer-features'],
});
console.log(`chrome ${browser.version()}`);
const base = `http://127.0.0.1:${server.address().port}`;
let failed = false;
const adapters = {};
try {
  await webgpuReady();
  for (const api of ['webgpu', 'webgl2']) {
    const page = await browser.newPage();
    page.on('console', (m) => console.log(`${api} console: ${m.text()}`));
    try {
      await page.goto(`${base}/triangle.html?api=${api}`);
      const result = await page.evaluate(() => window.result);
      const path = join(out, `chrome-${api}-triangle.png`);
      await writeFile(path, Buffer.from(result.png.split(',')[1], 'base64'));
      adapters[api] = result.adapter;
      console.log(`${api} on ${JSON.stringify(result.adapter)}: ${path}`);
    } catch (e) {
      console.log(`${api} failed: ${e.message}`);
      failed = true;
    }
    await page.close();
  }
  failed ||= !hardwareUsed(adapters);
  await reportGpu();
} finally {
  await browser.close();
  server.close();
}
process.exitCode = failed ? 1 : 0;

// Whether WebGPU drew on a GPU adapter that is not a fallback one, and WebGL 2 on the same GPU;
// prints why not.
function hardwareUsed({ webgpu, webgl2 }) {
  const problems = [];
  if (webgpu) {
    if (webgpu.fallback) problems.push('WebGPU used a fallback adapter');
    if (!['discrete GPU', 'integrated GPU'].includes(webgpu.type)) problems.push(`WebGPU's adapter is of type ${webgpu.type}, not a GPU`);
    if (!webgpu.description) problems.push('WebGPU gave no adapter description');
    else if (webgl2 && !webgl2.renderer.includes(webgpu.description)) problems.push(`WebGL 2's renderer does not name WebGPU's ${webgpu.description}`);
  }
  for (const problem of problems) console.log(`chrome: ${problem}`);
  return problems.length === 0;
}

// A WebGPU adapter request made just after Chrome starts can fail once while its GPU process
// comes up ("A valid external Instance reference no longer exists"); later ones succeed. This waits
// for the first success, and says how many failed.
async function webgpuReady() {
  const page = await browser.newPage();
  await page.goto(`${base}/triangle.html`);
  for (let attempt = 1; attempt <= 10; attempt++) {
    const found = await page.evaluate(async () => !!(await navigator.gpu?.requestAdapter()));
    if (found) {
      if (attempt > 1) console.log(`chrome: ${attempt - 1} WebGPU adapter requests failed before the GPU process was ready`);
      break;
    }
    await page.waitForTimeout(200);
  }
  await page.close();
}

// Prints the adapters chrome://gpu reports for WebGPU (Dawn) and WebGL (ANGLE).
async function reportGpu() {
  const page = await browser.newPage();
  await page.goto('chrome://gpu');
  await page.waitForFunction(() => document.querySelector('info-view')?.shadowRoot?.textContent.includes(' backend - '));
  const text = await page.evaluate(() => {
    const walk = (node) => node.shadowRoot ? walk(node.shadowRoot) : [...node.childNodes]
      .map((c) => (c.nodeType === 3 ? c.textContent : (/^(BR|LI|DIV|TR|H3)$/.test(c.tagName) ? '\n' : '') + walk(c)))
      .join('');
    return walk(document.body);
  });
  for (const line of text.split('\n')) {
    if (/ backend - |^GL_RENDERER|^\*?\s*WebGPU:|^\*?\s*WebGL2:/.test(line.trim())) console.log(`chrome://gpu: ${line.trim()}`);
  }
  await page.close();
}
