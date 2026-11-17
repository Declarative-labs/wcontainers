@echo off
rem Every check of the back end on the desktop, as one GPU job after build.cmd: the scenes on ANGLE and in
rem Chrome's WebGL 2 with every extension and with EXT_clip_control and WEBGL_provoking_vertex switched
rem off, and the shared suite on ANGLE with every extension, without EXT_clip_control, and without both.
rem all.cmd <out-dir>; run from the checkout root.
setlocal
set FAIL=
call crates\wcontainers-gpu-webgl\browser\run.cmd "%~1\with" || set FAIL=1
call crates\wcontainers-gpu-webgl\browser\run.cmd "%~1\without" "EXT_clip_control WEBGL_provoking_vertex" || set FAIL=1
set WEBGL_OFF=
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 || set FAIL=1
set WEBGL_OFF=EXT_clip_control
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 || set FAIL=1
set WEBGL_OFF=EXT_clip_control WEBGL_provoking_vertex
cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1 || set FAIL=1
if defined FAIL exit /b 1
