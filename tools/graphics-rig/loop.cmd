@echo off
rem The whole loop on the desktop, run as a GPU job by `loop`: every scene drawn by real Direct3D 11
rem and 12 on the GPU and on WARP, the triangle through wgpu (Direct3D 12 and Vulkan), through Chrome
rem (WebGPU and WebGL 2) and through the forwarding d3d11.dll, then each picture against the
rem references. A picture from the GPU must equal the GPU's Direct3D 11 picture; WARP may differ
rem from the GPU by 1 in a channel, which is what was measured. The forwarding run must also log the
rem program's device creation (D3D11CreateDevice or D3D11CreateDeviceAndSwapChain) going through the
rem replacement, which proves it was loaded. Exits 1 if any check fails.
rem   loop.cmd <name>   writes C:\wc\out\<name>
setlocal
set BIN=%CARGO_TARGET_DIR%\debug
set OUT=C:\wc\out\%~1
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%\forward" || exit /b 2
"%BIN%\gt-d3d11" all hardware "%OUT%" || exit /b 2
"%BIN%\gt-d3d11" all warp "%OUT%" || exit /b 2
"%BIN%\gt-d3d12" hardware "%OUT%" || exit /b 2
"%BIN%\gt-d3d12" warp "%OUT%" || exit /b 2
"%BIN%\gt-wgpu" dx12 "%OUT%" || exit /b 2
"%BIN%\gt-wgpu" vulkan "%OUT%" || exit /b 2
"%BIN%\gt-scene" triangle > "%OUT%\triangle.json" || exit /b 2
node tools\graphics-rig\browser\draw.mjs "%OUT%\triangle.json" "%OUT%" || exit /b 2
copy /y "%BIN%\gt-d3d11.exe" "%OUT%\forward" >nul || exit /b 2
copy /y "%BIN%\wcontainers_graphics_test_forward.dll" "%OUT%\forward\d3d11.dll" >nul || exit /b 2
"%OUT%\forward\gt-d3d11" triangle hardware "%OUT%\forward" 2> "%OUT%\forward\calls.log"
set CODE=%ERRORLEVEL%
type "%OUT%\forward\calls.log"
if not %CODE%==0 exit /b 2
copy /y "%OUT%\forward\d3d11-hardware-triangle.png" "%OUT%\forward-d3d11-triangle.png" >nul

set FAILED=0
findstr /b /c:"d3d11 forward: D3D11CreateDevice" "%OUT%\forward\calls.log" >nul || (echo the forwarding run did not create its device through the replacement d3d11.dll& set FAILED=1)
for %%s in (triangle quads instanced) do call :compare d3d11-hardware-%%s d3d11-warp-%%s 1
call :compare d3d12-hardware-triangle d3d12-warp-triangle 1
for %%p in (d3d12-hardware wgpu-dx12 wgpu-vulkan chrome-webgpu chrome-webgl2 forward-d3d11) do call :compare %%p-triangle d3d11-hardware-triangle 0
exit /b %FAILED%

:compare
<nul set /p "=%1 against %2, tolerance %3: "
"%BIN%\gt-compare" "%OUT%\%1.png" "%OUT%\%2.png" %3 || set FAILED=1
exit /b
