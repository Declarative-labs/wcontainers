// Draws the three reference scenes (and the quads on depth and stencil) through the WebGPU back end built for WebAssembly, in the rig's
// pinned Chrome, and saves them:
//   node draw.mjs <pkg-dir> <out-dir> [twist...]
// <pkg-dir> is wasm-bindgen's output for the scenes example. For each twist (default none) writes
// chrome-webgpu-<scene>.png into <out-dir>, or <out-dir>\<twist> for a twist other than none, and
// the eight-viewport scene as chrome-webgpu-viewports.png and, moved to whole pixels,
// chrome-webgpu-viewports-rounded.png, prints first-draw timings with and without prepare, and checks
// prepared strips, timestamps, the formats held converted, the vertex formats read through shadows
// and BGRA vertices, query sets past 4096 queries and the query ring, copies between depth16unorm
// and its group, and the cost of instance data fetched in the program.
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
  // Formats held converted: copies exact, texels within their bounds of Direct3D's values (seed 1,
  // 4 textures of 128 texels each).
  try {
    const lines = (await page.evaluate(() => window.converted(1, 4))).split('\n');
    for (const line of lines) console.log(`chrome ${line}`);
  } catch (e) {
    console.log(`chrome converted formats failed: ${e.message}`);
    failed = true;
  }
  // Vertex formats WebGPU lacks, read through shadow streams, and BGRA vertices (seed 188, 64
  // vertices each).
  try {
    console.log(`chrome ${await page.evaluate(() => window.vertexFormats(188, 64))}`);
  } catch (e) {
    console.log(`chrome vertex formats failed: ${e.message}`);
    failed = true;
  }
  // Textures written with Discard behind recorded draws, renamed from the pool.
  try {
    console.log(`chrome ${await page.evaluate(() => window.textureRenames())}`);
  } catch (e) {
    console.log(`chrome texture renames failed: ${e.message}`);
    failed = true;
  }
  // Query sets past WebGPU's 4096 queries a set, and query results from one ring (20 frames).
  try {
    const lines = (await page.evaluate(() => window.querySets())).split('\n');
    for (const line of lines) console.log(`chrome ${line}`);
  } catch (e) {
    console.log(`chrome query sets failed: ${e.message}`);
    failed = true;
  }
  // Copies between depth16unorm and its group's colour formats, whole and in part.
  try {
    console.log(`chrome ${await page.evaluate(() => window.depthGroupCopies())}`);
  } catch (e) {
    console.log(`chrome depth group copies failed: ${e.message}`);
    failed = true;
  }
  // Instance data fetched in the program against vertex buffers (20 frames a route): the routes
  // draw alike; their timings are printed.
  try {
    const lines = (await page.evaluate((n) => window.pulledCosts(n), 20)).split('\n');
    for (const line of lines) console.log(`chrome instance data: ${line}`);
  } catch (e) {
    console.log(`chrome instance data failed: ${e.message}`);
    failed = true;
  }
} finally {
  await browser.close();
  server.close();
}
process.exitCode = failed ? 1 : 0;
