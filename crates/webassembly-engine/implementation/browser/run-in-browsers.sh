#!/usr/bin/env bash
# Runs this crate's tests for wasm32 in headless Chrome and then headless Firefox: `run-in-browsers.sh [cargo test arguments]`.
# WC_BROWSER_TOOLS names a folder holding tools/bin (wasm-bindgen-cli of the version wasm-bindgen is pinned to),
# chromedriver-linux64/chromedriver, geckodriver and firefox/; Chrome itself is the installed google-chrome.
# WASM_BINDGEN_TEST_TIMEOUT is how many seconds the test runner waits for one test target to finish.
set -euo pipefail
tools=${WC_BROWSER_TOOLS:?the folder of the browser tools}
export PATH="$tools/tools/bin:$tools/firefox:$PATH"
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
: "${WASM_BINDGEN_TEST_TIMEOUT:?the seconds the test runner waits for one test target}"
export WASM_BINDGEN_TEST_TIMEOUT
status=0
for browser in chrome firefox; do
  unset CHROMEDRIVER GECKODRIVER
  if [ "$browser" = chrome ]; then
    export CHROMEDRIVER="$tools/chromedriver-linux64/chromedriver"
  else
    export GECKODRIVER="$tools/geckodriver"
  fi
  echo "== $browser"
  cargo test -p wcontainers-webassembly-engine-browser --target wasm32-unknown-unknown "$@" || status=1
done
exit $status
