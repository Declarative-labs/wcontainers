// Timestamps in the rig's pinned Chrome as users have it, without developer features (which turn
// off Chrome's rounding of timestamps): ordered and scaled with the work, and each a multiple of
// the step printed. Its own process: a second Chrome launched from one node process could not get
// an adapter.
//   node plain.mjs <pkg-dir>
import { serve, launch, adapter, NoAdapter, exitCode } from './chrome.mjs';

const [pkg] = process.argv.slice(2);
if (!pkg) throw new Error('usage: node plain.mjs <pkg-dir>');
const server = await serve(pkg);
const browser = await launch(false);
let failed = false;
let noAdapter = false;
try {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}/scenes.html`);
  await adapter(page);
  let line;
  for (let attempt = 1; !line && attempt <= 10; attempt++) {
    line = await page.evaluate(() => window.timestamps()).catch((e) => {
      if (attempt === 10) throw e;
      return null;
    });
    if (!line) await page.waitForTimeout(500);
  }
  console.log(`chrome timestamps without developer features: ${line}`);
} catch (e) {
  noAdapter = e instanceof NoAdapter;
  console.log(`chrome timestamps without developer features failed: ${e.message}`);
  failed = true;
} finally {
  await browser.close();
  server.close();
}
process.exitCode = exitCode(noAdapter, failed);
