@echo off
rem Builds ANGLE's libEGL.dll and libGLESv2.dll with the mozangle crate and copies them to
rem C:\wc\tools\angle-mozangle-0.7.1, where the WebGL 2 back end's native tests load them. Run from the
rem checkout root on the desktop. mozangle's ANGLE lacks three of upstream ANGLE's fixes to its Direct3D
rem 11 renderer under EXT_clip_control (named in d3d11-clip-control.patch) and its fix for immutable
rem textures whose base level changes (immutable-base-level.patch), which the patches apply to a fresh
rem unpacked copy of the crate in C:\wc\tools\mozangle-0.7.1-patched.
rem _CL_ holds the compiler to three processes (mozangle asks cl for /MP); mozangle's bindgen needs
rem libclang, taken from the libclang 18.1.1 wheel unpacked in C:\wc\tools, told that the Visual Studio
rem library's wish for clang 19 does not matter to bindings.
setlocal
set _CL_=/MP3
set LIBCLANG_PATH=C:\wc\tools\libclang-18.1.1\libclang-18.1.1.data\platlib\clang\native
set BINDGEN_EXTRA_CLANG_ARGS=-D_ALLOW_COMPILER_AND_STL_VERSION_MISMATCH
set DEST=C:\wc\tools\angle-mozangle-0.7.1
set SRC=C:\wc\tools\mozangle-0.7.1-patched
set PATCHES=%CD%\crates\gpu-webgl\angle
set UNPACK=C:\wc\tools\mozangle-0.7.1-unpack
if exist %UNPACK% rmdir /s /q %UNPACK%
mkdir %UNPACK%
curl.exe -sSfL -o %UNPACK%\mozangle-0.7.1.crate https://static.crates.io/crates/mozangle/mozangle-0.7.1.crate || exit /b 1
tar.exe -xzf %UNPACK%\mozangle-0.7.1.crate -C %UNPACK% || exit /b 1
git -C %UNPACK%\mozangle-0.7.1 apply --ignore-whitespace "%PATCHES%\d3d11-clip-control.patch" || exit /b 1
git -C %UNPACK%\mozangle-0.7.1 apply --ignore-whitespace "%PATCHES%\immutable-base-level.patch" || exit /b 1
if exist %SRC% rmdir /s /q %SRC%
move %UNPACK%\mozangle-0.7.1 %SRC% >nul || exit /b 1
rmdir /s /q %UNPACK%
rem A build from nothing: the unpacked sources keep the archive's old times, which cargo would trust.
set TARGET=%CARGO_TARGET_DIR%\angle-patched
if exist %TARGET% rmdir /s /q %TARGET%
cargo build -j 3 --release --manifest-path crates\gpu-webgl\angle\Cargo.toml --target-dir %TARGET% --config "patch.crates-io.mozangle.path='C:/wc/tools/mozangle-0.7.1-patched'" || exit /b 1
if not exist %DEST% mkdir %DEST%
for /d %%d in ("%TARGET%\release\build\mozangle-*") do if exist "%%d\out\libEGL.dll" copy /y "%%d\out\libEGL.dll" %DEST% >nul && copy /y "%%d\out\libGLESv2.dll" %DEST% >nul
if not exist %DEST%\libGLESv2.dll exit /b 1
dir %DEST%
