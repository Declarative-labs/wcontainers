//! The monitor's answer to the inside's translation requests: the native output's piece of the guest code the
//! link carries, kept in memory and in files of a directory, to be loaded again for another address by its
//! address places (docs/native.md section 3.2), and the words the inside fills to link pieces.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_arm64::Arm64;
use wcontainers_arm64::carry::link_words;
use wcontainers_guest_cpu::{Native, Translation};
use wcontainers_inside::link::Link;
use wcontainers_inside::translation::{
    self, Field, LinkWords, Piece, Request, TooLarge, Translate,
};
use wcontainers_translator::{LookAhead, translate_native};

/// The slow paths of the directory of kept translations, as counted and warned.
pub mod reason {
    /// A file of the directory could not be read whole or did not read as a kept translation.
    pub const FILE_REFUSED: &str = "native translation in the cache's directory refused";
    /// A translation could not be written to the directory.
    pub const FILE_NOT_KEPT: &str = "native translation not kept in the cache's directory";
}

/// The first bytes of a kept translation's file.
const FILE_MAGIC: &[u8; 8] = b"wcnative";

/// What a file being written has after its name, then the writing process's number, until it is renamed whole
/// into place.
const PARTIAL: &str = ".partial-";

/// What a translation is kept under: the guest code it is made from, where in its page that starts, whether the
/// count is written, and the page itself where the code's shape depends on its address.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    code: Vec<u8>,
    offset: u64,
    exact: bool,
    page: Option<u64>,
}

/// A kept translation (none when no instruction starts there) and the page it was made for.
#[derive(Clone, Serialize, Deserialize)]
struct Kept {
    translation: Option<Translation>,
    page: u64,
}

/// The build of the code that translates: a hash of the running program, which holds the translator and its
/// guests.
fn build() -> &'static [u8; blake3::OUT_LEN] {
    static BUILD: OnceLock<[u8; blake3::OUT_LEN]> = OnceLock::new();
    BUILD.get_or_init(|| {
        let program = std::env::current_exe()
            .and_then(std::fs::read)
            .expect("the running program reads");
        *blake3::hash(&program).as_bytes()
    })
}

/// The monitor's translations at one look-ahead.
pub struct Translator {
    look_ahead: LookAhead,
    kept: HashMap<Key, Kept>,
    /// Where translations are kept between runs, when anywhere.
    directory: Option<PathBuf>,
    /// How many answers came from a kept translation.
    pub reused: u64,
    /// How many of those were read from the directory.
    pub read: u64,
    /// Each slow path of the directory taken, by reason, with how often.
    pub slow: BTreeMap<&'static str, u64>,
}

impl Translator {
    /// A translator at `look_ahead`, keeping translations in memory, and in `directory` between runs when given
    /// one, which only translators write.
    pub fn new(look_ahead: LookAhead, directory: Option<PathBuf>) -> Translator {
        Translator {
            look_ahead,
            kept: HashMap::new(),
            directory,
            reused: 0,
            read: 0,
            slow: BTreeMap::new(),
        }
    }

    /// What it translates at.
    pub fn look_ahead(&self) -> LookAhead {
        self.look_ahead
    }

    /// Answers the translation request in `link`.
    pub fn answer(&mut self, link: &mut Link) {
        match Request::from_word(link.head.request).expect("a translation request") {
            Request::Translate => self.translate(link),
            Request::Linking => {
                let words = link_words();
                let field = |(clear, mask)| Field { clear, mask };
                link.put(&LinkWords {
                    moves: words.moves.map(field),
                    jump: words.jump,
                    branch: field(words.branch),
                    empty: words.empty,
                    last_target_first: field(words.last_target_first),
                    last_target_test: words.last_target_test,
                    last_target_branch: words.last_target_branch,
                });
            }
        }
    }

    /// Counts and warns a slow path of the directory.
    fn slow(&mut self, reason: &'static str, file: &Path) {
        let times = self.slow.entry(reason).or_default();
        *times += 1;
        eprintln!(
            "wcontainers warning: {reason}, {}, {times} so far",
            file.display()
        );
    }

    /// The file `key`'s translation is kept in.
    fn file(&self, key: &Key) -> Option<PathBuf> {
        let directory = self.directory.as_ref()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(build());
        hasher.update(format!("{:?} {:?} {:?}", self.look_ahead, key.exact, key.page).as_bytes());
        hasher.update(&key.offset.to_le_bytes());
        hasher.update(&key.code);
        Some(directory.join(hasher.finalize().to_hex().as_str()))
    }

    /// The translation kept in `file`: none when it is not there or does not read as one.
    fn load(&mut self, file: &Path) -> Option<Kept> {
        let bytes = match std::fs::read(file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(_) => {
                self.slow(reason::FILE_REFUSED, file);
                return None;
            }
        };
        let kept = bytes
            .strip_prefix(FILE_MAGIC)
            .and_then(|rest| postcard::from_bytes(rest).ok());
        if kept.is_none() {
            self.slow(reason::FILE_REFUSED, file);
        }
        kept
    }

    /// Keeps a translation in `file`, written whole or not at all.
    fn store(&mut self, file: &Path, kept: &Kept) {
        let mut bytes = FILE_MAGIC.to_vec();
        bytes.extend(postcard::to_allocvec(kept).expect("a translation serialises"));
        let mut partial = file.as_os_str().to_owned();
        partial.push(PARTIAL);
        partial.push(std::process::id().to_string());
        let partial = PathBuf::from(partial);
        let written = std::fs::create_dir_all(file.parent().expect("a directory"))
            .and_then(|()| std::fs::write(&partial, &bytes))
            .and_then(|()| std::fs::rename(&partial, file));
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
            self.slow(reason::FILE_NOT_KEPT, file);
        }
    }

    /// Answers a translate request with the piece at the look-ahead, cut shorter where its host code would not
    /// fit the link, from a kept translation when there is one.
    fn translate(&mut self, link: &mut Link) {
        let request: Translate = link.get();
        let code = link.data().to_vec();
        let page = request.address - request.address % PAGE_SIZE;
        let key = Key {
            code,
            offset: request.address % PAGE_SIZE,
            exact: request.exact != 0,
            page: (!Arm64::movable().contains(&page)).then_some(page),
        };
        let file = self.file(&key);
        if !self.kept.contains_key(&key)
            && let Some(kept) = file.as_deref().and_then(|file| self.load(file))
        {
            self.read += 1;
            self.kept.insert(key.clone(), kept);
        }
        if let Some(kept) = self.kept.get(&key) {
            self.reused += 1;
            let mut translation = kept.translation.clone();
            if let Some(translation) = &mut translation {
                translation.moved::<Arm64>(page.wrapping_sub(kept.page));
            }
            translation::answer(link, translation.map(piece).as_ref())
                .unwrap_or_else(|TooLarge| panic!("a kept piece fits the link"));
            return;
        }
        let mut look_ahead = self.look_ahead;
        loop {
            let translation =
                translate_native::<Arm64>(&key.code, request.address, look_ahead, key.exact)
                    .map(|code| code.translation());
            let instructions = translation.as_ref().map_or(0, |t| {
                let mut guests: Vec<u64> = t.spans.iter().map(|span| span.guest).collect();
                guests.sort_unstable();
                guests.dedup();
                guests.len()
            });
            match translation::answer(link, translation.clone().map(piece).as_ref()) {
                Ok(()) => {
                    let kept = Kept { translation, page };
                    if let Some(file) = &file {
                        self.store(file, &kept);
                    }
                    self.kept.insert(key, kept);
                    return;
                }
                Err(TooLarge) if instructions > 1 => {
                    let half =
                        u32::try_from(instructions / 2).expect("a piece's instructions fit u32");
                    look_ahead = LookAhead::Bounded(half);
                }
                Err(TooLarge) => panic!("one instruction's piece does not fit the link"),
            }
        }
    }
}

/// A translation as the link carries it.
fn piece(translation: Translation) -> Piece {
    Piece {
        code: translation.bytes,
        spans: translation.spans,
        exits: translation.exits,
        sites: translation.sites,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_arm64::{Arm64, encode};
    use wcontainers_guest_cpu::{Guest, Native};
    use wcontainers_inside::link::Link;
    use wcontainers_inside::translation::{self, Translate};
    use wcontainers_translator::{LookAhead, translate_native};

    use super::{Translator, piece};

    /// The knobs of the moving test: how many cases, and the seed.
    const MOVED_CASES: &str = "WCONTAINERS_MOVED_CASES";
    const MOVED_SEED: &str = "WCONTAINERS_MOVED_SEED";
    const DEFAULT_MOVED_CASES: u64 = 2000;
    const DEFAULT_MOVED_SEED: u64 = 0x6d6f_7665_6421;

    fn knob(name: &str, default: u64) -> u64 {
        std::env::var(name).map_or(default, |v| {
            let v = v.trim();
            v.strip_prefix("0x")
                .map_or_else(|| v.parse(), |hex| u64::from_str_radix(hex, 16))
                .unwrap_or_else(|_| panic!("{name} is a number"))
        })
    }

    /// A word of guest code: one of the forms that name an address (`adr`, `adrp`, `b`, `bl`, `b.cond`, a literal
    /// load, `blr`, `ret`) with random fields, or any word.
    fn word(rng: &mut fastrand::Rng) -> u32 {
        let forms = [
            (0x1000_0001u32, 0x60ff_ffffu32), // adr x1 and up
            (0x9000_0000, 0x60ff_ffff),       // adrp
            (0x1400_0000, 0x03ff_ffff),       // b
            (0x9400_0000, 0x03ff_ffff),       // bl
            (0x5400_0000, 0x00ff_ffef),       // b.cond
            (0x5800_0001, 0x00ff_ffff),       // ldr x1 and up, literal
            (0xd63f_0000, 0x0000_03e0),       // blr
            (0xd65f_03c0, 0),                 // ret
            (0xd100_0400, 0x0000_001f),       // sub x, x0, #1
            (0xb5ff_ffe0, 0x00ff_ffff),       // cbnz x
            (0, u32::MAX),                    // any word
        ];
        let (fixed, free) = forms[rng.usize(..forms.len())];
        fixed | rng.u32(..) & free
    }

    /// Translated code moved from one movable page to another is what translating it there gives: its bytes,
    /// position map, exits and places.
    #[test]
    fn moved_code_is_what_translating_there_gives() {
        let cases = knob(MOVED_CASES, DEFAULT_MOVED_CASES);
        let seed = knob(MOVED_SEED, DEFAULT_MOVED_SEED);
        let mut rng = fastrand::Rng::with_seed(seed);
        let movable = Arm64::movable();
        let pages = (movable.end - movable.start) / PAGE_SIZE;
        let page = |rng: &mut fastrand::Rng| movable.start + rng.u64(..pages) * PAGE_SIZE;
        let step = Arm64::MAX_LENGTH;
        let mut held = 0;
        for case in 0..cases {
            let offset = rng.u64(..PAGE_SIZE / step) * step;
            let words = rng.usize(1..=usize::try_from((PAGE_SIZE - offset) / step).expect("fits"));
            let bytes: Vec<u8> = (0..words)
                .flat_map(|_| word(&mut rng).to_le_bytes())
                .collect();
            let look_ahead = [
                LookAhead::None,
                LookAhead::Bounded(rng.u32(1..64)),
                LookAhead::Full,
            ][rng.usize(..3)];
            let exact = rng.bool();
            let (from, to) = (page(&mut rng) + offset, page(&mut rng) + offset);
            let made = translate_native::<Arm64>(&bytes, from, look_ahead, exact)
                .map(|code| code.translation());
            let there = translate_native::<Arm64>(&bytes, to, look_ahead, exact)
                .map(|code| code.translation());
            let moved = made.map(|mut t| {
                t.moved::<Arm64>(to.wrapping_sub(from));
                t
            });
            held += moved.as_ref().map_or(0, |t| t.places.len());
            assert_eq!(
                moved, there,
                "case {case} of seed {seed:#x}: {from:#x} to {to:#x}, {look_ahead:?}, exact {exact}"
            );
        }
        println!("moving: {cases} cases of seed {seed:#x}, {held} address places, 0 failed");
    }

    /// A piece asked for again at another movable page comes from the kept translation, moved there.
    #[test]
    fn a_kept_piece_is_loaded_at_another_page() {
        let movable = Arm64::movable();
        let words = [
            0x9400_0010u32, // bl . + 64
            0x9000_0001,    // adrp x1, .
            0x5800_0040,    // ldr x0, . + 8
            0x17ff_fffd,    // b . - 12
        ];
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut translator = Translator::new(LookAhead::Bounded(u32::MAX), None);
        for (k, at) in [
            movable.start,
            movable.start + PAGE_SIZE,
            movable.end - PAGE_SIZE,
        ]
        .into_iter()
        .enumerate()
        {
            let mut link = link();
            link.ask(
                &Translate {
                    address: at,
                    exact: 1,
                },
                &bytes,
            );
            translator.answer(&mut link);
            assert_eq!(translator.reused, k as u64);
            let made = translate_native::<Arm64>(&bytes, at, LookAhead::Bounded(u32::MAX), true)
                .map(|code| piece(code.translation()));
            assert_eq!(translation::piece(&link), made, "at {at:#x}");
        }
    }

    /// A link page on the heap, as the inside writes one.
    fn link() -> Box<Link> {
        // SAFETY: a link is plain words and bytes, for which all zeros is valid.
        unsafe { Box::new_zeroed().assume_init() }
    }

    /// Every word of a few routes comes back from the link as the emitter made it.
    #[test]
    fn a_piece_comes_back_whole() {
        let at = 0x40_1000;
        let words = [
            0xd400_0001u32, // svc #0
            0xf940_0020,    // ldr x0, [x1]
            0xa9bf_7bfd,    // stp x29, x30, [sp, #-16]!
            0x9400_0000,    // bl .
            0xd61f_0360,    // br x27
            0xd53b_4400,    // mrs x0, fpcr
        ];
        for word in words {
            for exact in [false, true] {
                let mut link = link();
                link.ask(
                    &Translate {
                        address: at,
                        exact: u64::from(exact),
                    },
                    &word.to_le_bytes(),
                );
                Translator::new(LookAhead::None, None).answer(&mut link);
                let got = translation::piece(&link).expect("a piece");
                let made =
                    translate_native::<Arm64>(&word.to_le_bytes(), at, LookAhead::None, exact)
                        .expect("a piece");
                assert_eq!(got.code, made.bytes, "{word:#010x}");
                assert_eq!(got.spans, made.spans, "{word:#010x}");
                assert_eq!(got.sites, made.sites, "{word:#010x}");
                assert_eq!(got.exits, made.exits, "{word:#010x}");
            }
        }
        let mut link = link();
        link.ask(
            &Translate {
                address: at,
                exact: 0,
            },
            &[0; Arm64::MAX_LENGTH as usize - 1],
        );
        Translator::new(LookAhead::None, None).answer(&mut link);
        assert_eq!(
            translation::piece(&link),
            None,
            "no instruction in three bytes"
        );
    }

    /// The guest addresses a piece holds code for, and the targets of its exits.
    fn shape(words: &[u32], at: u64) -> (BTreeSet<u64>, BTreeSet<u64>) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let made = translate_native::<Arm64>(&bytes, at, LookAhead::Bounded(u32::MAX), true)
            .expect("a piece");
        (
            made.spans.iter().map(|span| span.guest).collect(),
            made.exits.iter().map(|exit| exit.target).collect(),
        )
    }

    /// At bounded look-ahead a loop is held whole, its branch back going straight to its head, and the code after it
    /// is reached through an exit; a call's next instruction is left to the return.
    #[test]
    fn pieces_hold_loops_and_leave_calls() {
        let at = 0x40_1000;
        let step = Arm64::MAX_LENGTH;
        let looping = [
            0xd100_0400u32, // sub x0, x0, #1
            0xb5ff_ffe0,    // cbnz x0, . - 4
            0xd65f_03c0,    // ret
        ];
        let (held, exits) = shape(&looping, at);
        assert_eq!(held, BTreeSet::from([at, at + step]));
        assert_eq!(exits, BTreeSet::from([at + 2 * step]));
        let page = i64::try_from(PAGE_SIZE).expect("a page fits i64");
        let call = encode::branch(true, page).expect("a call to the next page");
        let (held, exits) = shape(&[call, looping[2]], at);
        assert_eq!(held, BTreeSet::from([at]));
        assert_eq!(exits, BTreeSet::from([at + PAGE_SIZE]));
    }

    /// The code after a branch back before the piece's start is held when it comes back into the piece, and
    /// starts a piece of its own when it only leaves.
    #[test]
    fn pieces_hold_the_rest_of_a_turn() {
        let at = 0x40_1000;
        let step = Arm64::MAX_LENGTH;
        let turning = [
            0xf100_05efu32, // subs x15, x15, #1
            0x54ff_fea0,    // b.eq . - 0x2c
            0x385f_f1d0,    // ldurb w16, [x14, #-1]
            0x6b10_01bf,    // cmp w13, w16
            0x54ff_ff81,    // b.ne . - 16
            0xd65f_03c0,    // ret
        ];
        let (held, exits) = shape(&turning, at);
        assert_eq!(held, (0..5).map(|k| at + k * step).collect());
        assert_eq!(exits, BTreeSet::from([at + step - 0x2c, at + 5 * step]));
        let leaving = [
            0x5100_0a73u32, // sub w19, w19, #2
            0x7100_067f,    // cmp w19, #1
            0x54ff_ff88,    // b.hi . - 16
            0xd65f_03c0,    // ret
        ];
        let (held, exits) = shape(&leaving, at);
        assert_eq!(held, (0..3).map(|k| at + k * step).collect());
        assert_eq!(exits, BTreeSet::from([at - 2 * step, at + 3 * step]));
    }

    /// An answer whose position map does not decode fails loudly, never as the guest's illegal instruction.
    #[test]
    #[should_panic(expected = "the monitor's translate answer is broken: position map")]
    fn a_broken_answer_panics() {
        let mut link = link();
        link.ask(
            &Translate {
                address: 0x40_1000,
                exact: 0,
            },
            &0xd400_0001u32.to_le_bytes(),
        );
        Translator::new(LookAhead::None, None).answer(&mut link);
        let answer: translation::Translated = link.get();
        let code = usize::try_from(answer.code).expect("fits");
        link.bytes[code..].fill(u8::MAX);
        translation::piece(&link);
    }
}
