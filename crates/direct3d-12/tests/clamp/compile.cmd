@rem Compiles the depth clamp scene's shaders (tests/gpu.rs) with the Windows SDK's fxc, beside this file.
@echo off
setlocal
for /d %%d in ("C:\Program Files (x86)\Windows Kits\10\bin\10.*") do if exist "%%d\x64\fxc.exe" set FXC=%%d\x64\fxc.exe
cd /d %~dp0
"%FXC%" /nologo /T vs_5_0 /E main /Fo vs.dxbc vs.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E main /Fo ps.dxbc ps.hlsl || exit /b 1
"%FXC%" /nologo /T gs_5_0 /E main /Fo gs.dxbc gs.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E main /Fo culled_vs.dxbc culled_vs.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E main /Fo clipped_vs.dxbc clipped_vs.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E main /Fo clipped_culled_vs.dxbc clipped_culled_vs.hlsl || exit /b 1
"%FXC%" /nologo /T hs_5_0 /E main /Fo hs.dxbc hs.hlsl || exit /b 1
"%FXC%" /nologo /T ds_5_0 /E main /Fo ds.dxbc ds.hlsl || exit /b 1
