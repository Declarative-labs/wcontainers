// Node against the system: timers, child processes, worker threads, file streams and an HTTP
// server and client on loopback. Everything printed is the same on every run.
'use strict';
const assert = require('assert');
const childProcess = require('child_process');
const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { pipeline } = require('stream/promises');
const { Worker } = require('worker_threads');

if (process.argv[2] === 'child') {
  process.on('message', (message) => {
    process.send({ doubled: message.value * 2 });
    process.disconnect();
  });
  return;
}

async function timers() {
  const order = [];
  await new Promise((resolve) => {
    setTimeout(() => order.push('timeout 200'), 200);
    setTimeout(() => order.push('timeout 0'), 0);
    setImmediate(() => order.push('immediate'));
    process.nextTick(() => order.push('tick'));
    Promise.resolve().then(() => order.push('microtask'));
    let count = 0;
    const interval = setInterval(() => {
      count += 1;
      if (count === 3) {
        clearInterval(interval);
        order.push('interval 3');
        setTimeout(resolve, 300);
      }
    }, 5);
  });
  // A timeout and an immediate queued from the main module race on Linux too, so their order is
  // not shown.
  console.log('timers', order.filter((name) => name !== 'timeout 0' && name !== 'immediate'));
  console.log('timers ran', order.length);
  // libuv's timers count from the loop's cached time in whole milliseconds, so a sleep measured
  // with the high resolution clock can come out a little short on Linux; only its end is shown.
  await new Promise((resolve) => setTimeout(resolve, 50));
  console.log('slept');
}

async function children() {
  const out = childProcess.execFileSync('/bin/sh', ['-c', 'echo from shell; echo to error >&2'], {
    stdio: ['ignore', 'pipe', 'ignore'],
  });
  console.log('exec', out.toString().trim());
  const result = childProcess.spawnSync('/bin/sh', ['-c', 'exit 4']);
  console.log('exit status', result.status);
  const sorted = await new Promise((resolve) => {
    const sort = childProcess.spawn('sort');
    let text = '';
    sort.stdout.on('data', (chunk) => { text += chunk; });
    sort.on('close', (code) => resolve([text.split('\n').filter(Boolean), code]));
    sort.stdin.end('pear\napple\nfig\n');
  });
  console.log('sorted', sorted);
  const forked = await new Promise((resolve) => {
    const child = childProcess.fork(__filename, ['child']);
    child.on('message', resolve);
    child.send({ value: 21 });
  });
  console.log('forked child answered', forked);
  const killed = await new Promise((resolve) => {
    const sleeper = childProcess.spawn('/bin/sleep', ['60']);
    sleeper.on('exit', (code, signal) => resolve(signal));
    sleeper.kill('SIGTERM');
  });
  console.log('killed child signal', killed);
}

async function workers() {
  const shared = new SharedArrayBuffer(4 * 4);
  const counters = new Int32Array(shared);
  const source = `
    const { parentPort, workerData } = require('worker_threads');
    const counters = new Int32Array(workerData.shared);
    for (let i = 0; i < 10000; i++) Atomics.add(counters, 0, 1);
    let sum = 0;
    for (let i = 0; i <= workerData.n; i++) sum += i;
    Atomics.add(counters, 1, 1);
    Atomics.notify(counters, 1);
    parentPort.postMessage({ n: workerData.n, sum });
  `;
  const answers = await Promise.all([100, 200, 300, 400].map((n) => new Promise((resolve, reject) => {
    const worker = new Worker(source, { eval: true, workerData: { shared, n } });
    worker.on('message', resolve);
    worker.on('error', reject);
  })));
  console.log('workers', answers.map((answer) => answer.sum));
  console.log('atomic count', Atomics.load(counters, 0), 'finished', Atomics.load(counters, 1));
}

async function files() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'wc-'));
  const source = path.join(root, 'source');
  const block = Buffer.alloc(64 * 1024, 'abcdefgh');
  const writer = fs.createWriteStream(source);
  for (let i = 0; i < 32; i++) {
    if (!writer.write(block)) await new Promise((resolve) => writer.once('drain', resolve));
  }
  await new Promise((resolve) => writer.end(resolve));
  const copy = path.join(root, 'copy');
  await pipeline(fs.createReadStream(source), fs.createWriteStream(copy));
  const hash = (file) => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  console.log('copied', fs.statSync(copy).size, hash(source) === hash(copy));
  const lines = path.join(root, 'lines');
  fs.writeFileSync(lines, Array.from({ length: 1000 }, (_, i) => `line ${i}`).join('\n'));
  let count = 0;
  for await (const chunk of fs.createReadStream(lines, { encoding: 'utf8', highWaterMark: 1024 })) {
    count += chunk.split('line').length - 1;
  }
  console.log('lines read', count);
  fs.mkdirSync(path.join(root, 'a', 'b'), { recursive: true });
  fs.symlinkSync('../source', path.join(root, 'a', 'link'));
  console.log('listing', fs.readdirSync(path.join(root, 'a')).sort());
  console.log('link size', fs.statSync(path.join(root, 'a', 'link')).size);
  const watched = await new Promise((resolve) => {
    const watcher = fs.watch(path.join(root, 'a'), (event, name) => {
      watcher.close();
      resolve([event, name]);
    });
    fs.writeFileSync(path.join(root, 'a', 'new'), 'x');
  });
  console.log('watched', watched);
  fs.rmSync(root, { recursive: true });
  console.log('removed', !fs.existsSync(root));
}

async function web() {
  const server = http.createServer((request, response) => {
    let body = '';
    request.on('data', (chunk) => { body += chunk; });
    request.on('end', () => {
      response.setHeader('content-type', 'text/plain');
      response.end(`${request.method} ${request.url} ${body.length}\n`.repeat(100));
    });
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address();
  const agent = new http.Agent({ keepAlive: true, maxSockets: 2 });
  const ask = (method, url, body) => new Promise((resolve, reject) => {
    const request = http.request({ host: '127.0.0.1', port, method, path: url, agent }, (response) => {
      let text = '';
      response.on('data', (chunk) => { text += chunk; });
      response.on('end', () => resolve([response.statusCode, text.length, text.split('\n')[0]]));
    });
    request.on('error', reject);
    request.end(body);
  });
  const answers = await Promise.all([
    ask('GET', '/one'),
    ask('POST', '/two', 'x'.repeat(100000)),
    ask('PUT', '/three', 'y'),
    ask('GET', '/four'),
  ]);
  console.log('http', answers);
  agent.destroy();
  await new Promise((resolve) => server.close(resolve));
  console.log('server closed');
}

(async () => {
  console.log('node', process.version, os.platform(), os.arch());
  await timers();
  await children();
  await workers();
  await files();
  await web();
  console.log('done');
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
