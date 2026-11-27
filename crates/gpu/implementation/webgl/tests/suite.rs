//! The shared back-end suite, and the back end's own checks, on ANGLE's OpenGL ES 3.0 (the desktop's
//! test route). `WEBGL_ANGLE_DIR`
//! names the directory holding the libEGL.dll and libGLESv2.dll angle\build.cmd builds (default
//! C:\wc\tools\angle-mozangle-0.7.1); `WEBGL_OFF` lists WebGL extensions to treat as missing,
//! separated by spaces, to take the routes of browsers without them.
#![cfg(windows)]

#[path = "../examples/common/angle.rs"]
mod host;

/// What the host shows of a presented frame, on ANGLE's window.
#[test]
#[ignore = "the OpenGL ES crate makes no EGL window surface yet, so the back end cannot present into a window"]
fn present_shows_the_frame() {
    checks::present_shows_the_frame();
}

#[path = "../examples/common/checks.rs"]
mod checks;

/// Runs a check of checks.rs with the patterns' seed from `WEBGL_SEED` (default 0) and as many
/// seeds from it as `WEBGL_CASES` says (default 1). A check with a route that ran nothing (its
/// feature absent, or its format without the use) passes, and prints "not run:" with its name and
/// why (libtest cannot skip at run time; all.cmd lists these lines).
fn sampled(check: fn()) {
    let var = |name, default| std::env::var(name).map_or(default, |v| v.parse().expect(name));
    checks::seeded(check, var("WEBGL_SEED", 0), var("WEBGL_CASES", 1));
    let why = checks::take_not_run();
    if !why.is_empty() {
        let name = std::thread::current().name().unwrap_or("?").to_string();
        println!("not run: {name}: {why}");
    }
}
#[path = "../examples/common/kit.rs"]
mod kit;

#[test]
fn vertex_and_instance_numbers_include_the_bases() {
    sampled(checks::vertex_and_instance_numbers_include_the_bases);
}

#[test]
fn discard_then_no_overwrite_writes() {
    sampled(checks::discard_then_no_overwrite_writes);
}

#[test]
fn depth_and_stencil_read_back() {
    sampled(checks::depth_and_stencil_read_back);
}

#[test]
fn stencil_sampled_and_read_back() {
    sampled(checks::stencil_sampled_and_read_back);
}

#[test]
fn copies_within_a_typeless_group() {
    sampled(checks::copies_within_a_typeless_group);
}

#[test]
fn cube_faces_copied_between_typeless_formats() {
    sampled(checks::cube_faces_copied_between_typeless_formats);
}

#[test]
fn copies_within_one_subresource() {
    sampled(checks::copies_within_one_subresource);
}

#[test]
fn depth_and_stencil_written_read_back_as_written() {
    sampled(checks::depth_and_stencil_written_read_back_as_written);
}

#[test]
fn copies_between_depth_and_colour_formats() {
    sampled(checks::copies_between_depth_and_colour_formats);
}

#[test]
fn copies_between_index_and_data_buffers() {
    sampled(checks::copies_between_index_and_data_buffers);
}

#[test]
fn loads_count_levels_from_the_view() {
    sampled(checks::loads_count_levels_from_the_view);
}

#[test]
fn fractional_viewports_are_refused() {
    sampled(checks::fractional_viewports_are_refused);
}

#[test]
fn viewports_past_their_bounds_are_refused() {
    sampled(checks::viewports_past_their_bounds_are_refused);
}

#[test]
fn immediates_reach_both_stages() {
    sampled(checks::immediates_reach_both_stages);
}

#[test]
fn packed_vertex_formats_are_converted() {
    sampled(checks::packed_vertex_formats_are_converted);
}

#[test]
fn timestamps_and_their_disjoint_flag() {
    sampled(checks::timestamps_and_their_disjoint_flag);
}

#[test]
fn multisampled_copies_resolve_as_their_source() {
    sampled(checks::multisampled_copies_resolve_as_their_source);
}

#[test]
fn fractional_depth_bias() {
    sampled(checks::fractional_depth_bias);
}

#[test]
fn signed_normalised_texels_drawn_then_copied() {
    sampled(checks::signed_normalised_texels_drawn_then_copied);
}

#[test]
fn signed_normalised_minimum_judged_by_whole_components() {
    sampled(checks::signed_normalised_minimum_judged_by_whole_components);
}

#[test]
fn half_floats_read_back_as_stored() {
    sampled(checks::half_floats_read_back_as_stored);
}

#[test]
fn two_devices_on_one_thread_keep_their_own_objects() {
    sampled(checks::two_devices_on_one_thread_keep_their_own_objects);
}

#[test]
fn programs_of_equal_shaders_draw_until_each_is_destroyed() {
    sampled(checks::programs_of_equal_shaders_draw_until_each_is_destroyed);
}

#[test]
fn bit_operations_equal_their_definition() {
    sampled(checks::bit_operations_equal_their_definition);
}

#[test]
fn gathers_equal_their_footprints() {
    sampled(checks::gathers_equal_their_footprints);
}

#[test]
fn depth_textures_compared_and_read_plainly() {
    sampled(checks::depth_textures_compared_and_read_plainly);
}

#[test]
fn linear_values_interpolate_in_window_space() {
    sampled(checks::linear_values_interpolate_in_window_space);
}

#[test]
fn programs_run_per_sample_and_write_the_sample_mask() {
    sampled(checks::programs_run_per_sample_and_write_the_sample_mask);
}

#[test]
fn special_float_values_keep_their_bits() {
    sampled(checks::special_float_values_keep_their_bits);
}

#[test]
fn lists_draw_the_largest_index_as_a_vertex() {
    sampled(checks::lists_draw_the_largest_index_as_a_vertex);
}

#[test]
fn constant_alpha_blends() {
    sampled(checks::constant_alpha_blends);
}

#[test]
fn vertices_captured_with_and_without_rasterising() {
    sampled(checks::vertices_captured_with_and_without_rasterising);
}

#[test]
fn block_compressed_and_shared_exponent_textures_read_back_as_written() {
    sampled(checks::block_compressed_and_shared_exponent_textures_read_back_as_written);
}

#[test]
fn interpolated_bc4_and_bc5_values() {
    sampled(checks::interpolated_bc4_and_bc5_values);
}

#[test]
fn fragment_depth_follows_the_clip_convention() {
    sampled(checks::fragment_depth_follows_the_clip_convention);
}

#[test]
fn captured_indices_drawn_both_ways() {
    sampled(checks::captured_indices_drawn_both_ways);
}

#[test]
fn strips_fans_loops_and_indexed_draws_captured_in_list_order() {
    sampled(checks::strips_fans_loops_and_indexed_draws_captured_in_list_order);
}

#[test]
fn absent_features_are_refused() {
    sampled(checks::absent_features_are_refused);
}

#[test]
fn copies_into_and_out_of_host_kept_textures() {
    sampled(checks::copies_into_and_out_of_host_kept_textures);
}

#[test]
fn srgb_views_of_block_compressed_textures() {
    sampled(checks::srgb_views_of_block_compressed_textures);
}

#[test]
fn bgra_textures_keep_their_byte_order() {
    sampled(checks::bgra_textures_keep_their_byte_order);
}

#[test]
fn bgrx_textures_hold_alpha_one() {
    sampled(checks::bgrx_textures_hold_alpha_one);
}

#[test]
fn srgb_copies_keep_every_byte() {
    sampled(checks::srgb_copies_keep_every_byte);
}

#[test]
fn sampled_views_are_whole_textures_of_one_mip_range() {
    sampled(checks::sampled_views_are_whole_textures_of_one_mip_range);
}

#[test]
fn generate_mipmap_equals_the_draws() {
    sampled(checks::generate_mipmap_equals_the_draws);
}

#[test]
fn sixteen_bit_normalised_formats_follow_their_extension() {
    sampled(checks::sixteen_bit_normalised_formats_follow_their_extension);
}

#[test]
fn bgra_surfaces_present() {
    sampled(checks::bgra_surfaces_present);
}

#[test]
fn signed_normalised_formats_follow_their_extension() {
    sampled(checks::signed_normalised_formats_follow_their_extension);
}

#[test]
fn levels_stay_as_written_while_others_are_read_and_drawn() {
    sampled(checks::levels_stay_as_written_while_others_are_read_and_drawn);
}

#[test]
fn passes_without_attachments_rasterise() {
    sampled(checks::passes_without_attachments_rasterise);
}

#[test]
fn independent_blends_per_target() {
    sampled(checks::independent_blends_per_target);
}

#[test]
fn second_source_blends() {
    sampled(checks::second_source_blends);
}

#[test]
fn depth_clamped_to_the_viewport_range() {
    sampled(checks::depth_clamped_to_the_viewport_range);
}

#[test]
fn depth_bias_clamped() {
    sampled(checks::depth_bias_clamped);
}

#[test]
fn polygons_filled_with_lines() {
    sampled(checks::polygons_filled_with_lines);
}

#[test]
fn mirror_once_addressing() {
    sampled(checks::mirror_once_addressing);
}

#[test]
fn clip_and_cull_distances() {
    sampled(checks::clip_and_cull_distances);
}

#[test]
fn points_sized_by_the_program() {
    sampled(checks::points_sized_by_the_program);
}

#[test]
fn broken_rules_lose_the_device() {
    sampled(checks::broken_rules_lose_the_device);
}

#[test]
fn stages_past_the_uniform_buffer_limit_are_refused() {
    sampled(checks::stages_past_the_uniform_buffer_limit_are_refused);
}

#[test]
fn copies_within_one_float_format_take_the_probed_route() {
    sampled(checks::copies_within_one_float_format_take_the_probed_route);
}
