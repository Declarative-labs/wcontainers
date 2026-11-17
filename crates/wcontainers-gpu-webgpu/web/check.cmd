@echo off
rem On the desktop, as a GPU job (run by web\check from the Mac): draws the three reference scenes,
rem and the quads again on depth24plus-stencil8, through the WebGPU back end natively (wgpu on
rem Direct3D 12 and on Vulkan) and in Chrome on
rem WebGPU (the back end built for WebAssembly), compares every picture with real Direct3D 11's at
rem tolerance 0, and checks that the quads with a changed blend factor or depth function differ from
rem it both ways, and runs the shared suite in Chrome (suite.mjs). Exits 1 if any check fails.
rem   check.cmd <name>   writes C:\wc\out\<name>-webgpu
setlocal
set BIN=%CARGO_TARGET_DIR%\debug
set OUT=C:\wc\out\%~1-webgpu
set REF=crates\wcontainers-graphics-test\references\d3d11
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%" || exit /b 2
set FAILED=0
wasm-bindgen --target web --out-dir "%OUT%\pkg" "%CARGO_TARGET_DIR%\wasm32-unknown-unknown\debug\examples\scenes.wasm" || exit /b 2
for %%a in (dx12 vulkan) do "%BIN%\examples\scenes" %%a "%OUT%" || set FAILED=1
for %%t in (blend-factor depth-function) do (
    for %%a in (dx12 vulkan) do "%BIN%\examples\scenes" %%a "%OUT%\%%t" %%t || set FAILED=1
)
node crates\wcontainers-gpu-webgpu\web\draw.mjs "%OUT%\pkg" "%OUT%" none blend-factor depth-function || set FAILED=1
cargo test --locked -j 3 -p wcontainers-gpu-webgpu --test suite -- --list --format terse > "%OUT%\suite-list.txt" || set FAILED=1
node crates\wcontainers-gpu-webgpu\web\suite.mjs "%OUT%\pkg" "%OUT%\suite-chrome.txt" "%OUT%\suite-list.txt" || set FAILED=1
for %%s in (triangle quads instanced) do (
    for %%r in (webgpu-dx12 webgpu-vulkan chrome-webgpu) do call :same %%r-%%s.png %%s.png
)
for %%r in (webgpu-dx12 webgpu-vulkan chrome-webgpu) do call :same %%r-quads-d24s8.png quads.png
for %%t in (blend-factor depth-function) do (
    for %%r in (webgpu-dx12 webgpu-vulkan chrome-webgpu) do (
        call :differs %%t\%%r-quads.png
        call :differs %%t\%%r-quads-d24s8.png
    )
)
exit /b %FAILED%

:same
<nul set /p "=%1 against Direct3D 11's %2: "
"%BIN%\gt-compare" "%OUT%\%1" "%REF%\%2" 0 || set FAILED=1
exit /b

:differs
<nul set /p "=%1 must differ from Direct3D 11's quads: "
"%BIN%\gt-compare" "%OUT%\%1" "%REF%\quads.png" 0
rem gt-compare exits 1 when the pictures differ, 2 when it could not compare them.
if errorlevel 2 (set FAILED=1) else if not errorlevel 1 set FAILED=1
exit /b
