@echo off
rem Draws the scenes on ANGLE (the DLLs angle\build.cmd makes) and in Chrome's WebGL 2, and checks both against
rem the stored Direct3D pictures: run.cmd <out-dir> [<extensions to switch off>]. A GPU job, after
rem build.cmd; run from the checkout root.
setlocal
set OUT=%~1
set OFF=
if not "%~2"=="" set OFF=--off "%~2"
set ANGLE=C:\wc\tools\angle-mozangle-0.7.1
set FAIL=
"%CARGO_TARGET_DIR%\debug\examples\webgl-scenes.exe" angle "%ANGLE%" "%OUT%\angle" %OFF% || set FAIL=1
node crates\wcontainers-gpu-webgl\browser\run.mjs "%CARGO_TARGET_DIR%\webgl-pkg" "%OUT%\chrome" "%~2" || set FAIL=1
"%CARGO_TARGET_DIR%\debug\examples\webgl-scenes.exe" check "%OUT%\chrome" || set FAIL=1
if defined FAIL exit /b 1
