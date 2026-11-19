// The rig's pinned Chrome and a local server for this directory's pages and wasm-bindgen's output,
// shared by draw.mjs, suite.mjs and plain.mjs. Uses the rig's page-driver packages (tools/graphics-rig/browser,
// installed by its install.ps1).
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const rig = createRequire(join(here, '../../../tools/graphics-rig/browser/package.json'));
const { chromium } = rig('playwright-core');
const { getInstalledBrowsers } = await import(pathToFileURL(rig.resolve('@puppeteer/browsers')).href);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

// Serves `/<page>.html` from this directory and `/pkg/...` from `pkg`; resolves once listening.
export async function serve(pkg) {
  const server = createServer(async (request, response) => {
    const path = new URL(request.url, 'http://localhost').pathname;
    const file = /^\/\w+\.html$/.test(path) ? join(here, path.slice(1))
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
  return server;
}

// Thrown where Chrome gave no WebGPU adapter at all: the script then exits with the environment's
// NO_ADAPTER, and check.cmd launches it again.
export class NoAdapter extends Error {}

// A page script's exit code: NO_ADAPTER where Chrome gave no adapter, else 1 if a check failed.
export function exitCode(noAdapter, failed) {
  return noAdapter && process.env.NO_ADAPTER ? Number(process.env.NO_ADAPTER) : failed ? 1 : 0;
}

// How often, and how many milliseconds apart, a page asks for an adapter before giving up.
export const ADAPTER_TRIES = 10;
export const ADAPTER_WAIT_MS = 200;

// Waits for Chrome's GPU process on `page` (an adapter request just after Chrome starts can fail
// while it comes up); throws NoAdapter if no adapter comes.
export async function adapter(page) {
  for (let attempt = 1; attempt <= ADAPTER_TRIES; attempt++) {
    if (await page.evaluate(async () => (await navigator.gpu.requestAdapter()) !== null).catch(() => false)) return;
    await page.waitForTimeout(ADAPTER_WAIT_MS);
  }
  throw new NoAdapter('no WebGPU adapter');
}

// The rig's Chrome, headless. The developer features flag adds the backend, adapter type and full
// name to GPUAdapterInfo, and turns off the rounding of timestamps that Chrome applies by default;
// `developer` false launches Chrome as users have it.
export async function launch(developer = true) {
  const installed = (await getInstalledBrowsers({ cacheDir: 'C:\\wc\\chrome' })).filter((b) => b.browser === 'chrome');
  if (installed.length !== 1) throw new Error(`C:\\wc\\chrome holds ${installed.length} Chrome builds, not the one the rig pins`);
  const browser = await chromium.launch({
    executablePath: installed[0].executablePath,
    headless: true,
    args: developer ? ['--enable-webgpu-developer-features'] : [],
  });
  console.log(`chrome ${browser.version()}`);
  return browser;
}
