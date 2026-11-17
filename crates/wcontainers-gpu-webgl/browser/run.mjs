// Draws the WebGL 2 back end's scenes in Chrome and saves each picture's texels:
//   node run.mjs <pkg-dir> <out-dir> [<extensions to switch off>]
// <pkg-dir> holds wasm-bindgen's output for examples/browser.rs (--target web). Writes
// <scene>[-<change>].rgba (RGBA8, rows top first) and caps-chrome.txt (the renderer, extensions and
// capability list the back end found, and the adapters chrome://gpu lists) into <out-dir>, which
// `webgl-scenes check <out-dir>` then compares with the stored Direct3D pictures. Uses the Chrome build
// and the playwright-core package the graphics rig installs (tools/graphics-rig/browser).
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../../../tools/graphics-rig/browser/package.json'));
const { chromium } = require('playwright-core');
const [pkg, out, off = ''] = process.argv.slice(2);
if (!out) throw new Error('usage: node run.mjs <pkg-dir> <out-dir> [<extensions to switch off>]');
await mkdir(out, { recursive: true });

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const server = createServer(async (request, response) => {
  const path = new URL(request.url, 'http://localhost').pathname;
  const file = path === '/page.html' ? join(here, 'page.html') : path.startsWith('/pkg/') ? join(pkg, path.slice(5)) : null;
  if (!file) return response.writeHead(404).end();
  try {
    response.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' }).end(await readFile(file));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

// The one Chrome build the rig pins in C:\wc\chrome.
const root = 'C:\\wc\\chrome\\chrome';
const builds = (await readdir(root)).filter((d) => d.startsWith('win64-'));
if (builds.length !== 1) throw new Error(`${root} holds ${builds.length} Chrome builds, not one`);
const browser = await chromium.launch({ executablePath: join(root, builds[0], 'chrome-win64', 'chrome.exe'), headless: true });
console.log(`chrome ${browser.version()}`);
const scenes = [['triangle', ''], ['quads', ''], ['instanced', ''], ['quads', 'blend_factor'], ['quads', 'depth_function']];
let failed = false;
try {
  await gpuReady(browser);
  const page = await browser.newPage();
  page.on('console', (m) => console.log(`console: ${m.text()}`));
  page.on('pageerror', (e) => { console.log(`page error: ${e.message}`); failed = true; });
  await page.goto(`http://127.0.0.1:${server.address().port}/page.html?off=${encodeURIComponent(off)}`);
  await page.waitForFunction(() => window.ready === true);
  let report = await page.evaluate(() => window.report());
  console.log(report.split('\n')[0]);
  for (const [scene, change] of scenes) {
    const name = change ? `${scene}-${change}` : scene;
    try {
      const texels = await page.evaluate(([s, c]) => window.run(s, c), [scene, change]);
      await writeFile(join(out, `${name}.rgba`), Buffer.from(texels, 'base64'));
    } catch (e) {
      console.log(`${name} failed: ${e.message}`);
      failed = true;
    }
  }
  report += await gpuReport(browser);
  await writeFile(join(out, 'caps-chrome.txt'), report);
} finally {
  await browser.close();
  server.close();
}
process.exitCode = failed ? 1 : 0;

// Chrome's GPU process can restart just after Chrome starts, losing the WebGL contexts made before it
// settles. This waits until a context survives half a second, and says how many did not.
async function gpuReady(browser) {
  const page = await browser.newPage();
  for (let attempt = 1; attempt <= 10; attempt++) {
    const survived = await page.evaluate(async () => {
      const gl = document.createElement('canvas').getContext('webgl2');
      gl.clear(gl.COLOR_BUFFER_BIT);
      await new Promise((resolve) => setTimeout(resolve, 500));
      return !gl.isContextLost();
    });
    if (survived) {
      if (attempt > 1) console.log(`chrome: ${attempt - 1} WebGL contexts were lost while the GPU process started`);
      break;
    }
  }
  await page.close();
}

// The adapters chrome://gpu reports for WebGL (ANGLE) and WebGPU (Dawn).
async function gpuReport(browser) {
  const page = await browser.newPage();
  await page.goto('chrome://gpu');
  await page.waitForFunction(() => document.querySelector('info-view')?.shadowRoot?.textContent.includes('GL_RENDERER'));
  const text = await page.evaluate(() => {
    const walk = (node) => node.shadowRoot ? walk(node.shadowRoot) : [...node.childNodes]
      .map((c) => (c.nodeType === 3 ? c.textContent : (/^(BR|LI|DIV|TR|H3)$/.test(c.tagName) ? '\n' : '') + walk(c)))
      .join('');
    return walk(document.body);
  });
  await page.close();
  const lines = text.split('\n').map((l) => l.trim()).filter((l) => /^GL_RENDERER|^GL_VENDOR|^\*?\s*WebGL2?:|^\*?\s*WebGPU:/.test(l));
  for (const l of lines) console.log(`chrome://gpu: ${l}`);
  return lines.map((l) => `chrome://gpu: ${l}\n`).join('');
}
