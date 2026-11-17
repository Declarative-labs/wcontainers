// Draws the WebGL 2 back end's scenes in Chrome and saves each picture's texels:
//   node run.mjs <pkg-dir> <out-dir> [<extensions to switch off>]
// <pkg-dir> holds wasm-bindgen's output for examples/browser.rs (--target web). Writes
// <scene>[-<change>].rgba (RGBA8, rows top first) and caps-chrome.txt (the renderer, extensions and
// capability list the back end found, and the adapters chrome://gpu lists) into <out-dir>, which
// `webgl-scenes check <out-dir>` then compares with the stored Direct3D pictures.
import { writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { serve, launch, gpuReport } from './chrome.mjs';

const [pkg, out, off = ''] = process.argv.slice(2);
if (!out) throw new Error('usage: node run.mjs <pkg-dir> <out-dir> [<extensions to switch off>]');
await mkdir(out, { recursive: true });
const server = await serve(pkg);
const browser = await launch();
let failed = false;
try {
  const page = await browser.newPage();
  page.on('console', (m) => console.log(`console: ${m.text()}`));
  page.on('pageerror', (e) => { console.log(`page error: ${e.message}`); failed = true; });
  await page.goto(`http://127.0.0.1:${server.address().port}/page.html?off=${encodeURIComponent(off)}`);
  await page.waitForFunction(() => window.ready === true);
  let report = await page.evaluate(() => window.report());
  console.log(report.split('\n')[0]);
  for (const [scene, change = ''] of await page.evaluate(() => window.pictures())) {
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
