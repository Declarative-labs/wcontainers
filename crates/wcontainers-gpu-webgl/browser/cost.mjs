// Measures the per-draw cost of the WebGL 2 and WebGPU back ends in Chrome (cost.html):
//   node cost.mjs <pkg-dir> <out-file> [<frames> [<draw counts, separated by spaces>]]
// <pkg-dir> holds wasm-bindgen's output for examples/draw-cost.rs. Writes the page's lines, with
// Chrome's version, to <out-file>.
import { writeFile } from 'node:fs/promises';
import { serve, launch } from './chrome.mjs';

const [pkg, out, frames = '30', draws = '1000 10000'] = process.argv.slice(2);
if (!out) throw new Error('usage: node cost.mjs <pkg-dir> <out-file> [<frames> [<draw counts>]]');
const server = await serve(pkg);
const browser = await launch();
let result;
try {
  const page = await browser.newPage();
  page.on('console', (m) => console.log(`console: ${m.text()}`));
  await page.goto(`http://127.0.0.1:${server.address().port}/cost.html?frames=${frames}&draws=${encodeURIComponent(draws)}`);
  await page.waitForFunction(() => window.done !== undefined, null, { timeout: 600000 });
  result = await page.evaluate(() => window.done);
  result.lines.unshift(`chrome ${browser.version()}`);
} finally {
  await browser.close();
  server.close();
}
console.log(result.lines.join('\n'));
await writeFile(out, `${result.lines.join('\n')}\n`);
process.exitCode = result.ok ? 0 : 1;
