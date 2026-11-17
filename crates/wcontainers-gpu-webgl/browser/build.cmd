@echo off
rem Builds the WebGL 2 back end's scene programs on the desktop: natively (for ANGLE and the checks) and
rem for WebAssembly with wasm-bindgen's output for Chrome. Run from the checkout root; the GPU half is
rem run.cmd.
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File tools\graphics-rig\browser\install.ps1 || exit /b 1
cargo build -j 3 -p wcontainers-gpu-webgl --example webgl-scenes || exit /b 1
cargo build -j 3 -p wcontainers-gpu-webgl --example browser --target wasm32-unknown-unknown || exit /b 1
C:\wc\tools\wasm-bindgen-0.2.130-x86_64-pc-windows-msvc\wasm-bindgen.exe --target web --out-dir "%CARGO_TARGET_DIR%\webgl-pkg" "%CARGO_TARGET_DIR%\wasm32-unknown-unknown\debug\examples\browser.wasm" || exit /b 1
