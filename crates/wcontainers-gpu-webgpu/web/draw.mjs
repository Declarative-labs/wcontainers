// Draws the three reference scenes (and the quads on depth and stencil) through the WebGPU back end built for WebAssembly, in the rig's
// pinned Chrome, and saves them:
//   node draw.mjs <pkg-dir> <out-dir> [twist...]
// <pkg-dir> is wasm-bindgen's output for the scenes example. For each twist (default none) writes
// chrome-webgpu-<scene>.png into <out-dir>, or <out-dir>\<twist> for a twist other than none, and
// the eight-viewport scene as chrome-webgpu-viewports.png and, moved to whole pixels,
// chrome-webgpu-viewports-rounded.png, prints first-draw timings with and without prepare, and checks
// timestamps.
// Fails unless WebGPU's adapter is a GPU (discrete or integrated, not a fallback).
import { writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { serve, launch } from './chrome.mjs';

const [pkg, out, ...given] = process.argv.slice(2);
if (!out) throw new Error('usage: node draw.mjs <pkg-dir> <out-dir> [twist...]');
const twists = given.length ? given : ['none'];
const scenes = ['triangle', 'quads', 'instanced', 'quads-d24s8'];
const server = await serve(pkg);
const browser = await launch();
let failed = false;
try {
  const page = await browser.newPage();
  page.on('console', (m) => console.log(`page: ${m.text()}`));
  page.on('pageerror', (e) => console.log(`page error: ${e.message}`));
  await page.goto(`http://127.0.0.1:${server.address().port}/scenes.html`);
  // An adapter request just after Chrome starts can fail while its GPU process comes up.
  let info;
  for (let attempt = 1; !info && attempt <= 10; attempt++) {
    info = await page.evaluate(() => window.adapterInfo()).catch(() => null);
    if (!info) await page.waitForTimeout(200);
  }
  if (!info) throw new Error('no WebGPU adapter');
  console.log(`webgpu adapter: ${JSON.stringify(info)}`);
  if (info.fallback || !['discrete GPU', 'integrated GPU'].includes(info.type)) {
    console.log('chrome: WebGPU did not use a GPU adapter');
    failed = true;
  }
  for (const twist of twists) {
    const dir = twist === 'none' ? out : join(out, twist);
    await mkdir(dir, { recursive: true });
    for (const scene of scenes) {
      try {
        const url = await page.evaluate(([s, t]) => window.drawScene(s, t), [scene, twist]);
        const path = join(dir, `chrome-webgpu-${scene}.png`);
        await writeFile(path, Buffer.from(url.split(',')[1], 'base64'));
        console.log(path);
      } catch (e) {
        console.log(`${scene} (${twist}) failed: ${e.message}`);
        failed = true;
      }
    }
  }
  // The eight-viewport scene, once at half pixels and once moved to whole pixels.
  for (const rounded of [false, true]) {
    try {
      const url = await page.evaluate((r) => window.drawViewports(r), rounded);
      const path = join(out, `chrome-webgpu-viewports${rounded ? '-rounded' : ''}.png`);
      await writeFile(path, Buffer.from(url.split(',')[1], 'base64'));
      console.log(path);
    } catch (e) {
      console.log(`viewports (rounded ${rounded}) failed: ${e.message}`);
      failed = true;
    }
  }
  // First draws with and without prepare: the prepared half must build nothing while drawing.
  try {
    const lines = (await page.evaluate((n) => window.prepareTimings(n), 16)).split('\n');
    for (const line of lines) console.log(`chrome prepare: ${line}`);
    if (!lines[1]?.endsWith(' 0 misses, 0 pipelines built while drawing')) {
      console.log('chrome prepare: the prepared draws built pipelines');
      failed = true;
    }
  } catch (e) {
    console.log(`prepare timings failed: ${e.message}`);
    failed = true;
  }
  // Timestamps: ordered, scaled with the work, at the page clock's rate.
  try {
    console.log(`chrome timestamps: ${await page.evaluate(() => window.timestamps())}`);
  } catch (e) {
    console.log(`chrome timestamps failed: ${e.message}`);
    failed = true;
  }
} finally {
  await browser.close();
  server.close();
}
process.exitCode = failed ? 1 : 0;
