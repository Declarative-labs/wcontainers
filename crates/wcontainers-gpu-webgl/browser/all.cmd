@echo off
rem Every check of the back end on the desktop, as one GPU job after build.cmd: the scenes on ANGLE and in
rem Chrome's WebGL 2 with every extension and with EXT_clip_control and WEBGL_provoking_vertex switched
rem off, and the shared suite in Chrome and on ANGLE with every extension, without EXT_clip_control, and
rem without both. all.cmd <out-dir>; run from the checkout root.
setlocal
set OUT=%~1
set FAIL=
if not exist "%OUT%" mkdir "%OUT%"
call crates\wcontainers-gpu-webgl\browser\run.cmd "%OUT%\with" || set FAIL=1
call crates\wcontainers-gpu-webgl\browser\run.cmd "%OUT%\without" "EXT_clip_control WEBGL_provoking_vertex" || set FAIL=1
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --list --format terse > "%OUT%\suite-list.txt" || set FAIL=1
set SUITE=node crates\wcontainers-gpu-webgl\browser\suite.mjs "%CARGO_TARGET_DIR%\webgl-pkg"
%SUITE% "%OUT%\suite-chrome.txt" "%OUT%\suite-list.txt" || set FAIL=1
%SUITE% "%OUT%\suite-chrome-noclip.txt" "%OUT%\suite-list.txt" "EXT_clip_control" || set FAIL=1
%SUITE% "%OUT%\suite-chrome-neither.txt" "%OUT%\suite-list.txt" "EXT_clip_control WEBGL_provoking_vertex" || set FAIL=1
set WEBGL_OFF=
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 || set FAIL=1
set WEBGL_OFF=EXT_clip_control
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 || set FAIL=1
set WEBGL_OFF=EXT_clip_control WEBGL_provoking_vertex
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 || set FAIL=1
if defined FAIL exit /b 1
