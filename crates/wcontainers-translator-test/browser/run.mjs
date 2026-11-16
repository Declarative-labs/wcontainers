// Runs one flat blob in Chrome through the browser runner and prints what happened as JSON.
//
// Usage: node run.mjs <case directory>. The directory holds case.json (where things go in guest memory, and
// how many times to run the program in the same instances), blob.bin, stack.bin and translator.wasm. Chrome is the one installed on the machine; playwright-core comes
// from the Nix shell (WC_PLAYWRIGHT_CORE).

import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.WC_PLAYWRIGHT_CORE);

const directory = process.argv[2];
const here = new URL(".", import.meta.url).pathname;
const origin = "https://wcontainers.test";
const files = {
  "/index.html": [join(here, "index.html"), "text/html"],
  "/runner.js": [join(here, "runner.js"), "text/javascript"],
  "/translator.wasm": [join(directory, "translator.wasm"), "application/wasm"],
  "/blob.bin": [join(directory, "blob.bin"), "application/octet-stream"],
  "/stack.bin": [join(directory, "stack.bin"), "application/octet-stream"],
  "/cases.bin": [join(directory, "cases.bin"), "application/octet-stream"],
};
const setup = JSON.parse(await readFile(join(directory, "case.json"), "utf8"));

// V8 flags, only for measuring what the engine does (such as which compiler runs the code).
const jsFlags = process.env.WC_CHROME_JS_FLAGS;
const browser = await chromium.launch({
  channel: "chrome",
  headless: true,
  args: jsFlags ? [`--js-flags=${jsFlags}`] : [],
});
try {
  const page = await browser.newPage();
  page.on("console", (message) => process.stderr.write(`chrome: ${message.text()}\n`));
  // Every file is served from here, with the headers that give the page shared memory.
  await page.route(`${origin}/**`, async (route) => {
    const file = files[new URL(route.request().url()).pathname];
    if (!file) return route.fulfill({ status: 404 });
    await route.fulfill({
      status: 200,
      body: await readFile(file[0]),
      headers: {
        "content-type": file[1],
        "cross-origin-opener-policy": "same-origin",
        "cross-origin-embedder-policy": "require-corp",
      },
    });
  });
  await page.goto(`${origin}/index.html`);
  const result = await page.evaluate(async (setup) => {
    if (!crossOriginIsolated) throw new Error("the page is not cross-origin isolated");
    const bytes = async (path) => new Uint8Array(await (await fetch(path)).arrayBuffer());
    const message = {
      ...setup,
      translatorBytes: await bytes("/translator.wasm"),
      blob: await bytes("/blob.bin"),
      stack: await bytes("/stack.bin"),
      caseBytes: setup.cases ? await bytes("/cases.bin") : null,
    };
    const worker = new Worker("/runner.js", { type: "module" });
    // V8 runs a function first from its baseline compiler and uses optimised code only from the function's
    // next entry, so a loop in a function entered once would stay in baseline code. The page raises the
    // thread's stop flag at 1, 2, 4, ... 1024 ms after the guest starts; after each stop a piece enters itself
    // again at its loop, and so picks up optimised code once V8 has it.
    const answer = await new Promise((resolve, reject) => {
      worker.onmessage = ({ data }) => {
        if (!data.stopFlag) return resolve(data);
        const flag = new Int32Array(data.stopFlag.buffer);
        for (let ms = 1; ms <= 1024; ms *= 2) {
          setTimeout(() => Atomics.store(flag, data.stopFlag.index, 1), ms);
        }
      };
      worker.onerror = (event) => reject(new Error(event.message));
      worker.postMessage(message);
    });
    worker.terminate();
    return answer;
  }, setup);
  process.stdout.write(JSON.stringify({ ...result, userAgent: await page.evaluate(() => navigator.userAgent) }));
} finally {
  await browser.close();
}
