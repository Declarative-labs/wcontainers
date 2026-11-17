// Compiles converted shaders in the rig's pinned Chrome: every .wgsl file with WebGPU (shader module
// creation and its compilation messages) and every .vert and .frag file with WebGL 2 (compile status),
// on a device with every feature the adapter offers and a context with every extension it offers
// enabled. Writes `chrome.txt` (file, verdict) into each directory and prints the counts.
//   node compile.mjs <dir>...
// Fails unless WebGPU's adapter is a GPU (not a fallback). Uses the rig's page-driver packages
// (tools/graphics-rig/browser, installed by its install.ps1).
import { createServer } from 'node:http';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const rig = createRequire(join(here, '../../../tools/graphics-rig/browser/package.json'));
const { chromium } = rig('playwright-core');
const { getInstalledBrowsers } = await import(pathToFileURL(rig.resolve('@puppeteer/browsers')).href);

const dirs = process.argv.slice(2);
if (!dirs.length) throw new Error('usage: node compile.mjs <dir>...');

const server = createServer((_, response) => {
  response.writeHead(200, { 'content-type': 'text/html' }).end('<!doctype html><canvas id=c></canvas>');
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const installed = (await getInstalledBrowsers({ cacheDir: 'C:\\wc\\chrome' })).filter((b) => b.browser === 'chrome');
if (installed.length !== 1) throw new Error(`C:\\wc\\chrome holds ${installed.length} Chrome builds, not the one the rig pins`);
const browser = await chromium.launch({
  executablePath: installed[0].executablePath,
  headless: true,
  args: ['--enable-webgpu-developer-features'],
});
console.log(`chrome ${browser.version()}`);
let failed = false;
try {
  const page = await browser.newPage();
  page.on('pageerror', (e) => console.log(`page error: ${e.message}`));
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  // An adapter request just after Chrome starts can fail while its GPU process comes up.
  let info;
  for (let attempt = 1; !info && attempt <= 10; attempt++) {
    info = await page.evaluate(async () => {
      const adapter = await navigator.gpu?.requestAdapter();
      if (!adapter) return null;
      window.device = await adapter.requestDevice({ requiredFeatures: [...adapter.features] });
      window.gl = document.getElementById('c').getContext('webgl2');
      const extensions = window.gl.getSupportedExtensions();
      for (const e of extensions) window.gl.getExtension(e);
      const i = adapter.info;
      return {
        webgpu: `${i.vendor} ${i.architecture} ${i.description} type ${i.type} backend ${i.backend}`,
        features: [...adapter.features].sort().join(' '),
        webgl: window.gl.getParameter(window.gl.getExtension('WEBGL_debug_renderer_info')?.UNMASKED_RENDERER_WEBGL ?? window.gl.RENDERER),
        extensions: extensions.join(' '),
        fallback: i.isFallbackAdapter || !['discrete GPU', 'integrated GPU'].includes(i.type),
      };
    }).catch(() => null);
    if (!info) await page.waitForTimeout(200);
  }
  if (!info) throw new Error('no WebGPU adapter');
  console.log(`webgpu: ${info.webgpu}\nfeatures: ${info.features}\nwebgl 2: ${info.webgl}\nextensions: ${info.extensions}`);
  if (info.fallback) throw new Error('the WebGPU adapter is not a GPU');

  for (const dir of dirs) {
    const names = (await readdir(dir)).filter((n) => /\.(wgsl|vert|frag)$/.test(n)).sort();
    const counts = {};
    let lines = '';
    for (const name of names) {
      const src = await readFile(join(dir, name), 'utf8');
      const verdict = await page.evaluate(async ({ name, src }) => {
        if (name.endsWith('.wgsl')) {
          device.pushErrorScope('validation');
          const m = device.createShaderModule({ code: src });
          const info = await m.getCompilationInfo();
          const err = await device.popErrorScope();
          const errs = info.messages.filter((x) => x.type === 'error').map((x) => x.message);
          if (err) errs.push(err.message);
          return errs.length ? `fail ${errs.join(' | ').replace(/\s+/g, ' ').slice(0, 300)}` : 'ok';
        }
        const sh = gl.createShader(name.endsWith('.vert') ? gl.VERTEX_SHADER : gl.FRAGMENT_SHADER);
        gl.shaderSource(sh, src);
        gl.compileShader(sh);
        const ok = gl.getShaderParameter(sh, gl.COMPILE_STATUS);
        const log = ok ? '' : (gl.getShaderInfoLog(sh) || '').replace(/\s+/g, ' ').slice(0, 300);
        gl.deleteShader(sh);
        return ok ? 'ok' : `fail ${log}`;
      }, { name, src });
      const kind = name.endsWith('.wgsl') ? 'webgpu' : 'webgl2';
      const key = `${kind} ${verdict.split(' ')[0]}`;
      counts[key] = (counts[key] ?? 0) + 1;
      lines += `${name}\t${verdict}\n`;
      if (verdict !== 'ok') {
        failed = true;
        console.log(`FAIL ${dir} ${name}: ${verdict.slice(5)}`);
      }
    }
    await writeFile(join(dir, 'chrome.txt'), lines);
    console.log(`${dir}: ${Object.entries(counts).sort().map(([k, n]) => `${k} ${n}`).join(', ')}`);
  }
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
