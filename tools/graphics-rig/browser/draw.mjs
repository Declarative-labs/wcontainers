// Draws the triangle in Chrome through WebGPU and WebGL 2 and saves the pictures:
//   node draw.mjs <out-dir>
// writes chrome-webgpu-triangle.png and chrome-webgl2-triangle.png and prints the adapter each API
// reports, and the adapters chrome://gpu lists for WebGPU and WebGL, so a fall back to software
// drawing shows. Chrome is the newest one in C:\wc\chrome.
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getInstalledBrowsers } from '@puppeteer/browsers';
import { chromium } from 'playwright-core';

const here = dirname(fileURLToPath(import.meta.url));
const files = {
  '/triangle.html': [join(here, 'triangle.html'), 'text/html'],
  '/triangle.wgsl': [join(here, '../../../crates/wcontainers-graphics-test/src/triangle.wgsl'), 'text/plain'],
};
const out = process.argv[2];
if (!out) throw new Error('usage: node draw.mjs <out-dir>');
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
      console.log(`${api} on ${result.adapter}: ${path}`);
    } catch (e) {
      console.log(`${api} failed: ${e.message}`);
      failed = true;
    }
    await page.close();
  }
  await reportGpu();
} finally {
  await browser.close();
  server.close();
}
process.exitCode = failed ? 1 : 0;

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
