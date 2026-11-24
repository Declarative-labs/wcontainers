@rem Compiles the format use test's programs (tests/answers.rs) with the Windows SDK's fxc, beside this file.
@echo off
setlocal
for /d %%d in ("C:\Program Files (x86)\Windows Kits\10\bin\10.*") do if exist "%%d\x64\fxc.exe" set FXC=%%d\x64\fxc.exe
cd /d %~dp0
for %%u in (load multisample_load buffer_load store buffer_store typed_load) do (
    call :cs %%u f float4 float || exit /b 1
    call :cs %%u u uint4 uint || exit /b 1
    call :cs %%u i int4 int || exit /b 1
)
for %%u in (sample compare gather gather_compare atomic_exchange) do (
    call :cs %%u f float4 float || exit /b 1
)
for %%u in (atomic_add atomic_bitwise atomic_compare atomic_exchange atomic_unsigned_min_or_max) do (
    call :cs %%u u uint4 uint || exit /b 1
)
for %%u in (atomic_add atomic_bitwise atomic_compare atomic_exchange atomic_signed_min_or_max) do (
    call :cs %%u i int4 int || exit /b 1
)
call :stages f float4 || exit /b 1
call :stages u uint4 || exit /b 1
call :stages i int4 || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E main /Fo bindings.dxbc bindings.hlsl || exit /b 1
exit /b 0

:cs
"%FXC%" /nologo /T cs_5_0 /E main /D use_%1 /D KIND=%3 /D SCALAR=%4 /Fo %1_%2.dxbc uses.hlsl
exit /b %errorlevel%

:stages
"%FXC%" /nologo /T vs_5_0 /E main /D KIND=%2 /Fo vertex_%1.dxbc vertex.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E main /D KIND=%2 /Fo pixel_%1.dxbc pixel.hlsl
exit /b %errorlevel%
