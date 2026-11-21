@echo off
rem The whole loop on the desktop, run as a GPU job by `loop`: every Direct3D 11 and 12 scene on the
rem GPU checked against its stored reference (gt-run check), three scenes again through the
rem forwarding d3d11.dll, which must have loaded from the programs' directory and logged the device
rem creation going through it, and the triangle through wgpu (Direct3D 12 and Vulkan) and Chrome
rem (WebGPU and WebGL 2), each of which must equal Direct3D 11's triangle. Exits 1 if any check fails.
rem   loop.cmd <name>   writes C:\wc\out\<name>
setlocal
set BIN=%CARGO_TARGET_DIR%\debug
set OUT=C:\wc\out\%~1
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%" || exit /b 2
set FAILED=0
"%BIN%\gt-run" check "%OUT%\scenes" || set FAILED=1
"%BIN%\gt-run" check "%OUT%\forward" --api d3d11 --dll "d3d11.dll=%BIN%\wcontainers_graphics_test_forward.dll" triangle quads topology || set FAILED=1
findstr /b /c:"d3d11 forward: D3D11CreateDevice" "%OUT%\forward\d3d11\triangle.log" >nul || (echo the forwarding run did not create its device through the replacement d3d11.dll& set FAILED=1)
"%BIN%\gt-wgpu" dx12 "%OUT%" || exit /b 2
"%BIN%\gt-wgpu" vulkan "%OUT%" || exit /b 2
"%BIN%\gt-scene" triangle > "%OUT%\triangle.json" || exit /b 2
node tools\graphics-rig\browser\draw.mjs "%OUT%\triangle.json" "%OUT%" || exit /b 2
for %%p in (wgpu-dx12 wgpu-vulkan chrome-webgpu chrome-webgl2) do call :compare %%p-triangle.png scenes\d3d11\triangle.png
exit /b %FAILED%

:compare
<nul set /p "=%1 against %2: "
"%BIN%\gt-compare" "%OUT%\%1" "%OUT%\%2" 0 || set FAILED=1
exit /b
