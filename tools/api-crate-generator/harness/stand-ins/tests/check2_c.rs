//! check-2's C games against the front through the serving glue and the fake through both glues, each alone under a
//! time limit: what each reports is printed, and each must return.
#![forbid(unsafe_code)]

use std::sync::mpsc::channel;
use std::time::Duration;

use wcontainers_direct3d_11_direct_harness::front::Front;
use wcontainers_direct3d_11_direct_harness::{Check2, Entry};

fn within_time(entry: Entry, game: Check2) -> Option<(i32, String)> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || drop(sender.send(entry.check2(game))));
    receiver.recv_timeout(Duration::from_secs(10)).ok()
}

fn both(game: Check2) -> [Option<(i32, String)>; 2] {
    let outcomes = [within_time(Entry::served(Front), game), within_time(Entry::pass_through(), game)];
    println!("CHECK2 {game:?}: front {:?}; pass-through {:?}", outcomes[0], outcomes[1]);
    outcomes
}

macro_rules! game {
    ($($name:ident: $game:expr;)*) => {$(
        #[test]
        fn $name() {
            let outcomes = both($game);
            assert!(outcomes.iter().all(Option::is_some), "a game hung for 10 s");
        }
    )*};
}

game! {
    get_device_twice: Check2::GetDeviceTwice;
    held_texture_after_draw: Check2::HeldTextureAfterDraw;
    get_type_then_get_desc: Check2::GetTypeThenGetDesc { exact: true };
    const_initial: Check2::ConstInitial;
    fill_by_depth_pitch: Check2::FillByDepthPitch;
    huge_texture: Check2::HugeTexture;
    callbacks: Check2::Callbacks;
    write_after_release: Check2::WriteAfterRelease;
    map_race: Check2::MapRace { threads: 8, iterations: 2_000 };
}
