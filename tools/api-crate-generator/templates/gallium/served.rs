//! The serving glue's first slice under its hostile C caller (caller.c): each case on a screen of the recording
//! implementation, alone in a process of its own; what C sees (the case's status) and what reached the implementation
//! (the recording's log, in order) are both checked. Built against the pinned Mesa with the `hostile` feature.
#![cfg(all(mesa, feature = "hostile"))]

use std::process::Command;
use std::sync::Arc;

use wcontainers_gallium_recording::Recording;

#[expect(unsafe_code, reason = "caller.c defines wc_caller_cases and wc_caller_case as declared here")]
unsafe extern "C" {
    fn wc_caller_cases() -> i32;
    fn wc_caller_case(address: usize, which: i32) -> i32;
    fn wc_caller_caps(address: usize) -> i32;
    fn wc_caller_two_screens(first: usize, second: usize) -> i32;
}

/// The environment variable naming the one case a child process runs.
const CASE_VARIABLE: &str = "WCONTAINERS_GALLIUM_CASE";

/// The methods serve.c calls while it makes the screen, before any case: each stage's capabilities (the eight of
/// MESA_SHADER_MESH_STAGES), the screen's, the compute capabilities.
const SCREEN_MADE: &[&str] = &["shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "caps", "compute_caps"];

/// What each case must leave in the log after the screen is made: the methods that reached the implementation, in order.
const EXPECTED: &[&[&str]] = &[
    &[
        "context_create",
        "create_blend_state",
        "bind_blend_state",
        "bind_blend_state",
        "delete_blend_state",
        "create_depth_stencil_alpha_state",
        "bind_depth_stencil_alpha_state",
        "delete_depth_stencil_alpha_state",
        "create_rasterizer_state",
        "bind_rasterizer_state",
        "delete_rasterizer_state",
        "create_sampler_state",
        "create_sampler_state",
        "bind_sampler_states",
        "delete_sampler_state",
        "delete_sampler_state",
        "create_vertex_elements_state",
        "bind_vertex_elements_state",
        "delete_vertex_elements_state",
        "destroy",
        "destroy",
    ],
    &["context_create", "destroy", "destroy"],
    &["context_create", "create_blend_state", "delete_blend_state", "destroy", "destroy"],
    &["context_create", "create_rasterizer_state", "delete_rasterizer_state", "destroy", "destroy"],
    &["context_create", "destroy", "destroy"],
    &["context_create", "destroy", "destroy"],
];

// FILLED
/// How every case makes its context: caller.c's private pointer (0x5a5a) and PIPE_CONTEXT_ROBUST_BUFFER_ACCESS (1 << 2).
const CONTEXT_CREATED: &str = "context_create priv_ 23130 flags 4";

/// The first case's whole log, as the implementation must receive it: each state as caller.c filled it (`FILLED`),
/// each object the recording made (numbered in the order made) bound, unbound and given back, the samplers bound at the
/// geometry stage (MESA_SHADER_GEOMETRY, 3) from slot 3.
fn first_case() -> Vec<String> {
    let object = |kind: &str, number: u32| format!("Object {{ kind: {kind:?}, number: {number} }}");
    // The screen made: each stage's capabilities, then the screen's and the compute ones, each as Mesa's defaults (the
    // trait's Default; `caps_are_mesas_defaults` holds them to Mesa's own function).
    let mut log: Vec<String> = (0..8).map(|stage| format!("shader_caps stage {:?} caps {:?}", wcontainers_gallium::values::MesaShaderStage(stage), wcontainers_gallium::ShaderCaps::default())).collect();
    log.push(format!("caps caps {:?}", wcontainers_gallium::Caps::default()));
    log.push(format!("compute_caps caps {:?}", wcontainers_gallium::ComputeCaps::default()));
    log.push(CONTEXT_CREATED.to_string());
    for (at, kind, call) in [(0, "Blend", "blend"), (1, "DepthStencilAlpha", "depth_stencil_alpha"), (2, "Rasterizer", "rasterizer")] {
        log.push(FILLED[at].to_string());
        log.push(format!("bind_{call}_state arg1 Some({})", object(kind, at.try_into().unwrap_or(0))));
        if at == 0 {
            log.push("bind_blend_state arg1 None".to_string());
        }
        log.push(format!("delete_{call}_state arg1 {}", object(kind, at.try_into().unwrap_or(0))));
    }
    log.push(FILLED[3].to_string());
    log.push(FILLED[4].to_string());
    log.push(format!("bind_sampler_states shader MesaShaderStage(3) start_slot 3 samplers [Some({}), Some({})]", object("Sampler", 3), object("Sampler", 4)));
    log.push(format!("delete_sampler_state arg1 {}", object("Sampler", 3)));
    log.push(format!("delete_sampler_state arg1 {}", object("Sampler", 4)));
    log.push(FILLED[5].to_string());
    log.push(format!("bind_vertex_elements_state arg1 Some({})", object("VertexElements", 5)));
    log.push(format!("delete_vertex_elements_state arg1 {}", object("VertexElements", 5)));
    log.push("destroy".to_string());
    log.push("destroy".to_string());
    log
}

/// One case on a new screen of the recording implementation: the case's status and the log.
fn run(which: i32) -> (i32, Vec<String>) {
    let recording = Recording::new();
    let log = Arc::clone(&recording.log);
    let address = wcontainers_gallium_direct::serve::serve(recording).expect("the screen is served");
    #[expect(unsafe_code, reason = "caller.c: wc_caller_case takes the address serve returned (serve.c's live struct pipe_screen) and a case number, and calls only that screen's pointers")]
    // SAFETY: the address is the screen serve made just now, not yet destroyed; the case destroys it last.
    let status = unsafe { wc_caller_case(address, which) };
    (status, log.calls())
}

/// Checks one case: its status 0, the methods that reached the implementation exactly those expected, and the first
/// case's arguments.
fn holds(which: i32) -> Result<(), String> {
    let (status, calls) = run(which);
    let names: Vec<&str> = calls.iter().map(|call| call.split(' ').next().unwrap_or_default()).collect();
    let case = EXPECTED.get(usize::try_from(which).unwrap_or(usize::MAX)).ok_or("no such case")?;
    let expected: Vec<&str> = SCREEN_MADE.iter().chain(case.iter()).copied().collect();
    if status != 0 {
        return Err(format!("case {which}: C saw status {status}; calls {names:?}"));
    }
    if names != *expected {
        return Err(format!("case {which}: calls {names:?}, expected {expected:?}"));
    }
    if calls.get(SCREEN_MADE.len()).map(String::as_str) != Some(CONTEXT_CREATED) {
        return Err(format!("case {which}: the context made as {:?}, expected {CONTEXT_CREATED}", calls.get(SCREEN_MADE.len())));
    }
    if which == 0 {
        // The whole log, line by line: every argument of every call as caller.c made it.
        let expected = first_case();
        if calls != expected {
            return Err(format!("case 0: the log differs from what caller.c made:\nreceived:\n{}\nexpected:\n{}", calls.join("\n"), expected.join("\n")));
        }
    }
    Ok(())
}

/// The capabilities serve.c writes into the screen are Mesa's own: a screen of the recording implementation (which
/// changes no capability) holds, byte for byte, the `caps` u_init_pipe_screen_caps writes for its stages'
/// capabilities.
#[test]
fn caps_are_mesas_defaults() {
    let address = wcontainers_gallium_direct::serve::serve(Recording::new()).expect("the screen is served");
    #[expect(unsafe_code, reason = "caller.c: wc_caller_caps takes the address serve returned (serve.c's live struct pipe_screen), reads its capabilities and destroys it")]
    // SAFETY: the address is the screen serve made just now, not yet destroyed; the call destroys it last.
    let status = unsafe { wc_caller_caps(address) };
    println!("CAPS status {status}");
    assert_eq!(status, 0, "the screen's caps differ from Mesa's u_init_pipe_screen_caps first at byte {}", status - 2);
}

/// Two screens served at once are kept apart: the second's destroy ends the second, and the first still serves a
/// context, which reaches the first's implementation.
#[test]
fn screens_are_kept_apart() {
    let (first, second) = (Recording::new(), Recording::new());
    let (first_log, second_log) = (Arc::clone(&first.log), Arc::clone(&second.log));
    let one = wcontainers_gallium_direct::serve::serve(first).expect("the first screen is served");
    let two = wcontainers_gallium_direct::serve::serve(second).expect("the second screen is served");
    #[expect(unsafe_code, reason = "caller.c: wc_caller_two_screens takes the two addresses serve returned (serve.c's live struct pipe_screen), destroys the second, makes and destroys a context of the first and destroys it")]
    // SAFETY: both addresses are screens serve made just now, not yet destroyed; the call destroys both.
    let status = unsafe { wc_caller_two_screens(one, two) };
    assert_eq!(status, 0, "the first screen still serves a context after the second's destroy");
    let names = |calls: Vec<String>| calls.into_iter().filter(|call| !call.contains("caps")).map(|call| call.split(' ').next().unwrap_or_default().to_string()).collect::<Vec<_>>();
    assert_eq!(names(second_log.calls()), ["destroy"], "the second screen's implementation: its destroy alone");
    assert_eq!(names(first_log.calls()), ["context_create", "destroy", "destroy"], "the first screen's implementation: the context and both destroys");
}

/// Every case, each alone in a process of its own (this test binary run again for one case).
#[test]
fn every_case_alone() {
    #[expect(unsafe_code, reason = "caller.c: wc_caller_cases returns a number, reading nothing")]
    // SAFETY: a plain call of our own C function.
    let cases = unsafe { wc_caller_cases() };
    assert_eq!(usize::try_from(cases).ok(), Some(EXPECTED.len()), "a log expected for every case of caller.c");
    let binary = std::env::current_exe().expect("the test binary");
    for which in 0..cases {
        let output = Command::new(&binary).args(["--exact", "one_case", "--nocapture", "--test-threads=1"]).env(CASE_VARIABLE, which.to_string()).output().expect("the case's process runs");
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        println!("SERVED CASE {which}: {}", if output.status.success() && text.contains(&format!("CASE {which} HELD")) { "held".to_string() } else { format!("FAILED {:?} {text}", output.status) });
        assert!(output.status.success() && text.contains(&format!("CASE {which} HELD")), "case {which}: {text}");
    }
}

/// The case WCONTAINERS_GALLIUM_CASE names; where it names none, every case in this process.
#[test]
fn one_case() {
    let cases: Vec<i32> = match std::env::var(CASE_VARIABLE) {
        Ok(which) => vec![which.parse().expect("a case number")],
        Err(_) => (0..i32::try_from(EXPECTED.len()).unwrap_or(0)).collect(),
    };
    for which in cases {
        match holds(which) {
            Ok(()) => println!("CASE {which} HELD"),
            Err(problem) => panic!("{problem}"),
        }
    }
}
