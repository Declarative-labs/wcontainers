@echo off
rem Builds ANGLE's libEGL.dll and libGLESv2.dll with the mozangle crate and copies them to
rem C:\wc\tools\angle-mozangle-0.7.1, where the WebGL 2 back end's native tests load them. Run from the
rem checkout root on the desktop. _CL_ holds the compiler to three processes (mozangle asks cl for /MP);
rem mozangle's bindgen needs libclang, taken from the libclang 18.1.1 wheel unpacked in C:\wc\tools, told
rem that the Visual Studio library's wish for clang 19 does not matter to bindings.
setlocal
set _CL_=/MP3
set LIBCLANG_PATH=C:\wc\tools\libclang-18.1.1\libclang-18.1.1.data\platlib\clang\native
set BINDGEN_EXTRA_CLANG_ARGS=-D_ALLOW_COMPILER_AND_STL_VERSION_MISMATCH
set DEST=C:\wc\tools\angle-mozangle-0.7.1
cargo build -j 3 --release --manifest-path crates\wcontainers-gpu-webgl\angle\Cargo.toml || exit /b 1
if not exist %DEST% mkdir %DEST%
for /d %%d in ("%CARGO_TARGET_DIR%\release\build\mozangle-*") do if exist "%%d\out\libEGL.dll" copy /y "%%d\out\libEGL.dll" %DEST% >nul && copy /y "%%d\out\libGLESv2.dll" %DEST% >nul
if not exist %DEST%\libGLESv2.dll exit /b 1
dir %DEST%
