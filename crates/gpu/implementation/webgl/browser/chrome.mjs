// What run.mjs and suite.mjs share: serving this directory's pages and wasm-bindgen's output, launching
// the rig's Chrome with a working WebGL 2, and reading chrome://gpu. Uses the Chrome build and the
// playwright-core package the graphics rig installs (tools/graphics-rig/browser); where CHROME names a
// Chrome or Chromium executable (the rented Linux machines, which have no GPU), that one, drawing WebGL 2
// on SwiftShader, and the playwright-core package whose package.json PLAYWRIGHT_PACKAGE names.
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { readFile, readdir } from 'node:fs/promises';
import { join, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(process.env.PLAYWRIGHT_PACKAGE ?? join(here, '../../../../../tools/graphics-rig/browser/package.json'));
const { chromium } = require('playwright-core');

// Serves <name>.html and <name>.js from this directory and /pkg/ from <pkg>; resolves to the server.
// Pages are cross-origin isolated, so performance.now() reads to 5 us rather than 100 us.
export async function serve(pkg) {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
  const server = createServer(async (request, response) => {
    const path = new URL(request.url, 'http://localhost').pathname;
    const file = /^\/[a-z]+\.(html|js)$/.test(path) ? join(here, path.slice(1)) : path.startsWith('/pkg/') ? join(pkg, path.slice(5)) : null;
    if (!file) return response.writeHead(404).end();
    try {
      response.writeHead(200, {
        'content-type': types[extname(file)] ?? 'application/octet-stream',
        'cross-origin-opener-policy': 'same-origin',
        'cross-origin-embedder-policy': 'require-corp',
      }).end(await readFile(file));
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return server;
}

// Launches the one Chrome build the rig pins in C:\wc\chrome, headless, once a WebGL 2 context
// survives in it. Chrome's GPU process can restart just after Chrome starts and lose the contexts made
// before it settles; a launch where none survives in ten tries is closed and Chrome launched again,
// up to three times.
export async function launch() {
  // The developer features flag only adds the backend, adapter type and full name to
  // GPUAdapterInfo, so the cost page can confirm WebGPU drew on a GPU.
  const args = ['--enable-webgpu-developer-features'];
  let executablePath = process.env.CHROME;
  if (executablePath) {
    // No GPU and no setuid sandbox helper on the build machines: ANGLE on SwiftShader, Chrome's own
    // CPU renderer, which Chrome takes for WebGL only when asked.
    args.push('--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--no-sandbox');
  } else {
    const root = 'C:\\wc\\chrome\\chrome';
    const builds = (await readdir(root)).filter((d) => d.startsWith('win64-'));
    if (builds.length !== 1) throw new Error(`${root} holds ${builds.length} Chrome builds, not one`);
    executablePath = join(root, builds[0], 'chrome-win64', 'chrome.exe');
  }
  for (let launches = 1; launches <= 3; launches++) {
    const browser = await chromium.launch({ executablePath, headless: true, args });
    const lost = await gpuReady(browser);
    if (lost < 10) {
      console.log(`chrome ${browser.version()}${lost ? `: ${lost} WebGL contexts were lost while the GPU process started` : ''}`);
      return browser;
    }
    console.log(`chrome: no WebGL 2 context survived half a second in ten tries; launch ${launches} closed`);
    await browser.close();
  }
  throw new Error('no launch of Chrome kept a WebGL 2 context in three');
}

// How many WebGL 2 contexts were lost (or not made) before one survived half a second; 10 if none did.
async function gpuReady(browser) {
  const page = await browser.newPage();
  try {
    for (let attempt = 0; attempt < 10; attempt++) {
      const survived = await page.evaluate(async () => {
        const gl = document.createElement('canvas').getContext('webgl2');
        if (!gl) return false;
        gl.clear(gl.COLOR_BUFFER_BIT);
        await new Promise((resolve) => setTimeout(resolve, 500));
        return !gl.isContextLost();
      });
      if (survived) return attempt;
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    return 10;
  } finally {
    await page.close();
  }
}

// The adapters chrome://gpu reports for WebGL (ANGLE) and WebGPU (Dawn), as lines.
export async function gpuReport(browser) {
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
