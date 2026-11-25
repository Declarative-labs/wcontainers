//! The shared suite's tests other than `present_shows_the_frame`, for the hosts that run present
//! apart: the browser page (examples/scenes/suite.rs) and macOS, whose windows need the main thread
//! (tests/present.rs). web/check runs the page against the whole native list on Windows, so a test
//! the shared crate adds and this list lacks fails there.

/// Calls the macro `$m` with `$pre` followed by every name.
macro_rules! suite_but_present {
    ($($m:ident)::+ { $($pre:tt)* }) => {
        $($m)::+!($($pre)*
            buffer_writes_land_in_order_and_read_back,
            texture_writes_land_and_read_back,
            copies_move_bytes_exactly,
            clears_fill_exactly,
            triangle_covers_the_pixels_whose_centres_it_holds,
            textured_draw_with_depth_and_blending,
            edges_through_pixel_centres_follow_the_top_left_rule,
            strips_take_flat_values_from_the_provoking_vertex,
            depth_writes_off_leave_the_depth,
            a_fragment_covering_no_sample_writes_nothing,
            srgb_targets_convert_colour_but_not_alpha,
            generate_mips_averages_in_the_values_format,
            values_are_interpolated_with_perspective,
            the_scissor_bounds_the_draw,
            a_scissor_past_the_target_is_clipped_to_it,
            clear_rects_clear_each_rectangle_clipped_to_the_view,
            copies_between_depth_stencil_textures_move_both_aspects,
            stencil_operations_and_tests,
            near_clipping_and_the_viewport_depth_range,
            linear_filtering_and_address_modes,
            instances_start_at_the_first_instance,
            a_stream_of_stride_zero_reads_its_first_element,
            comparison_samplers_compare_with_the_stored_depth,
            indexed_strips_restart_and_add_the_base_vertex,
            fragments_write_storage_buffers,
            multisampled_targets_cover_their_samples_and_resolve,
            resolves_average_in_the_format_named,
            programs_run_per_sample_and_read_samples,
            occlusion_queries_answer_whether_samples_passed,
            the_occlusion_counter_counts_every_sample_that_passes,
            fill_modes_draw_edges_or_corners,
            values_pass_between_programs_by_location,
            vertices_pulled_from_storage_match_the_fetched_ones,
            constants_read_through_a_texture_lie_256_rows_to_a_texture_row,
            missing_components_read_their_defaults,
            typed_buffers_through_storage_read_their_views_place_and_length,
            typed_buffers_through_a_texture_lie_2048_elements_to_a_row,
            captures_write_location_outputs_packed_in_draw_order,
            a_copy_within_one_subresource_moves_the_region,
            a_copy_within_one_depth_or_stencil_subresource_moves_the_region,
            every_vertex_format_reads_its_element,
            an_indexed_draw_past_a_streams_end_covers_only_what_its_stream_allows,
            a_copy_within_one_3d_or_compressed_subresource_moves_the_region,
            indirect_draws_and_dispatches_read_their_arguments_when_they_run,
        );
    };
}
