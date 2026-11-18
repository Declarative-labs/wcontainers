@echo off
rem some.cmd <out-dir> "<names>": the tests of the suite and the checks whose names hold any of the
rem space-separated <names>, on ANGLE and in Chrome with every extension. A GPU job, after build.cmd.
setlocal
set OUT=%~1
if not exist "%OUT%" mkdir "%OUT%"
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --list --format terse | findstr "%~2" > "%OUT%\list.txt"
type "%OUT%\list.txt"
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 %~2
node crates\wcontainers-gpu-webgl\browser\suite.mjs "%CARGO_TARGET_DIR%\webgl-pkg" "%OUT%\chrome.txt" "%OUT%\list.txt"
type "%OUT%\chrome.txt"
