@rem Compiles the entry points of own.hlsl, more.hlsl, spaces.hlsl, signed.hlsl, route.hlsl, values.hlsl, negate.hlsl, variants.hlsl, pull.hlsl, filter.hlsl, geometry.hlsl and compute.hlsl with
@rem the Windows SDK's fxc (ps_spaces, vs_transform, ps_textured, signed.hlsl's, route.hlsl's, values.hlsl's, negate.hlsl's, variants.hlsl's, pull.hlsl's, filter.hlsl's, the geometry and the compute shaders also
@rem with its dxc, IEEE-strict so NaN tests stay), beside this file.
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
for %%e in (ps_ftoi ps_table ps_deriv ps_coarse) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc values.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil values.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_carry /Fo vs_carry.dxbc values.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_carry -Gis -Fo vs_carry.dxil values.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_negate /Fo ps_negate.dxbc negate.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_negate -Gis -Fo ps_negate.dxil negate.hlsl || exit /b 1
for %%e in (ps_dual ps_levels ps_samples ps_ids ps_minlod ps_mingather ps_mirror) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc variants.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil variants.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_ids /Fo vs_ids.dxbc variants.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_ids -Gis -Fo vs_ids.dxil variants.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_clip /Fo vs_clip.dxbc variants.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_clip -Gis -Fo vs_clip.dxil variants.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_deep /Fo vs_deep.dxbc variants.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_deep -Gis -Fo vs_deep.dxil variants.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_pull /Fo vs_pull.dxbc pull.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_pull -Gis -Fo vs_pull.dxil pull.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_pull /Fo ps_pull.dxbc pull.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_pull -Gis -Fo ps_pull.dxil pull.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_filter /Fo vs_filter.dxbc filter.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_filter -Gis -Fo vs_filter.dxil filter.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_filter /Fo ps_filter.dxbc filter.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_filter -Gis -Fo ps_filter.dxil filter.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_geometry /Fo vs_geometry.dxbc geometry.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_geometry /Fo ps_geometry.dxbc geometry.hlsl || exit /b 1
for %%e in (gs_quads gs_cut gs_instances gs_lines gs_points gs_adjacent gs_line_adjacent) do (
  "%FXC%" /nologo /T gs_5_0 /E %%e /Fo %%e.dxbc geometry.hlsl || exit /b 1
  "%DXC%" -nologo -T gs_6_0 -E %%e -Gis -Fo %%e.dxil geometry.hlsl || exit /b 1
)
for %%e in (cs_atomics cs_decrement cs_images) do (
  "%FXC%" /nologo /T cs_5_0 /E %%e /Fo %%e.dxbc compute.hlsl || exit /b 1
  "%DXC%" -nologo -T cs_6_0 -E %%e -Gis -Fo %%e.dxil compute.hlsl || exit /b 1
)
echo %FXC% %DXC%
