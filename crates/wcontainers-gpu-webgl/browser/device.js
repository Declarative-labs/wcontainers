// The device thread's host import (module `wcontainers-device`): `next_task` suspends the WebAssembly
// stack through JSPI until the browser's next task, when WebGL hands out query results.
export const next_task = new WebAssembly.Suspending(() => new Promise((resolve) => setTimeout(resolve, 0)));
