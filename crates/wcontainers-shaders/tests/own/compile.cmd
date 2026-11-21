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
for %%e in (ds_tri ds_quad ds_line) do (
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
echo %FXC% %DXC%
