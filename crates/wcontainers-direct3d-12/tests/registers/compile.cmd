@rem Compiles the stage register test's programs (tests/registers.rs) with the Windows SDK's fxc, beside this file.
@echo off
setlocal
for /d %%d in ("C:\Program Files (x86)\Windows Kits\10\bin\10.*") do if exist "%%d\x64\fxc.exe" set FXC=%%d\x64\fxc.exe
cd /d %~dp0
"%FXC%" /nologo /T vs_5_0 /E main /Fo vs.dxbc vs.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E main /Fo ps.dxbc ps.hlsl
exit /b %errorlevel%
