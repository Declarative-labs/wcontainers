// Replays a recorded call stream (src/trace.rs) in Chrome on the WebGL 2 and the WebGPU back ends
// (replay.html) and writes where each frame's time goes:
//   node replay.mjs <pkg-dir> <stream-file> <out-file> [<frames skipped>]
// <pkg-dir> holds wasm-bindgen's output for examples/replay.rs; the stream is copied into it.
import { copyFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { serve, launch } from './chrome.mjs';

const [pkg, stream, out, skip = '10'] = process.argv.slice(2);
if (!out) throw new Error('usage: node replay.mjs <pkg-dir> <stream-file> <out-file> [<frames skipped>]');
await copyFile(stream, join(pkg, 'stream.bin'));
const server = await serve(pkg);
const browser = await launch();
let result;
try {
  const page = await browser.newPage();
  page.on('console', (m) => console.log(`console: ${m.text()}`));
  await page.goto(`http://127.0.0.1:${server.address().port}/replay.html?skip=${skip}`);
  await page.waitForFunction(() => window.done !== undefined, null, { timeout: 600000, polling: 1000 });
  result = await page.evaluate(() => window.done);
  result.lines.unshift(`chrome ${browser.version()}, stream ${stream}`);
} finally {
  await browser.close();
  server.close();
}
console.log(result.lines.join('\n'));
await writeFile(out, `${result.lines.join('\n')}\n`);
process.exitCode = result.ok ? 0 : 1;
