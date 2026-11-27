//! The spike's game, through raw COM calls only, on three targets: the fake directly, the front through the serving
//! glue, and the fake through both glues (pass-through). Run with one test thread: the fake counts its live objects.
#![forbid(unsafe_code)]

use wcontainers_direct3d_11_direct_harness::front::Front;
use wcontainers_direct3d_11_direct_harness::{Entry, live_fake_objects, observed};

#[test]
fn the_game_runs_on_the_fake_directly() {
    let before = live_fake_objects();
    Entry::fake().play().unwrap();
    Entry::fake().play_textures().unwrap();
    assert_eq!(live_fake_objects(), before, "every fake object destroyed");
}

#[test]
fn the_game_runs_on_the_front_through_the_serving_glue() {
    Entry::served(Front).play().unwrap();
    Entry::served(Front).play_textures().unwrap();
}

#[test]
fn the_game_runs_on_the_fake_through_both_glues() {
    let before = live_fake_objects();
    Entry::pass_through().play().unwrap();
    Entry::pass_through().play_textures().unwrap();
    let seen = observed();
    assert_eq!(seen.mapped_now, 0, "every mapping unmapped");
    assert!(seen.protected_contexts > 0, "the calling glue turned on the context's multithread protection");
    assert_eq!(live_fake_objects(), before, "every fake object destroyed through the chain");
}

#[test]
fn a_hostile_program_finds_nothing_in_our_glue() {
    Entry::served(Front).play_hostile(Entry::fake()).unwrap();
    Entry::pass_through().play_hostile(Entry::fake()).unwrap();
}

#[test]
fn calls_from_many_threads_at_once_keep_counts_right() {
    const THREADS: u32 = 8;
    const ITERATIONS: u32 = 20_000;
    let before = live_fake_objects();
    assert_eq!(Entry::fake().hammer(THREADS, ITERATIONS), 0, "the fake");
    assert_eq!(Entry::served(Front).hammer(THREADS, ITERATIONS), 0, "the front");
    assert_eq!(Entry::pass_through().hammer(THREADS, ITERATIONS), 0, "the chain");
    assert_eq!(live_fake_objects(), before);
}
