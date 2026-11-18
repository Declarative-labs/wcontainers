//! The shared back-end suite, and the back end's own checks, on ANGLE's OpenGL ES 3.0 (the desktop's
//! test route). `WEBGL_ANGLE_DIR`
//! names the directory holding the libEGL.dll and libGLESv2.dll angle\build.cmd builds (default
//! C:\wc\tools\angle-mozangle-0.7.1); `WEBGL_OFF` lists WebGL extensions to treat as missing,
//! separated by spaces, to take the routes of browsers without them.
#![cfg(windows)]

use std::path::PathBuf;
use wcontainers_gpu_webgl::WebGl;

struct Angle;

impl wcontainers_gpu_test::Host for Angle {
    type Backend = WebGl;

    fn make() -> WebGl {
        let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
            || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
            PathBuf::from,
        );
        let off = std::env::var("WEBGL_OFF").unwrap_or_default();
        let off: Vec<&str> = off.split_whitespace().collect();
        let mut b = WebGl::on_angle(&dir, &off).expect("ANGLE's OpenGL ES 3.0 context");
        use wcontainers_gpu::Backend;
        assert_eq!(
            b.status(),
            wcontainers_gpu::DeviceStatus::Ok,
            "the back end was lost as it was made: {}",
            b.last_log()
        );
        b
    }

    fn window(b: &mut WebGl) -> u32 {
        const WINDOW: u32 = 1;
        // SAFETY: the window is never destroyed while the test runs.
        unsafe { b.add_window(WINDOW, window::open()) };
        WINDOW
    }

    fn shown(_: &mut WebGl, _: u32) -> Vec<u8> {
        window::shown()
    }
}

/// The test's window: a 4 by 4 popup, read as the compositor shows it.
mod window {
    use std::cell::Cell;
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Dwm::DwmFlush;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::Storage::Xps::PrintWindow;
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetClientRect, MSG, PM_REMOVE, PW_RENDERFULLCONTENT,
        PeekMessageW, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    thread_local! {
        static OPEN: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    }

    /// Opens the window on this thread, which is the device thread.
    pub fn open() -> *mut core::ffi::c_void {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: a predefined window class, no parent, menu or creation data. Awareness of each
        // monitor's scale keeps the window 4 by 4 pixels.
        let hwnd = unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                4,
                4,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null(), "CreateWindowExW");
        OPEN.with(|h| h.set(hwnd));
        hwnd
    }

    /// The window's pixels after the compositor has shown the last frame, rows top first, RGBA with
    /// opaque alpha (a window holds none).
    pub fn shown() -> Vec<u8> {
        let hwnd = OPEN.with(Cell::get);
        // SAFETY: plain Win32 calls on this thread's window; every object made is released.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                DispatchMessageW(&msg);
            }
            // The composition the swapped frame entered, and one that shows it.
            DwmFlush();
            DwmFlush();
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(dc, bitmap);
            // Includes swap chain contents, also on a desktop no one sees.
            let printed = PrintWindow(hwnd, dc, PW_RENDERFULLCONTENT);
            SelectObject(dc, old);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let rows = GetDIBits(
                dc,
                bitmap,
                0,
                h as u32,
                bgra.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            );
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            assert_ne!(printed, 0, "PrintWindow");
            assert_eq!(rows, h, "GetDIBits");
            bgra.as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], 255])
                .collect()
        }
    }
}

wcontainers_gpu_test::suite!(Angle);

#[path = "../examples/common/checks.rs"]
mod checks;

/// Runs a check of checks.rs with the patterns' seed from `WEBGL_SEED` (default 0) and as many
/// seeds from it as `WEBGL_CASES` says (default 1). A check whose feature is absent passes having
/// run only the refusal, and prints "not run, feature absent:" with its name (libtest cannot skip
/// at run time; all.cmd lists these lines).
fn sampled(check: fn()) {
    let var = |name, default| std::env::var(name).map_or(default, |v| v.parse().expect(name));
    checks::seeded(check, var("WEBGL_SEED", 0), var("WEBGL_CASES", 1));
    let absent = checks::take_not_run();
    if !absent.is_empty() {
        let name = std::thread::current().name().unwrap_or("?").to_string();
        println!("not run, feature absent: {name}: {absent:?}");
    }
}
#[path = "../examples/common/kit.rs"]
mod kit;

#[test]
fn vertex_and_instance_numbers_include_the_bases() {
    sampled(checks::vertex_and_instance_numbers_include_the_bases::<Angle>);
}

#[test]
fn discard_then_no_overwrite_writes() {
    sampled(checks::discard_then_no_overwrite_writes::<Angle>);
}

#[test]
fn depth_and_stencil_read_back() {
    sampled(checks::depth_and_stencil_read_back::<Angle>);
}

#[test]
fn stencil_sampled_and_read_back() {
    sampled(checks::stencil_sampled_and_read_back::<Angle>);
}

#[test]
fn copies_within_a_typeless_group() {
    sampled(checks::copies_within_a_typeless_group::<Angle>);
}

#[test]
fn cube_faces_copied_between_typeless_formats() {
    sampled(checks::cube_faces_copied_between_typeless_formats::<Angle>);
}

#[test]
fn copies_within_one_subresource() {
    sampled(checks::copies_within_one_subresource::<Angle>);
}

#[test]
fn depth_and_stencil_written_read_back_as_written() {
    sampled(checks::depth_and_stencil_written_read_back_as_written::<Angle>);
}

#[test]
fn copies_between_depth_and_colour_formats() {
    sampled(checks::copies_between_depth_and_colour_formats::<Angle>);
}

#[test]
fn copies_between_index_and_data_buffers() {
    sampled(checks::copies_between_index_and_data_buffers::<Angle>);
}

#[test]
fn loads_count_levels_from_the_view() {
    sampled(checks::loads_count_levels_from_the_view::<Angle>);
}

#[test]
fn fractional_viewports_are_refused() {
    sampled(checks::fractional_viewports_are_refused::<Angle>);
}

#[test]
fn viewports_past_their_bounds_are_refused() {
    sampled(checks::viewports_past_their_bounds_are_refused::<Angle>);
}

#[test]
fn immediates_reach_both_stages() {
    sampled(checks::immediates_reach_both_stages::<Angle>);
}

#[test]
fn packed_vertex_formats_are_converted() {
    sampled(checks::packed_vertex_formats_are_converted::<Angle>);
}

#[test]
fn timestamps_and_their_disjoint_flag() {
    sampled(checks::timestamps_and_their_disjoint_flag::<Angle>);
}

#[test]
fn multisampled_copies_resolve_as_their_source() {
    sampled(checks::multisampled_copies_resolve_as_their_source::<Angle>);
}

#[test]
fn fractional_depth_bias() {
    sampled(checks::fractional_depth_bias::<Angle>);
}

#[test]
fn lists_draw_the_largest_index_as_a_vertex() {
    sampled(checks::lists_draw_the_largest_index_as_a_vertex::<Angle>);
}

#[test]
fn constant_alpha_blends() {
    sampled(checks::constant_alpha_blends::<Angle>);
}

#[test]
fn vertices_captured_with_and_without_rasterising() {
    sampled(checks::vertices_captured_with_and_without_rasterising::<Angle>);
}

#[test]
fn block_compressed_and_shared_exponent_textures_read_back_as_written() {
    sampled(checks::block_compressed_and_shared_exponent_textures_read_back_as_written::<Angle>);
}

#[test]
fn interpolated_bc4_and_bc5_values() {
    sampled(checks::interpolated_bc4_and_bc5_values::<Angle>);
}

#[test]
fn fragment_depth_follows_the_clip_convention() {
    sampled(checks::fragment_depth_follows_the_clip_convention::<Angle>);
}

#[test]
fn captured_indices_drawn_both_ways() {
    sampled(checks::captured_indices_drawn_both_ways::<Angle>);
}

#[test]
fn strips_fans_loops_and_indexed_draws_captured_in_list_order() {
    sampled(checks::strips_fans_loops_and_indexed_draws_captured_in_list_order::<Angle>);
}

#[test]
fn absent_features_are_refused() {
    sampled(checks::absent_features_are_refused::<Angle>);
}

#[test]
fn copies_into_and_out_of_host_kept_textures() {
    sampled(checks::copies_into_and_out_of_host_kept_textures::<Angle>);
}

#[test]
fn srgb_views_of_block_compressed_textures() {
    sampled(checks::srgb_views_of_block_compressed_textures::<Angle>);
}

#[test]
fn bgra_textures_keep_their_byte_order() {
    sampled(checks::bgra_textures_keep_their_byte_order::<Angle>);
}

#[test]
fn sampled_views_are_whole_textures_of_one_mip_range() {
    sampled(checks::sampled_views_are_whole_textures_of_one_mip_range::<Angle>);
}

#[test]
fn generate_mipmap_equals_the_draws() {
    sampled(checks::generate_mipmap_equals_the_draws::<Angle>);
}

#[test]
fn sixteen_bit_normalised_formats_follow_their_extension() {
    sampled(checks::sixteen_bit_normalised_formats_follow_their_extension::<Angle>);
}

#[test]
fn bgra_surfaces_present() {
    sampled(checks::bgra_surfaces_present::<Angle>);
}

#[test]
fn signed_normalised_formats_follow_their_extension() {
    sampled(checks::signed_normalised_formats_follow_their_extension::<Angle>);
}

#[test]
fn levels_stay_as_written_while_others_are_read_and_drawn() {
    sampled(checks::levels_stay_as_written_while_others_are_read_and_drawn::<Angle>);
}

#[test]
fn passes_without_attachments_rasterise() {
    sampled(checks::passes_without_attachments_rasterise::<Angle>);
}

#[test]
fn independent_blends_per_target() {
    sampled(checks::independent_blends_per_target::<Angle>);
}

#[test]
fn second_source_blends() {
    sampled(checks::second_source_blends::<Angle>);
}

#[test]
fn depth_clamped_to_the_viewport_range() {
    sampled(checks::depth_clamped_to_the_viewport_range::<Angle>);
}

#[test]
fn depth_bias_clamped() {
    sampled(checks::depth_bias_clamped::<Angle>);
}

#[test]
fn polygons_filled_with_lines() {
    sampled(checks::polygons_filled_with_lines::<Angle>);
}

#[test]
fn mirror_once_addressing() {
    sampled(checks::mirror_once_addressing::<Angle>);
}

#[test]
fn clip_and_cull_distances() {
    sampled(checks::clip_and_cull_distances::<Angle>);
}

#[test]
fn points_sized_by_the_program() {
    sampled(checks::points_sized_by_the_program::<Angle>);
}

#[test]
fn broken_rules_lose_the_device() {
    sampled(checks::broken_rules_lose_the_device::<Angle>);
}

#[test]
fn copies_within_one_float_format_take_the_probed_route() {
    sampled(checks::copies_within_one_float_format_take_the_probed_route::<Angle>);
}
