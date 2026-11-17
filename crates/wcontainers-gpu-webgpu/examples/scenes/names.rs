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
            stencil_operations_and_tests,
            near_clipping_and_the_viewport_depth_range,
            linear_filtering_and_address_modes,
            instances_start_at_the_first_instance,
            indexed_strips_restart_and_add_the_base_vertex,
            fragments_write_storage_buffers,
            multisampled_targets_cover_their_samples_and_resolve,
            programs_run_per_sample_and_read_samples,
            occlusion_queries_answer_whether_samples_passed,
        );
    };
}
