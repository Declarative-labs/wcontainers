// Draws the three reference scenes (and the quads on depth and stencil) through the WebGPU back end built for WebAssembly, in the rig's
// pinned Chrome, and saves them:
//   node draw.mjs <pkg-dir> <out-dir> [twist...]
// <pkg-dir> is wasm-bindgen's output for the scenes example. For each twist (default none) writes
// chrome-webgpu-<scene>.png into <out-dir>, or <out-dir>\<twist> for a twist other than none.
// Fails unless WebGPU's adapter is a GPU (discrete or integrated, not a fallback). Uses the rig's
// page-driver packages (tools/graphics-rig/browser, installed by its install.ps1).
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const rig = createRequire(join(here, '../../../tools/graphics-rig/browser/package.json'));
const { chromium } = rig('playwright-core');
const { getInstalledBrowsers } = await import(pathToFileURL(rig.resolve('@puppeteer/browsers')).href);

const [pkg, out, ...given] = process.argv.slice(2);
if (!out) throw new Error('usage: node draw.mjs <pkg-dir> <out-dir> [twist...]');
const twists = given.length ? given : ['none'];
const scenes = ['triangle', 'quads', 'instanced', 'quads-d24s8'];
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

const server = createServer(async (request, response) => {
  const path = new URL(request.url, 'http://localhost').pathname;
  const file = path === '/scenes.html' ? join(here, 'scenes.html')
    : path.startsWith('/pkg/') && !path.includes('..') ? join(pkg, path.slice(5)) : null;
  try {
    if (!file) throw new Error('not found');
    const body = await readFile(file);
    response.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' }).end(body);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

const installed = (await getInstalledBrowsers({ cacheDir: 'C:\\wc\\chrome' })).filter((b) => b.browser === 'chrome');
if (installed.length !== 1) throw new Error(`C:\\wc\\chrome holds ${installed.length} Chrome builds, not the one the rig pins`);
// The developer features flag only adds the backend, adapter type and full name to GPUAdapterInfo.
const browser = await chromium.launch({
  executablePath: installed[0].executablePath,
  headless: true,
  args: ['--enable-webgpu-developer-features'],
});
console.log(`chrome ${browser.version()}`);
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
} finally {
  await browser.close();
  server.close();
}
process.exitCode = failed ? 1 : 0;
