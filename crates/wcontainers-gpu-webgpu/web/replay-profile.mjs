// Where this back end's time goes in Chrome on a real program's frames: replays a recorded call
// stream with the WebGL track's replay page (crates/wcontainers-gpu-webgl/browser/replay.html,
// built by its build.cmd) under V8's sampling profiler, and prints, for the samples taken inside
// this back end, the time under each of its interface calls and the functions the time is spent in.
//   node replay-profile.mjs <pkg-dir> <stream-file> <out-file>
import { copyFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { serve, launch } from '../../wcontainers-gpu-webgl/browser/chrome.mjs';

/// Microseconds between samples.
const SAMPLING_US = 50;
/// Functions listed in each part of the report.
const LISTED = 40;
/// How long the replay may take before the job gives up, in milliseconds.
const LIMIT_MS = 300000;
const OURS = 'wcontainers_gpu_webgpu';
const BACKEND_CALL = /wcontainers_gpu::backend::Backend>::([a-z_]+)/;

const [pkg, stream, out] = process.argv.slice(2);
if (!out) throw new Error('usage: node replay-profile.mjs <pkg-dir> <stream-file> <out-file>');
await copyFile(stream, join(pkg, 'stream.bin'));
const limit = setTimeout(() => {
  console.error('replay-profile: no result in time; stopped');
  process.exit(2);
}, LIMIT_MS);
const server = await serve(pkg);
const browser = await launch();
let result;
let profile;
try {
  const page = await browser.newPage();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: SAMPLING_US });
  await cdp.send('Profiler.start');
  await page.goto(`http://127.0.0.1:${server.address().port}/replay.html`);
  await page.waitForFunction(() => window.done !== undefined, null, { timeout: LIMIT_MS, polling: 1000 });
  result = await page.evaluate(() => window.done);
  ({ profile } = await cdp.send('Profiler.stop'));
} finally {
  await browser.close();
  server.close();
}
clearTimeout(limit);

const nodes = new Map(profile.nodes.map((n) => [n.id, n]));
const parent = new Map();
for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
const name = (n) => n.callFrame.functionName || `(${n.callFrame.url.split('/').pop()})`;
const self = new Map();
const under = new Map();
let inside = 0;
let total = 0;
profile.samples.forEach((id, i) => {
  const ms = (profile.timeDeltas[i] ?? 0) / 1000;
  total += ms;
  const stack = [];
  for (let at = id; at !== undefined; at = parent.get(at)) stack.push(nodes.get(at));
  if (!stack.some((n) => name(n).includes(OURS))) return;
  inside += ms;
  self.set(name(stack[0]), (self.get(name(stack[0])) ?? 0) + ms);
  const call = stack.map((n) => name(n).match(BACKEND_CALL)).filter(Boolean).pop();
  const key = call ? call[1] : '(no interface call)';
  under.set(key, (under.get(key) ?? 0) + ms);
});
const top = (m) => [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, LISTED);
const lines = [
  ...result.lines,
  `profile: ${total.toFixed(1)} ms sampled every ${SAMPLING_US} us, ${inside.toFixed(1)} ms inside this back end`,
  'time under each interface call of this back end:',
  ...top(under).map(([k, ms]) => `  ${ms.toFixed(2)} ms ${k}`),
  'functions the time inside this back end is spent in (self time):',
  ...top(self).map(([k, ms]) => `  ${ms.toFixed(2)} ms ${k}`),
];
console.log(lines.join('\n'));
await writeFile(out, `${lines.join('\n')}\n`);
process.exitCode = result.ok ? 0 : 1;
