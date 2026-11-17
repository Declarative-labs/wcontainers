@rem Compiles own.hlsl's six entry points with the Windows SDK's fxc, beside this file.
@echo off
setlocal
for /d %%d in ("C:\Program Files (x86)\Windows Kits\10\bin\10.*") do if exist "%%d\x64\fxc.exe" set FXC=%%d\x64\fxc.exe
cd /d %~dp0
"%FXC%" /nologo /T vs_5_0 /E vs_transform /Fo vs_transform.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_textured /Fo ps_textured.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_integer /Fo ps_integer.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_loop /Fo ps_loop.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_fullscreen /Fo vs_fullscreen.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_window /Fo ps_window.dxbc own.hlsl || exit /b 1
echo %FXC%
