@rem Compiles the entry points of own.hlsl, more.hlsl, spaces.hlsl, twospaces.hlsl, signed.hlsl, route.hlsl, values.hlsl, evaluate.hlsl, negate.hlsl, variants.hlsl, pull.hlsl, filter.hlsl, geometry.hlsl, geometry_rows.hlsl, compute.hlsl, readonly.hlsl, targets.hlsl, texels.hlsl, tessellation.hlsl, tessellation_values.hlsl, domain_rows.hlsl, layers.hlsl, uav.hlsl, arrays.hlsl, msarray.hlsl, early.hlsl, shadow.hlsl, msloop.hlsl (shader model 4.0), divide.hlsl, narrow.hlsl, components.hlsl, one_d.hlsl, lines.hlsl, vertex_storage.hlsl, geometry_typed.hlsl, geometry_patch.hlsl, hull_none.hlsl (fxc only) and waves.hlsl (dxc only) with
@rem the Windows SDK's fxc (ps_spaces, vs_transform, ps_textured, signed.hlsl's, route.hlsl's, values.hlsl's, negate.hlsl's, variants.hlsl's, pull.hlsl's, filter.hlsl's, the geometry, the compute and the typed buffer shaders also
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
"%FXC%" /nologo /T ps_5_0 /E ps_gatheroff /Fo ps_gatheroff.dxbc own.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_gatheroff -Gis -Fo ps_gatheroff.dxil own.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_primid /Fo ps_primid.dxbc own.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_primid -Gis -Fo ps_primid.dxil own.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_shade /Fo vs_shade.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_flat /Fo ps_flat.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shade /Fo ps_shade.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_depth /Fo ps_depth.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_local /Fo ps_local.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_size /Fo ps_size.dxbc more.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_1 /E ps_spaces /Fo ps_spaces.dxbc spaces.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_spaces -Fo ps_spaces.dxil spaces.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_twospaces /Fo ps_twospaces.dxbc twospaces.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_twospaces -D SPACES -Fo ps_twospaces.dxil twospaces.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_transform -Gis -Fo vs_transform.dxil own.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_textured -Gis -Fo ps_textured.dxil own.hlsl || exit /b 1
for %%e in (ps_signed ps_offset ps_special ps_half ps_cases ps_pack) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc signed.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil signed.hlsl || exit /b 1
)
for %%e in (ps_rows ps_clip) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc route.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil route.hlsl || exit /b 1
)
"%FXC%" /nologo /T ps_5_0 /E ps_component /Fo ps_component.dxbc components.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_component -Gis -Fo ps_component.dxil components.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_narrow /Fo ps_narrow.dxbc narrow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_narrow -Gis -Fo ps_narrow.dxil narrow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_2 -enable-16bit-types -E ps_narrow16 -Gis -Fo ps_narrow16.dxil narrow.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_pack /Fo vs_pack.dxbc signed.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_pack -Gis -Fo vs_pack.dxil signed.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_rows_io /Fo vs_rows_io.dxbc signed.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_rows_io -Gis -Fo vs_rows_io.dxil signed.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_rows_io /Fo ps_rows_io.dxbc signed.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_rows_io -Gis -Fo ps_rows_io.dxil signed.hlsl || exit /b 1
for %%e in (ps_ftoi ps_table ps_deriv ps_coarse) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc values.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil values.hlsl || exit /b 1
)
"%FXC%" /nologo /T ps_5_0 /E ps_helper /Fo ps_helper.dxbc values.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_6 -E ps_helper -Gis -Fo ps_helper.dxil values.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_carry /Fo vs_carry.dxbc values.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_carry -Gis -Fo vs_carry.dxil values.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_negate /Fo ps_negate.dxbc negate.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_negate -Gis -Fo ps_negate.dxil negate.hlsl || exit /b 1
for %%e in (ps_dual ps_levels ps_levels_past ps_samples ps_ids ps_minlod ps_mingather ps_mirror) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc variants.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil variants.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_ids /Fo vs_ids.dxbc variants.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_ids -Gis -Fo vs_ids.dxil variants.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_clip /Fo vs_clip.dxbc variants.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_clip -Gis -Fo vs_clip.dxil variants.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_clip_six /Fo vs_clip_six.dxbc variants.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_clip_six -Gis -Fo vs_clip_six.dxil variants.hlsl || exit /b 1
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
"%FXC%" /nologo /T ps_5_0 /E ps_clamp /Fo ps_clamp.dxbc filter.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_clamp -Gis -Fo ps_clamp.dxil filter.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_lod /Fo ps_lod.dxbc filter.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_lod -Gis -Fo ps_lod.dxil filter.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_geometry /Fo vs_geometry.dxbc geometry.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_geometry -Fo vs_geometry.dxil geometry.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_geometry /Fo ps_geometry.dxbc geometry.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_geometry -Fo ps_geometry.dxil geometry.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_cull /Fo vs_cull.dxbc geometry.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_cull -Fo vs_cull.dxil geometry.hlsl || exit /b 1
for %%e in (gs_quads gs_cut gs_instances gs_lines gs_points gs_unplaced gs_adjacent gs_line_adjacent gs_clip gs_cull gs_cull_lines gs_cull_points gs_streams) do (
  "%FXC%" /nologo /T gs_5_0 /E %%e /Fo %%e.dxbc geometry.hlsl || exit /b 1
  "%DXC%" -nologo -T gs_6_0 -E %%e -Fo %%e.dxil geometry.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_rows /Fo vs_rows.dxbc geometry_rows.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_rows -Fo vs_rows.dxil geometry_rows.hlsl || exit /b 1
for %%e in (gs_rows_lines gs_rows_streams gs_rows_out) do (
  "%FXC%" /nologo /T gs_5_0 /E %%e /Fo %%e.dxbc geometry_rows.hlsl || exit /b 1
  "%DXC%" -nologo -T gs_6_0 -E %%e -Fo %%e.dxil geometry_rows.hlsl || exit /b 1
)
"%FXC%" /nologo /T ds_5_0 /E ds_rows_gap /Fo ds_rows_gap.dxbc domain_rows.hlsl || exit /b 1
"%DXC%" -nologo -T ds_6_0 -E ds_rows_gap -Fo ds_rows_gap.dxil domain_rows.hlsl || exit /b 1
for %%e in (cs_atomics cs_decrement cs_images cs_shared) do (
  "%FXC%" /nologo /T cs_5_0 /E %%e /Fo %%e.dxbc compute.hlsl || exit /b 1
  "%DXC%" -nologo -T cs_6_0 -E %%e -Gis -Fo %%e.dxil compute.hlsl || exit /b 1
)
"%FXC%" /nologo /T ps_5_0 /E ps_two /Fo ps_two.dxbc targets.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_two -Gis -Fo ps_two.dxil targets.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_readonly /Fo cs_readonly.dxbc readonly.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_divide /Fo cs_divide.dxbc divide.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_divide -Fo cs_divide.dxil divide.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_escape /Fo cs_escape.dxbc divide.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_escape -Fo cs_escape.dxil divide.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_readonly -Gis -Fo cs_readonly.dxil readonly.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_texels /Fo cs_texels.dxbc texels.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_texels -Gis -Fo cs_texels.dxil texels.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_texels /Fo ps_texels.dxbc texels.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_texels -Gis -Fo ps_texels.dxil texels.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_through /Fo cs_through.dxbc texels.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_through -Gis -Fo cs_through.dxil texels.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_texels /Fo vs_texels.dxbc texels.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_texels -Gis -Fo vs_texels.dxil texels.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_early /Fo vs_early.dxbc early.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_early -Gis -Fo vs_early.dxil early.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_early /Fo ps_early.dxbc early.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_early -Gis -Fo ps_early.dxil early.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_mark /Fo ps_mark.dxbc early.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_mark -Gis -Fo ps_mark.dxil early.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_contend /Fo ps_contend.dxbc early.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_contend -Gis -Fo ps_contend.dxil early.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_patch /Fo vs_patch.dxbc tessellation.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_patch -Gis -Fo vs_patch.dxil tessellation.hlsl || exit /b 1
for %%d in (tri quad) do for %%m in (integer pow2 odd even) do for %%o in (point cw ccw) do (
  "%FXC%" /nologo /T hs_5_0 /E hs_%%d_%%m_%%o /Fo hs_%%d_%%m_%%o.dxbc tessellation.hlsl || exit /b 1
  "%DXC%" -nologo -T hs_6_0 -E hs_%%d_%%m_%%o -Gis -Fo hs_%%d_%%m_%%o.dxil tessellation.hlsl || exit /b 1
)
for %%m in (integer pow2 odd even) do for %%o in (point line) do (
  "%FXC%" /nologo /T hs_5_0 /E hs_line_%%m_%%o /Fo hs_line_%%m_%%o.dxbc tessellation.hlsl || exit /b 1
  "%DXC%" -nologo -T hs_6_0 -E hs_line_%%m_%%o -Gis -Fo hs_line_%%m_%%o.dxil tessellation.hlsl || exit /b 1
)
"%FXC%" /nologo /T hs_5_0 /E hs_tri_none /Fo hs_tri_none.dxbc hull_none.hlsl || exit /b 1
for %%e in (ds_tri ds_quad ds_line ds_tri_none) do (
  "%FXC%" /nologo /T ds_5_0 /E %%e /Fo %%e.dxbc tessellation.hlsl || exit /b 1
  "%DXC%" -nologo -T ds_6_0 -E %%e -Gis -Fo %%e.dxil tessellation.hlsl || exit /b 1
)
for %%e in (gs_tess_point gs_tess_line gs_tess_tri) do (
  "%FXC%" /nologo /T gs_5_0 /E %%e /Fo %%e.dxbc tessellation.hlsl || exit /b 1
  "%DXC%" -nologo -T gs_6_0 -E %%e -Gis -Fo %%e.dxil tessellation.hlsl || exit /b 1
)
for %%e in (hs_values_odd_ccw hs_values_even_cw hs_values_integer_point) do (
  "%FXC%" /nologo /T hs_5_0 /E %%e /Fo %%e.dxbc tessellation_values.hlsl || exit /b 1
  "%DXC%" -nologo -T hs_6_0 -E %%e -Gis -Fo %%e.dxil tessellation_values.hlsl || exit /b 1
)
"%FXC%" /nologo /T ds_5_0 /E ds_values /Fo ds_values.dxbc tessellation_values.hlsl || exit /b 1
"%DXC%" -nologo -T ds_6_0 -E ds_values -Gis -Fo ds_values.dxil tessellation_values.hlsl || exit /b 1
"%FXC%" /nologo /T gs_5_0 /E gs_values_tess /Fo gs_values_tess.dxbc tessellation_values.hlsl || exit /b 1
"%DXC%" -nologo -T gs_6_0 -E gs_values_tess -Gis -Fo gs_values_tess.dxil tessellation_values.hlsl || exit /b 1
"%FXC%" /nologo /T gs_5_0 /E gs_layers /Fo gs_layers.dxbc layers.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_layer /Fo ps_layer.dxbc layers.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_layer -Gis -Fo ps_layer.dxil layers.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_halves -Gis -Fo ps_halves.dxil uav.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_array -Gis -Fo ps_array.dxil arrays.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_image_array -Gis -Fo cs_image_array.dxil arrays.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_1 /E ps_array /Fo ps_array.dxbc arrays.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_1 /E cs_image_array /Fo cs_image_array.dxbc arrays.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_1 /E cs_buffer_array /enable_unbounded_descriptor_tables /Fo cs_buffer_array.dxbc arrays.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_buffer_array -Gis -Fo cs_buffer_array.dxil arrays.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_1 /E cs_typed_array /enable_unbounded_descriptor_tables /Fo cs_typed_array.dxbc arrays.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_typed_array -Gis -Fo cs_typed_array.dxil arrays.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_1 /E cs_structured_array /enable_unbounded_descriptor_tables /Fo cs_structured_array.dxbc arrays.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_structured_array -Gis -Fo cs_structured_array.dxil arrays.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_one_d /Fo cs_one_d.dxbc one_d.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_one_d -Gis -Fo cs_one_d.dxil one_d.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_view_kinds /Fo cs_view_kinds.dxbc views.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_view_kinds -Gis -Fo cs_view_kinds.dxil views.hlsl || exit /b 1
"%FXC%" /nologo /T cs_5_0 /E cs_view_sampled /Fo cs_view_sampled.dxbc views.hlsl || exit /b 1
"%DXC%" -nologo -T cs_6_0 -E cs_view_sampled -Gis -Fo cs_view_sampled.dxil views.hlsl || exit /b 1
for %%e in (ps_fill ps_msarray) do (
  "%FXC%" /nologo /T ps_5_0 /E %%e /Fo %%e.dxbc msarray.hlsl || exit /b 1
  "%DXC%" -nologo -T ps_6_0 -E %%e -Gis -Fo %%e.dxil msarray.hlsl || exit /b 1
)
"%DXC%" -nologo -T ps_6_0 -E ps_wave -Gis -Fo ps_wave.dxil waves.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_wave_signed -Gis -Fo ps_wave_signed.dxil waves.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow /Fo ps_shadow.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow -Gis -Fo ps_shadow.dxil shadow.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow_wide /Fo ps_shadow_wide.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow_wide -Gis -Fo ps_shadow_wide.dxil shadow.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow_gathers /Fo ps_shadow_gathers.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow_gathers -Gis -Fo ps_shadow_gathers.dxil shadow.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow_compared_red /Fo ps_shadow_compared_red.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow_compared_red -Gis -Fo ps_shadow_compared_red.dxil shadow.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow_compared_green /Fo ps_shadow_compared_green.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow_compared_green -Gis -Fo ps_shadow_compared_green.dxil shadow.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow_compared_blue /Fo ps_shadow_compared_blue.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow_compared_blue -Gis -Fo ps_shadow_compared_blue.dxil shadow.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_shadow_compared_alpha /Fo ps_shadow_compared_alpha.dxbc shadow.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_shadow_compared_alpha -Gis -Fo ps_shadow_compared_alpha.dxil shadow.hlsl || exit /b 1
for %%e in (ps_msloop ps_msswap) do (
  "%FXC%" /nologo /T ps_4_0 /E %%e /Fo %%e.dxbc msloop.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_evaluate /Fo vs_evaluate.dxbc evaluate.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_evaluate -Gis -Fo vs_evaluate.dxil evaluate.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_evaluate /Fo ps_evaluate.dxbc evaluate.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_evaluate -Gis -Fo ps_evaluate.dxil evaluate.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_eval_exact /Fo vs_eval_exact.dxbc evaluate.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_eval_exact /Fo ps_eval_exact.dxbc evaluate.hlsl || exit /b 1
"%FXC%" /nologo /T vs_5_0 /E vs_line /Fo vs_line.dxbc lines.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_line -Gis -Fo vs_line.dxil lines.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_line /Fo ps_line.dxbc lines.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_line -Gis -Fo ps_line.dxil lines.hlsl || exit /b 1
"%FXC%" /nologo /T ps_5_0 /E ps_primitive /Fo ps_primitive.dxbc lines.hlsl || exit /b 1
"%DXC%" -nologo -T ps_6_0 -E ps_primitive -Gis -Fo ps_primitive.dxil lines.hlsl || exit /b 1
for %%e in (vs_store_texture vs_store_typed vs_store_raw) do (
  "%FXC%" /nologo /T vs_5_0 /E %%e /Fo %%e.dxbc vertex_storage.hlsl || exit /b 1
  "%DXC%" -nologo -T vs_6_0 -E %%e -Fo %%e.dxil vertex_storage.hlsl || exit /b 1
)
"%FXC%" /nologo /T vs_5_0 /E vs_store_drawn /Fo vs_store_drawn.dxbc vertex_storage.hlsl || exit /b 1
"%DXC%" -nologo -T vs_6_0 -E vs_store_drawn -Gis -Fo vs_store_drawn.dxil vertex_storage.hlsl || exit /b 1
"%FXC%" /nologo /T gs_5_0 /E gs_typed /Fo gs_typed.dxbc geometry_typed.hlsl || exit /b 1
"%DXC%" -nologo -T gs_6_0 -E gs_typed -Gis -Fo gs_typed.dxil geometry_typed.hlsl || exit /b 1
"%FXC%" /nologo /T gs_5_0 /E gs_patch /Fo gs_patch.dxbc geometry_patch.hlsl || exit /b 1
"%DXC%" -nologo -T gs_6_0 -E gs_patch -Gis -Fo gs_patch.dxil geometry_patch.hlsl || exit /b 1
echo %FXC% %DXC%
