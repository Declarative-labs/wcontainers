@rem Compiles the entry points of own.hlsl, more.hlsl, spaces.hlsl, signed.hlsl and route.hlsl with the Windows
@rem SDK's fxc (ps_spaces, vs_transform, ps_textured, signed.hlsl's and route.hlsl's also with its dxc, IEEE-strict
@rem so NaN tests stay), beside this file.
@echo off
setlocal
for /d %%d in ("C:\Program Files (x86)\Windows Kits\10\bin\10.*") do if exist "%%d\x64\fxc.exe" set FXC=%%d\x64\fxc.exe
for /d %%d in ("C:\Program Files (x86)\Windows Kits\10\bin\10.*") do if exist "%%d\x64\dxc.exe" set DXC=%%d\x64\dxc.exe
cd /d %~dp0
"%FXC%" /nologo /T vs_5_0 /E vs_transform /Fo vs_transform.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_textured /Fo ps_textured.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_integer /Fo ps_integer.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_loop /Fo ps_loop.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_fullscreen /Fo vs_fullscreen.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_window /Fo ps_window.dxbc own.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_shade /Fo vs_shade.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_flat /Fo ps_flat.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shade /Fo ps_shade.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_depth /Fo ps_depth.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_local /Fo ps_local.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_size /Fo ps_size.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_1 /E ps_spaces /Fo ps_spaces.dxbc spaces.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_spaces -Fo ps_spaces.dxil spaces.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_transform -Gis -Fo vs_transform.dxil own.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_textured -Gis -Fo ps_textured.dxil own.hlsl || exit /b 1
for %%e in (ps_signed ps_offset ps_special ps_cases ps_pack) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc signed.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil signed.hlsl || exit /b 1
)
for %%e in (ps_rows ps_clip) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc route.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil route.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_pack /Fo vs_pack.dxbc signed.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_pack -Gis -Fo vs_pack.dxil signed.hlsl || exit /b 1
echo %FXC% %DXC%
