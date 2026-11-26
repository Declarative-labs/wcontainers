//! The monitor's translations kept in files between runs (docs/native.md section 3.2): a second translator reads
//! what the first wrote, moved to another page, and a file that does not read as one is refused and made again.

use std::path::PathBuf;

use wcontainers_abi::PAGE_SIZE;
use wcontainers_arm64::Arm64;
use wcontainers_executor_hardware_virtualization::translate::{Translator, reason};
use wcontainers_executor_hardware_virtualization_inside::link::Link;
use wcontainers_executor_hardware_virtualization_inside::translation::{self, Piece, Translate};
use wcontainers_guest_cpu::Native;
use wcontainers_translator::{LookAhead, translate_native};
use zerocopy::FromZeros;

/// A link page on the heap, as the inside writes one.
fn link() -> Box<Link> {
    Link::new_box_zeroed().expect("memory for a link")
}

/// The piece `translator` answers for `bytes` at `at`.
fn ask(translator: &mut Translator, bytes: &[u8], at: u64) -> Option<Piece> {
    let mut link = link();
    link.ask(
        &Translate {
            address: at,
            exact: 1,
        },
        bytes,
    );
    translator.answer(&mut link);
    translation::piece(&link)
}

/// The piece translating `bytes` at `at` gives.
fn made(bytes: &[u8], at: u64, look_ahead: LookAhead) -> Option<Piece> {
    translate_native::<Arm64>(bytes, at, look_ahead, true).map(|code| {
        let t = code.translation();
        Piece {
            code: t.bytes,
            spans: t.spans,
            exits: t.exits,
            sites: t.sites,
            tables: t.tables,
        }
    })
}

#[test]
fn kept_files_are_read_by_the_next_translator() {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("kept-translations");
    let _ = std::fs::remove_dir_all(&directory);
    let look_ahead = LookAhead::Bounded(u32::MAX);
    let words = [
        0x9400_0010u32, // bl . + 64
        0x9000_0001,    // adrp x1, .
        0x5800_0040,    // ldr x0, . + 8
        0x17ff_fffd,    // b . - 12
    ];
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let movable = Arm64::movable();
    let (first, second) = (movable.start, movable.start + PAGE_SIZE);

    let mut writer = Translator::new(look_ahead, Some(directory.clone()));
    assert_eq!(
        ask(&mut writer, &bytes, first),
        made(&bytes, first, look_ahead)
    );
    assert_eq!((writer.reused, writer.read), (0, 0));
    let files: Vec<_> = std::fs::read_dir(&directory)
        .expect("the directory was made")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(files.len(), 1, "one file a translation");

    let mut reader = Translator::new(look_ahead, Some(directory.clone()));
    assert_eq!(
        ask(&mut reader, &bytes, second),
        made(&bytes, second, look_ahead)
    );
    assert_eq!((reader.reused, reader.read), (1, 1));
    assert!(reader.slow.is_empty());

    std::fs::write(&files[0], b"not a translation").expect("the file is written");
    let mut refused = Translator::new(look_ahead, Some(directory.clone()));
    assert_eq!(
        ask(&mut refused, &bytes, second),
        made(&bytes, second, look_ahead)
    );
    assert_eq!((refused.reused, refused.read), (0, 0));
    assert_eq!(refused.slow.get(reason::FILE_REFUSED), Some(&1));

    let mut again = Translator::new(look_ahead, Some(directory.clone()));
    assert_eq!(
        ask(&mut again, &bytes, first),
        made(&bytes, first, look_ahead)
    );
    assert_eq!(
        (again.reused, again.read),
        (1, 1),
        "the refused file was made again"
    );
    std::fs::remove_dir_all(&directory).expect("the directory is removed");
}
