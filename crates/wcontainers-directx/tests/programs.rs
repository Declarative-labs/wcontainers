//! Our own Direct3D 11 programs (the examples `own_handled`, `own_refused`, `own_edges`,
//! `own_depth_copy`, `own_swap_chain`, `own_volumes`, `own_queries`, `own_interfaces`,
//! `own_indirect`, `own_min_lod`, `own_packed`, `own_nv12`, `own_yuy2`, `own_cases`,
//! `own_level10`, `own_d3d10`, `own_shared` and `lifetimes`) run as ordinary programs, once beside the system's d3d11.dll and once with our
//! forwarding d3d11.dll and library beside them, and what each printed compared.
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{fresh, place_libraries, profile};
use wcontainers_directx::com::FEATURES_OFF;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

/// Runs example `name` from `dir` (copying it there) with `args`, returning its exit code and output.
fn run_with(name: &str, dir: &Path, args: &[&str]) -> (i32, String) {
    run_env(name, dir, args, &[])
}

/// As [`run_with`], with environment variables `env` set.
fn run_env(name: &str, dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    let (code, text, _) = run_logged(name, dir, args, env);
    (code, text)
}

/// As [`run_env`], also returning what the program wrote to its error output.
fn run_logged(
    name: &str,
    dir: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> (i32, String, String) {
    let exe = dir.join(format!("{name}.exe"));
    std::fs::copy(profile().join(format!("examples/{name}.exe")), &exe).unwrap();
    let out = Command::new(&exe)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let errors = String::from_utf8_lossy(&out.stderr).into_owned();
    eprintln!("{name} in {}:\n{text}{errors}", dir.display());
    (out.status.code().unwrap_or(-1), text, errors)
}

fn run(name: &str, dir: &Path) -> (i32, String) {
    run_with(name, dir, &[])
}

/// A directory with nothing but what `name` needs: with ours, the forwarder and the library.
fn place(label: &str, ours: bool) -> PathBuf {
    let dir = fresh(label);
    if ours {
        place_libraries(&dir);
    }
    dir
}

/// The value of the line of `text` starting `key `.
fn line<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(' '))
        .unwrap_or("")
}

#[test]
fn own_handled_draws_the_same_through_ours() {
    let (code, text) = run("own_handled", &place("handled-system", false));
    assert_eq!((code, text.lines().last()), (0, Some("pass")), "{text}");
    let (code, ours) = run("own_handled", &place("handled-ours", true));
    assert_eq!((code, ours.lines().last()), (0, Some("pass")), "{ours}");
    assert_eq!(line(&text, "adapter"), line(&ours, "adapter"));
    let system = line(&text, "D3D11CreateDevice in").to_ascii_lowercase();
    assert!(system.ends_with("\\system32\\d3d11.dll"), "{system}");
    let ours = line(&ours, "D3D11CreateDevice in").to_ascii_lowercase();
    assert!(
        ours.ends_with("\\handled-ours\\wcontainers_directx.dll"),
        "{ours}"
    );
}

#[test]
fn own_refused_cases_are_served_through_ours_as_direct3d_serves_them() {
    let (code, text) = run("own_refused", &place("refused-system", false));
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        text,
        "CreateTexture2D(keyed mutex) 0x00000000\nCreateQuery(occlusion) 0x00000000\nCreateQuery(timestamp) 0x00000000\n\
         CreateBuffer(structured) 0x00000000\nCreateShaderResourceView(structured) 0x00000000\nCheckMultisampleQualityLevels(4) 0x00000000 1\n\
         CreateTexture2D(4 samples) 0x00000000\nGetDeviceRemovedReason 0x00000000\n"
    );
    let (code, ours) = run("own_refused", &place("refused-ours", true));
    assert_eq!(code, 0, "{ours}");
    assert_eq!(ours, text);
}

#[test]
fn deferred_contexts_states_and_resolves_answer_as_direct3d_does() {
    let (code, want) = run("own_deferred", &place("deferred-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_deferred", &place("deferred-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn own_edges_read_and_copy_as_direct3d_does() {
    let (code, want) = run("own_edges", &place("edges-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_edges", &place("edges-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn indirect_draws_read_their_bases_as_direct3d_does() {
    let (code, want) = run("own_indirect", &place("indirect-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_indirect", &place("indirect-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn two_pixel_formats_sample_load_and_copy_as_direct3d_does() {
    let (code, want) = run("own_packed", &place("packed-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_packed", &place("packed-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn resources_shared_between_devices_answer_and_read_as_direct3d_does() {
    let (code, want) = run("own_shared", &place("shared-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_shared", &place("shared-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn yuy2_pixel_pairs_view_write_copy_and_map_as_direct3d_does() {
    yuy2_compared(&[]);
}

/// A known failure: `GetDimensions` of a YUY2 view reports the texture's width in pixels on the
/// RTX 3080 and in pixel pairs, the view's texels, through ours (no helper value gives a program
/// another width than its texture's).
#[test]
#[ignore = "GetDimensions of YUY2 views: pixels on the RTX 3080, pixel pairs through ours"]
fn yuy2_view_sizes_read_as_direct3d_reads_them() {
    yuy2_compared(&["sizes"]);
}

fn yuy2_compared(args: &[&str]) {
    let (code, want) = run_with("own_yuy2", &place("yuy2-system", false), args);
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_yuy2", &place("yuy2-ours", true), args);
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn nv12_planes_view_write_copy_and_map_as_direct3d_does() {
    // The WebGL 2 build's device is 10_0's, so both are made at 10_0 there.
    let args: &[&str] = if cfg!(feature = "webgl") {
        &["10_0"]
    } else {
        &[]
    };
    let (code, want) = run_with("own_nv12", &place("nv12-system", false), args);
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_nv12", &place("nv12-ours", true), args);
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

/// Runs one case of `own_cases` beside both libraries and compares what each printed: lines
/// naming a filtered result within one unit a channel (Direct3D's filtering tolerance), every
/// other line exactly.
fn case(name: &str) {
    case_env(name, "ours", &[]);
}

/// As [`case`], ours run with environment variables `env` set (in a directory labelled `label`),
/// returning what ours wrote to its error output.
fn case_env(name: &str, label: &str, env: &[(&str, &str)]) -> String {
    case_args(&[name], label, env)
}

/// As [`case_env`], the case and its further arguments (a feature level) given as `args`.
fn case_args(args: &[&str], label: &str, env: &[(&str, &str)]) -> String {
    let name = args.join("-");
    let (code, want) = run_with("own_cases", &place(&format!("{name}-system"), false), args);
    assert_eq!(code, 0, "{want}");
    let (code, got, errors) = run_logged(
        "own_cases",
        &place(&format!("{name}-{label}"), true),
        args,
        env,
    );
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        if w.starts_with("filtered") {
            assert!(
                within_one(w, g),
                "the first line that differs by more than one\n{w}\n{g}"
            );
        } else {
            assert_eq!(w, g, "the first line that differs");
        }
    }
    assert_eq!(want.lines().count(), got.lines().count());
    errors
}

/// Runs one case of `own_level10` beside both libraries and compares what each printed, line by
/// line. Built on WebGL 2, ours runs without OES_draw_buffers_indexed, as browsers may.
fn level10(name: &str) {
    let label = |side: &str| place(&format!("level10-{name}-{side}"), side == "ours");
    let (code, want) = run_with("own_level10", &label("system"), &[name]);
    assert_eq!(code, 0, "{want}");
    let off: &[(&str, &str)] = if cfg!(feature = "webgl") {
        &[(FEATURES_OFF, "OES_draw_buffers_indexed")]
    } else {
        &[]
    };
    let (code, got) = run_env("own_level10", &label("ours"), &[name], off);
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn targets_blended_and_masked_each_their_own_way_at_10_0_as_direct3d_does() {
    level10("targets");
}

#[test]
fn view_sizes_and_level_counts_at_10_0_as_direct3d_does() {
    level10("resinfo");
}

#[test]
fn an_empty_slots_size_and_level_count_at_10_0_as_direct3d_does() {
    level10("resinfo_empty");
}

#[test]
fn stream_output_of_a_vertex_shader_alone_at_10_0_as_direct3d_does() {
    level10("stream");
}

/// A vertex shader reads a `B8G8R8X8_UNORM` element's three colours and its fourth component as 1
/// (on the RTX 3080; Wine's test says AMD's is undefined). A known failure: the interface has no
/// vertex format reading the X byte as 1, so ours reports no vertex buffers of the format and
/// refuses the layout, as Direct3D refuses a format it does not report (Gap list).
#[test]
#[ignore]
fn b8g8r8x8_elements_read_as_direct3d_reads_them() {
    case("bgrx");
}

/// `R10G10B10_XR_BIAS_A2_UNORM` textures take the shapes, bindings, CPU access and copies
/// Direct3D gives them, keeping their bytes.
#[test]
fn xr_bias_textures_as_direct3d_does() {
    case("xr");
}

#[test]
fn texture_atomics_as_direct3d_does() {
    case("atomics");
}

#[test]
fn raw_signed_atomics_from_pixels_and_threads_as_direct3d_does() {
    case("raw_signed_atomics");
}

#[test]
fn typed_storage_stores_as_direct3d_does() {
    case("typed");
}

/// Format support bits of abilities this device reports absent through their own queries, which
/// real Direct3D may have and ours never claims: video, the first word's bits from
/// `DECODER_OUTPUT` up (`ID3D11VideoDevice` is not served), logic
/// operations (`D3D11_OPTIONS.OutputMergerLogicOp`), tiled resources
/// (`D3D11_OPTIONS1.TiledResourcesTier`) and multiplane overlays
/// (`IDXGIOutput2::SupportsOverlays`).
const ABSENT_SUPPORT1: i32 = !(D3D11_FORMAT_SUPPORT_DECODER_OUTPUT.0 - 1);
/// Multisampling bits, claimed where the back end draws some sample count of the format
/// (`CheckMultisampleQualityLevels` answers which); real Direct3D's further counts are the plan's
/// listed shortfall (section 9), so ours may lack these bits where real Direct3D has them, never
/// the other way.
const MULTISAMPLE_SUPPORT1: i32 = D3D11_FORMAT_SUPPORT_MULTISAMPLE_RENDERTARGET.0
    | D3D11_FORMAT_SUPPORT_MULTISAMPLE_LOAD.0
    | D3D11_FORMAT_SUPPORT_MULTISAMPLE_RESOLVE.0;
const ABSENT_SUPPORT2: i32 = D3D11_FORMAT_SUPPORT2_OUTPUT_MERGER_LOGIC_OP.0
    | D3D11_FORMAT_SUPPORT2_TILED.0
    | D3D11_FORMAT_SUPPORT2_MULTIPLANE_OVERLAY.0;

/// Video formats Microsoft's tables leave optional at 11_0, which ours reports absent altogether.
const OPTIONAL_FORMATS_ABSENT: &[DXGI_FORMAT] = {
    use windows::Win32::Graphics::Dxgi::Common::*;
    &[
        DXGI_FORMAT_AYUV,
        DXGI_FORMAT_Y410,
        DXGI_FORMAT_Y416,
        DXGI_FORMAT_P010,
        DXGI_FORMAT_P016,
        DXGI_FORMAT_Y210,
        DXGI_FORMAT_Y216,
        DXGI_FORMAT_NV11,
        DXGI_FORMAT_AI44,
        DXGI_FORMAT_IA44,
        DXGI_FORMAT_P8,
        DXGI_FORMAT_A8P8,
    ]
};

/// Formats whose support differs from the RTX 3080's at feature level `level` (`"11_0"`,
/// `"10_1"`, `"10_0"` or one of 9_x) outside the absent abilities: each with the bits of both words that
/// differ and why (the status file's Gap list names each). Below 11_0 no format claims storage
/// views, and only formats of a red channel alone claim gathers (at 10_1), so those differences
/// are 11_0's alone.
fn support_differences(level: &str) -> Vec<(DXGI_FORMAT, i32, i32, &'static str)> {
    use windows::Win32::Graphics::Dxgi::Common::*;
    const TARGET: i32 = D3D11_FORMAT_SUPPORT_RENDER_TARGET.0
        | D3D11_FORMAT_SUPPORT_BLENDABLE.0
        | D3D11_FORMAT_SUPPORT_MIP_AUTOGEN.0;
    let at_11_0 = level == "11_0";
    // Below 10_0 the 10Level9 tables give none of the other formats' named abilities.
    if level.starts_with("9_") {
        return vec![(
            DXGI_FORMAT_B5G6R5_UNORM,
            TARGET,
            0,
            "B5G6R5 targets: the back end has no such target",
        )];
    }
    let gather = if at_11_0 {
        D3D11_FORMAT_SUPPORT_SHADER_GATHER.0
    } else {
        0
    };
    let mut differences = vec![
        (
            DXGI_FORMAT_R32G32B32_FLOAT,
            D3D11_FORMAT_SUPPORT_SHADER_SAMPLE.0 | gather,
            0,
            "sampling, optional: served, where the back end filters the format",
        ),
        (
            DXGI_FORMAT_B5G6R5_UNORM,
            TARGET,
            0,
            "B5G6R5 targets: the back end has no such target",
        ),
        (
            DXGI_FORMAT_B5G5R5A1_UNORM,
            TARGET,
            0,
            "B5G5R5A1 targets: the back end has no such target",
        ),
        (
            DXGI_FORMAT_B8G8R8X8_UNORM,
            D3D11_FORMAT_SUPPORT_BUFFER.0 | D3D11_FORMAT_SUPPORT_IA_VERTEX_BUFFER.0,
            0,
            "B8G8R8X8 elements: the interface has no vertex format giving X as 1",
        ),
        (
            DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM,
            D3D11_FORMAT_SUPPORT_DISPLAY.0 | D3D11_FORMAT_SUPPORT_BACK_BUFFER_CAST.0,
            0,
            "extended-range swap chains: the infrastructure crate makes none",
        ),
    ];
    if at_11_0 {
        differences.push((
            DXGI_FORMAT_A8_UNORM,
            D3D11_FORMAT_SUPPORT_TYPED_UNORDERED_ACCESS_VIEW.0,
            D3D11_FORMAT_SUPPORT2_UAV_TYPED_LOAD.0 | D3D11_FORMAT_SUPPORT2_UAV_TYPED_STORE.0,
            "A8_UNORM storage views: the shaders crate has no storage form of it",
        ));
    }
    differences
}

/// Both words of every format's support, and whether the second query succeeds, as real Direct3D
/// answers them, every bit compared: the bits of absent abilities are never claimed, multisampling
/// is claimed only where real Direct3D claims it, and the others differ only as
/// `support_differences` names. Direct3D fails the second query where a format has none of its
/// bits, so it fails on ours where real Direct3D's word holds only bits ours lacks.
#[test]
fn format_support_as_direct3d_reports_it_but_the_named_shortfalls() {
    format_support_compared("11_0", "ours", &[], &[]);
}

/// The same on devices of feature level 10_1, which claim gathers only of formats of a red
/// channel alone and of the video formats, no storage views and, in the second word, sharing
/// alone.
#[cfg(not(feature = "webgl"))]
#[test]
fn format_support_at_level_10_1_as_direct3d_reports_it_but_the_named_shortfalls() {
    format_support_compared("10_1", "ours", &[], &[]);
}

/// The same on devices of feature level 10_0, which also read no multisampled depth. The WebGL 2
/// build's own 10_0 table is compared in the Gap list's WebGL 2 item, not here.
#[cfg(not(feature = "webgl"))]
#[test]
fn format_support_at_level_10_0_as_direct3d_reports_it_but_the_named_shortfalls() {
    format_support_compared("10_0", "ours", &[], &[]);
}

/// The same on devices of feature levels 9_1, 9_2 and 9_3, which report only what the 10Level9
/// tables give; and what such devices make and refuse, line by line.
#[cfg(not(feature = "webgl"))]
#[test]
fn format_support_and_creations_below_10_0_as_direct3d_reports_them() {
    for level in ["9_1", "9_2", "9_3"] {
        format_support_compared(level, "ours", &[], &[]);
        case_args(&["level9_make", level], "ours", &[]);
    }
}

/// The same where the back end lacks the 16-bit normalised formats (a browser without
/// `texture-formats-tier1`): they are held as their integer twins (`NormAsInteger`) and so not
/// blended (`BlendInShader` is not built), and claim nothing else Direct3D does not.
#[cfg(not(feature = "webgl"))]
#[test]
fn format_support_without_16_bit_norm_as_direct3d_reports_it_but_the_named_shortfalls() {
    use windows::Win32::Graphics::Dxgi::Common::*;
    let off = wgpu::Features::TEXTURE_FORMAT_16BIT_NORM.as_str().unwrap();
    let unblended = |f| {
        (
            f,
            D3D11_FORMAT_SUPPORT_BLENDABLE.0,
            0,
            "blending into integer twins",
        )
    };
    let twins = [
        DXGI_FORMAT_R16G16B16A16_UNORM,
        DXGI_FORMAT_R16G16B16A16_SNORM,
        DXGI_FORMAT_R16G16_UNORM,
        DXGI_FORMAT_R16G16_SNORM,
        DXGI_FORMAT_R16_UNORM,
        DXGI_FORMAT_R16_SNORM,
    ]
    .map(unblended);
    format_support_compared("11_0", "ours-no16", &[(FEATURES_OFF, off)], &twins);
}

/// The 8- and 16-bit normalised formats a back end without `texture-formats-tier1` samples but
/// does not draw into: drawn into as their integer twins (`NormAsInteger`), so not blended
/// (`BlendInShader` is not built).
#[cfg(not(feature = "webgl"))]
fn twin_targets_unblended() -> Vec<(DXGI_FORMAT, i32, i32, &'static str)> {
    use windows::Win32::Graphics::Dxgi::Common::*;
    [
        DXGI_FORMAT_R8G8B8A8_SNORM,
        DXGI_FORMAT_R8G8_SNORM,
        DXGI_FORMAT_R8_SNORM,
        DXGI_FORMAT_R16G16B16A16_UNORM,
        DXGI_FORMAT_R16G16B16A16_SNORM,
        DXGI_FORMAT_R16G16_UNORM,
        DXGI_FORMAT_R16G16_SNORM,
        DXGI_FORMAT_R16_UNORM,
        DXGI_FORMAT_R16_SNORM,
    ]
    .map(|f| {
        (
            f,
            D3D11_FORMAT_SUPPORT_BLENDABLE.0,
            0,
            "blending into integer twins",
        )
    })
    .to_vec()
}

/// `WEBGPU_OFF` as a browser without `texture-formats-tier1` has it (natively, the adapter's own
/// format features off), with `more` features off too.
#[cfg(not(feature = "webgl"))]
fn untiered(more: &[wgpu::Features]) -> String {
    std::iter::once(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        .chain(more.iter().copied())
        .map(|f| f.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The same without `texture-formats-tier1`: the normalised formats the back end does not draw
/// into are drawn as their integer twins and not blended; depth formats claim no storage views.
#[cfg(not(feature = "webgl"))]
#[test]
fn format_support_without_texture_formats_tier1_as_direct3d_reports_it_but_the_named_shortfalls() {
    let off = untiered(&[]);
    let twins = twin_targets_unblended();
    format_support_compared("11_0", "ours-untiered", &[(FEATURES_OFF, &off)], &twins);
}

/// The same where the back end cannot filter the 32-bit float formats either (a browser without
/// `float32-filterable`, which has no `texture-formats-tier1` either): they are sampled, gathered
/// and their mips generated by filtering in the program, so nothing else differs.
#[cfg(not(feature = "webgl"))]
#[test]
fn format_support_without_float32_filterable_as_direct3d_reports_it_but_the_named_shortfalls() {
    let off = untiered(&[wgpu::Features::FLOAT32_FILTERABLE]);
    let twins = twin_targets_unblended();
    format_support_compared("11_0", "ours-unfiltered", &[(FEATURES_OFF, &off)], &twins);
}

/// Gathers through point and linear samplers, samples and generated mips of the 32-bit float
/// formats as real Direct3D reads them, natively and filtered in the program (a browser without
/// `float32-filterable`).
#[cfg(not(feature = "webgl"))]
#[test]
fn float32_textures_gathered_sampled_and_their_mips_generated_as_direct3d_does() {
    case("float32_reads");
    let off = untiered(&[wgpu::Features::FLOAT32_FILTERABLE]);
    let errors = case_env("float32_reads", "unfiltered", &[(FEATURES_OFF, &off)]);
    let route = wcontainers_gpu::SlowPath::FilterInShader.reason();
    assert!(errors.contains(route), "{errors}");
}

/// The 8- and 16-bit normalised formats drawn into, read back and their mips generated as real
/// Direct3D does, natively and, without `texture-formats-tier1`, through their integer twins.
#[cfg(not(feature = "webgl"))]
#[test]
fn normalised_targets_drawn_and_their_mips_generated_as_direct3d_does() {
    case("norm_targets");
    let off = untiered(&[]);
    let errors = case_env("norm_targets", "untiered", &[(FEATURES_OFF, &off)]);
    let route = wcontainers_gpu::SlowPath::NormAsInteger.reason();
    assert!(errors.contains(route), "{errors}");
}

/// Without a route for a partial sample mask (WebGL 2 without the back end's mask; plan section
/// 9), no format reports multisampling at 10_0, and so no multisampled texture is made.
#[cfg(feature = "webgl")]
#[test]
fn no_multisampling_without_a_sample_mask_route_at_10_0() {
    let (code, got) = run_with(
        "own_cases",
        &place("format_support-10_0-ours-unmasked", true),
        &["format_support", "10_0"],
    );
    assert_eq!(code, 0, "{got}");
    for l in got.lines() {
        if let Some(rest) = l.strip_prefix("msq ") {
            assert!(rest.ends_with(" 0"), "{l}");
        }
        if let Some(rest) = l.strip_prefix("format ") {
            let support1 =
                u32::from_str_radix(rest.split(' ').nth(2).unwrap().trim_start_matches("0x"), 16)
                    .unwrap() as i32;
            assert_eq!(support1 & MULTISAMPLE_SUPPORT1, 0, "{l}");
        }
    }
}

/// Compares every format's support on a device of feature level `level` through ours, run with
/// `env` in a directory labelled `label`, with real Direct3D's, allowing `support_differences`
/// and `more` (whose bits are added).
fn format_support_compared(
    level: &str,
    label: &str,
    env: &[(&str, &str)],
    more: &[(DXGI_FORMAT, i32, i32, &str)],
) {
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("format_support-{level}-system"), false),
        &["format_support", level],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_env(
        "own_cases",
        &place(&format!("format_support-{level}-{label}"), true),
        &["format_support", level],
        env,
    );
    assert_eq!(code, 0, "{got}");
    let words = |l: &str| -> (i32, [i32; 4]) {
        let mut w = l.split(' ').skip(1);
        let format = w.next().unwrap().parse().unwrap();
        let mut hex =
            w.map(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).unwrap() as i32);
        (format, std::array::from_fn(|_| hex.next().unwrap()))
    };
    let formats = |t: &str| {
        t.lines()
            .filter(|l| l.starts_with("format "))
            .map(String::from)
            .collect::<Vec<_>>()
    };
    let options2 = |t: &str| {
        t.lines()
            .find(|l| l.starts_with("options2 "))
            .map(String::from)
    };
    assert_eq!(options2(&want), options2(&got));
    // Quality levels at four samples: the same answer, and levels only where real Direct3D has them.
    let levels = |t: &str| -> Vec<(i32, i32, u32)> {
        t.lines()
            .filter_map(|l| l.strip_prefix("msq "))
            .map(|l| {
                let w: Vec<&str> = l.split(' ').collect();
                let code = u32::from_str_radix(w[1].trim_start_matches("0x"), 16).unwrap();
                (w[0].parse().unwrap(), code as i32, w[2].parse().unwrap())
            })
            .collect()
    };
    let (want_levels, got_levels) = (levels(&want), levels(&got));
    assert_eq!(want_levels.len(), got_levels.len());
    for (w, g) in want_levels.iter().zip(&got_levels) {
        if OPTIONAL_FORMATS_ABSENT.contains(&DXGI_FORMAT(w.0)) {
            continue;
        }
        assert!(
            w.1 == g.1 && g.2 <= w.2,
            "quality levels at 4 samples {w:?} ours {g:?}"
        );
    }
    let (want, got) = (formats(&want), formats(&got));
    let mut differing = Vec::new();
    for (w, g) in want.iter().zip(&got) {
        let (format, [want_code, want1, want2, want_first]) = words(w);
        let (_, [got_code, got1, got2, got_first]) = words(g);
        if OPTIONAL_FORMATS_ABSENT.contains(&DXGI_FORMAT(format)) {
            assert_eq!((got1, got2), (0, 0), "format {format}");
            continue;
        }
        let claimed = (
            got1 & (ABSENT_SUPPORT1 | MULTISAMPLE_SUPPORT1 & !want1),
            got2 & ABSENT_SUPPORT2,
        );
        let diff1 = (want1 ^ got1) & !(ABSENT_SUPPORT1 | MULTISAMPLE_SUPPORT1);
        let diff2 = (want2 ^ got2) & !ABSENT_SUPPORT2;
        // `R32G32B32_FLOAT`'s optional sampling is served only where the back end filters it.
        let unfiltered = env
            .iter()
            .any(|(_, off)| off.contains(wgpu::Features::FLOAT32_FILTERABLE.as_str().unwrap()));
        let named = support_differences(level)
            .iter()
            .filter(|s| {
                !(unfiltered
                    && s.0 == windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32G32B32_FLOAT)
            })
            .chain(more)
            .filter(|s| s.0 == DXGI_FORMAT(format))
            .fold((0, 0), |(a, b), s| (a | s.1, b | s.2));
        // Direct3D fails the second query of a word without bits.
        // The first query fails exactly where real Direct3D's fails.
        let codes_agree =
            (want_code == got_code || got2 == 0 && got_code != 0) && want_first == got_first;
        if claimed != (0, 0) || (diff1, diff2) != named || !codes_agree {
            differing.push(format!(
                "format {format}: support {want1:#010x} ours {got1:#010x} (differing {diff1:#x}), \
                 support2 {want_code:#x} {want2:#010x} ours {got_code:#x} {got2:#010x} \
                 (differing {diff2:#x}), absent bits claimed {claimed:x?}, first query \
                 {want_first:#x} ours {got_first:#x}"
            ));
        }
    }
    assert_eq!(want.len(), got.len());
    assert!(differing.is_empty(), "{}", differing.join("\n"));
}

/// `ClearView` clears targets' and storage views' rectangles, clipped to the view, in each
/// layer or slice, whole views of buffers and of 1D, 2D and 3D textures, and the depth of depth
/// views; integer values past 32 bits and the infinities are converted as the RTX 3080's target
/// clears convert them; `D3D11_OPTIONS.ClearView` is TRUE.
#[test]
fn clear_view_of_rectangles_and_storage_as_direct3d_does() {
    case("clear_view");
}

/// `ClearDepthStencilView` with a depth outside 0 to 1 (NaN and the infinities among them) is
/// dropped whole, as Direct3D drops it; `ClearView` of normalised depth limits the value to 0 to 1.
#[test]
fn depth_clears_outside_the_range_as_direct3d_does() {
    case("depth_clears");
}

/// Comparisons and gathers with comparison through views of `R32_FLOAT` and `R16_UNORM` colour
/// textures (Microsoft's 11_0 table requires both), read through depth copies of their texels,
/// as real Direct3D reads them; and the same with 16-bit normalised formats held as their integer
/// twins.
#[test]
fn comparisons_through_colour_views_as_direct3d_does() {
    case("compare_colour");
    case_env(
        "compare_colour",
        "no16",
        &[(FEATURES_OFF, "wgpu-texture-format-16-bit-norm")],
    );
}

/// The same without `texture-formats-tier1` (natively, the adapter's own format features): a
/// storage view of `R8G8B8A8_SNORM`, which no pass then draws into, is cleared by the storage
/// clear program, whole or its rectangles.
#[cfg(not(feature = "webgl"))]
#[test]
fn clear_view_without_texture_formats_tier1_as_direct3d_does() {
    let tier1 = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        .as_str()
        .unwrap();
    case_env("clear_view", "untiered", &[(FEATURES_OFF, tier1)]);
}

/// Four-sample depth textures made for shaders to read, refused below 10_1, and views of each
/// aspect.
#[cfg(not(feature = "webgl"))]
#[test]
fn multisampled_depth_read_by_shaders_as_direct3d_takes_it_at_each_level() {
    for level in ["11_0", "10_1", "10_0"] {
        case_args(&["ms_depth_read", level], "ours", &[]);
    }
}

/// A compute shader sampling, loading and gathering under resource clamps takes the clamped
/// levels, as a pixel shader does.
#[cfg(not(feature = "webgl"))]
#[test]
fn a_compute_shader_samples_under_minimum_level_clamps_as_direct3d_does() {
    case("cs_min_lod");
}

/// Without `texture-formats-tier1` (natively, the adapter's own format features), the formats
/// WebGPU cannot store into are stored through a buffer of their texels.
#[cfg(not(feature = "webgl"))]
#[test]
fn typed_storage_stores_without_texture_formats_tier1_as_direct3d_does() {
    let tier1 = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        .as_str()
        .unwrap();
    let errors = case_env("typed", "untiered", &[(FEATURES_OFF, tier1)]);
    let route = wcontainers_gpu::SlowPath::StorageFormatThroughBuffer.reason();
    assert!(errors.contains(route), "{errors}");
}

/// The same without the 16-bit normalised formats either, as a browser without
/// `texture-formats-tier1` lacks them: `R16G16_UNORM` is then held as its integer twin
/// (`NormAsInteger`) and stored through a buffer.
#[cfg(not(feature = "webgl"))]
#[test]
fn typed_storage_stores_without_texture_formats_tier1_or_16_bit_norm_as_direct3d_does() {
    let off = [
        wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        wgpu::Features::TEXTURE_FORMAT_16BIT_NORM,
    ]
    .map(|f| f.as_str().unwrap())
    .join(" ");
    let errors = case_env("typed", "untiered16", &[(FEATURES_OFF, &off)]);
    let route = wcontainers_gpu::SlowPath::NormAsInteger.reason();
    assert!(errors.contains(route), "{errors}");
}

#[test]
fn a_storage_view_read_by_the_next_draw_on_the_same_targets_as_direct3d_does() {
    case("split");
}

#[test]
fn depth_and_colour_filtered_through_one_sampler_as_direct3d_does() {
    case("depth_filter");
}

#[test]
fn a_depth_buffer_larger_than_its_target_as_direct3d_does() {
    case("deep");
}

#[test]
fn copies_between_multisampled_textures_as_direct3d_does() {
    case("msaa_copy");
}

#[test]
fn stream_output_also_drawn_as_direct3d_does() {
    case("stream");
}

#[test]
fn early_depth_tests_before_storage_writes_as_direct3d_does() {
    case("early");
}

#[test]
fn early_depth_tests_before_storage_writes_writing_depth_as_direct3d_does() {
    case("early_writes");
}

#[test]
fn stencil_views_read_the_stencil_as_direct3d_does() {
    case("stencil");
}

#[test]
fn depth_only_draws_clamped_and_clipped_as_direct3d_does() {
    case("unshaded");
}

/// The same without the back end's depth clamping, which the pixel program writing nothing then
/// applies (`DepthClampInShader`).
#[cfg(not(feature = "webgl"))]
#[test]
fn depth_only_draws_clamped_in_the_program_as_direct3d_does() {
    let clamp = wgpu::Features::DEPTH_CLIP_CONTROL.as_str().unwrap();
    let errors = case_env("unshaded", "unclamped", &[(FEATURES_OFF, clamp)]);
    let route = wcontainers_gpu::SlowPath::DepthClampInShader.reason();
    assert!(errors.contains(route), "{errors}");
}

#[test]
fn a_depth_bias_on_lines_and_points_as_direct3d_does() {
    case("biased_lines");
}

#[test]
fn a_depth_bias_on_what_geometry_and_tessellation_stages_rasterise_as_direct3d_does() {
    case("staged_bias");
}

/// The rows of `text`'s lines starting `prefix`, each a row of bytes written in hex.
fn hex_rows(text: &str, prefix: &str) -> Vec<Vec<u8>> {
    text.lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .map(|l| {
            let hex = l.rsplit_once(' ').map_or(l, |(_, h)| h);
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect()
        })
        .collect()
}

/// Lines under `MultisampleEnable` are drawn by the back end's rule, a loss plan 4.12 lists as
/// at most the pixels at a line's ends and along its edges: every pixel real Direct3D covers in
/// all its samples is covered so, and a pixel may differ only where it touches one real
/// Direct3D covers.
#[test]
fn multisampled_lines_differ_from_direct3d_only_at_their_ends_and_edges() {
    let name = "msaa_lines";
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("{name}-system"), false),
        &[name],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_cases", &place(&format!("{name}-ours"), true), &[name]);
    assert_eq!(code, 0, "{got}");
    let prefix = "msaa lines: row ";
    let (w, g) = (hex_rows(&want, prefix), hex_rows(&got, prefix));
    assert!(!w.is_empty());
    assert_eq!((w.len(), w[0].len()), (g.len(), g[0].len()));
    // The most samples real Direct3D covers in a pixel: all of them, along a line's middle.
    let full = *w.iter().flatten().max().unwrap();
    let covered = |y: usize, x: usize| w[y][x] != 0;
    let touches = |y: usize, x: usize| {
        (y.saturating_sub(1)..=(y + 1).min(w.len() - 1))
            .any(|ny| (x.saturating_sub(1)..=(x + 1).min(w[0].len() - 1)).any(|nx| covered(ny, nx)))
    };
    for y in 0..w.len() {
        for x in 0..w[0].len() {
            let (a, b) = (w[y][x], g[y][x]);
            if a == full {
                assert_eq!(
                    b, full,
                    "a fully covered pixel at ({x}, {y})\n{want}\n{got}"
                );
            } else if a != b {
                assert!(
                    touches(y, x),
                    "a pixel off the line at ({x}, {y})\n{want}\n{got}"
                );
            }
        }
    }
    let rest = |t: &str| -> Vec<String> {
        t.lines()
            .filter(|l| !l.starts_with(prefix))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(rest(&want), rest(&got));
}

/// Aliased lines (`MultisampleEnable` FALSE) into four samples: every sample of each pixel a
/// line covers is hit, as Direct3D 11.3's functional specification requires (15.14), with no
/// leeway away from the viewport's edges (3.4.3).
#[test]
#[ignore = "WebGPU rasterises lines into a multisampled target by its own multisampled rule; the interface has no aliased-line route"]
fn aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does() {
    case("aliased_msaa_lines");
}

/// The most a channel of an alpha-antialiased line's pixel may differ from real Direct3D's, in
/// steps of 8 bits: the functional specification (3.4.4) leaves the coverage's filter to the
/// implementation, and the RTX 3080 and WARP differ by up to 128 (the shaders track's
/// `antialiased_lines_draw_as_direct3d_does`, which holds the stage helper's route to this).
const ANTIALIASED_LINE_ALLOWANCE: u8 = 64;

/// Alpha-antialiased lines, a list and a strip blended by their coverage: every channel within
/// the allowance of real Direct3D's, alpha (the coverage) included.
#[test]
fn alpha_antialiased_lines_blend_their_coverage_as_direct3d_does() {
    antialiased_lines("aa_lines");
}

/// The same lines as an indexed strip cut by the restart index.
#[test]
fn an_indexed_alpha_antialiased_strip_blends_its_coverage_as_direct3d_does() {
    antialiased_lines("aa_strip_indexed");
}

/// Runs own case `name`, alpha-antialiased lines, beside both libraries and compares the
/// target's bytes within the allowance and the rest line by line.
fn antialiased_lines(name: &str) {
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("{name}-system"), false),
        &[name],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_cases", &place(&format!("{name}-ours"), true), &[name]);
    assert_eq!(code, 0, "{got}");
    let prefix = "aa lines: row ";
    let (w, g) = (hex_rows(&want, prefix), hex_rows(&got, prefix));
    assert!(!w.is_empty());
    assert_eq!(w.len(), g.len(), "{got}");
    assert_eq!(w[0].len(), g[0].len());
    let texel = std::mem::size_of::<u32>();
    let drawn = |rows: &[Vec<u8>]| {
        rows.iter()
            .flat_map(|r| r.chunks(texel))
            .filter(|t| t[..texel - 1].iter().any(|c| *c != 0))
            .count()
    };
    let (drawn_want, drawn_got) = (drawn(&w), drawn(&g));
    assert!(drawn_want > 0 && drawn_got > 0, "{want}\n{got}");
    let mut largest = 0;
    for (y, (wr, gr)) in w.iter().zip(&g).enumerate() {
        for (x, (a, b)) in wr.iter().zip(gr).enumerate() {
            let d = a.abs_diff(*b);
            largest = largest.max(d);
            assert!(
                d <= ANTIALIASED_LINE_ALLOWANCE,
                "byte {x} of row {y}: {a} and {b}\n{want}\n{got}"
            );
        }
    }
    eprintln!(
        "{drawn_want} pixels drawn by direct3d, {drawn_got} by ours, largest difference {largest}"
    );
    let rest = |t: &str| -> Vec<String> {
        t.lines()
            .filter(|l| !l.starts_with(prefix))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(rest(&want), rest(&got));
}

#[test]
fn minimum_level_clamps_and_both_helper_blocks_as_direct3d_does() {
    let (code, want) = run("own_min_lod", &place("min-lod-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_min_lod", &place("min-lod-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        // A bias the program adds to an implicit level rounds the level apart from the sampler's
        // own on the RTX 3080 (one unit at 4 of 256 texels): within Direct3D's filtering
        // tolerance, one unit a channel. Everything else is exact.
        if w.contains("biased") {
            assert!(
                within_one(w, g),
                "the first line that differs by more than one\n{w}\n{g}"
            );
        } else {
            assert_eq!(w, g, "the first line that differs");
        }
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

/// Whether two printed rows name the same thing and their hex bytes differ by at most one each.
fn within_one(want: &str, got: &str) -> bool {
    let (Some((wn, wh)), Some((gn, gh))) = (want.rsplit_once(' '), got.rsplit_once(' ')) else {
        return want == got;
    };
    let byte = |h: &str, i: usize| i16::from_str_radix(&h[i..i + 2], 16).ok();
    wn == gn
        && wh.len() == gh.len()
        && (0..wh.len())
            .step_by(2)
            .all(|i| match (byte(wh, i), byte(gh, i)) {
                (Some(a), Some(b)) => (a - b).abs() <= 1,
                _ => wh[i..i + 2] == gh[i..i + 2],
            })
}

#[test]
fn one_and_three_dimensional_textures_as_direct3d_does() {
    let (code, want) = run("own_volumes", &place("volumes-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_volumes", &place("volumes-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn queries_answer_as_direct3d_does() {
    let (code, want) = run("own_queries", &place("queries-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_queries", &place("queries-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn interfaces_answer_as_direct3d_does() {
    let (code, want) = run("own_interfaces", &place("interfaces-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_interfaces", &place("interfaces-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
#[cfg_attr(
    feature = "webgl",
    ignore = "it makes an 11_0 device and a 10_1 state, and the WebGL 2 build reports 10_0 alone"
)]
fn direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does() {
    let (code, want) = run("own_d3d10", &place("d3d10-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_d3d10", &place("d3d10-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn a_swap_chain_presents_and_resizes_as_direct3d_does() {
    let (code, want) = run("own_swap_chain", &place("swap-chain-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_swap_chain", &place("swap-chain-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn depth_with_stencil_copies_as_direct3d_does() {
    let (code, want) = run("own_depth_copy", &place("depth-copy-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_depth_copy", &place("depth-copy-ours", true));
    assert_eq!(code, 0, "{got}");
    assert_eq!(want, got);
}

/// Seeds and steps of the `lifetimes` runs: `WC_LIFETIMES=<first seed>,<seeds>,<steps>`.
fn lifetime_cases() -> (u64, u64, usize) {
    let v: Vec<u64> = std::env::var("WC_LIFETIMES")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    match v[..] {
        [first, seeds, steps] => (first, seeds, steps as usize),
        _ => (1, 8, 300),
    }
}

#[test]
fn object_lifetimes_count_as_direct3d_counts_them() {
    let (first, seeds, steps) = lifetime_cases();
    let (system, ours) = (
        place("lifetimes-system", false),
        place("lifetimes-ours", true),
    );
    for seed in first..first + seeds {
        let args = [&seed.to_string()[..], &steps.to_string()[..]];
        let (code, want) = run_with("lifetimes", &system, &args);
        assert_eq!(code, 0, "{want}");
        let (code, got) = run_with("lifetimes", &ours, &args);
        assert_eq!(code, 0, "{got}");
        for (w, g) in want.lines().zip(got.lines()) {
            assert_eq!(w, g, "seed {seed}: the first line that differs");
        }
        assert_eq!(want.lines().count(), got.lines().count(), "seed {seed}");
    }
    eprintln!(
        "lifetimes: seeds {first}..{} of {steps} steps each",
        first + seeds
    );
}

/// `own_handled` with a depth-stencil view of `format`, beside the system's library and ours.
fn handled_with_depth(format: &str) {
    for ours in [false, true] {
        let dir = place(&format!("handled-{format}-{ours}"), ours);
        let (code, text) = run_with("own_handled", &dir, &[format]);
        assert_eq!((code, text.lines().last()), (0, Some("pass")), "{text}");
    }
}

#[test]
fn own_handled_draws_the_same_through_ours_with_32_bit_depth() {
    handled_with_depth("d32");
}

#[test]
fn own_handled_draws_the_same_through_ours_with_depth_and_stencil() {
    handled_with_depth("d24s8");
}
