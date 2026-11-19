//! Translation through the link (docs/native.md section 3.2): the inside sends the guest code from an address to
//! the end of its page, and the monitor answers with the piece's host code, then its position map and its
//! direct exits, in the link's bytes; and the words the inside fills to link pieces.

use alloc::vec::Vec;

use wcontainers_arm64::mirror::SLOT_MOVES;
use wcontainers_guest_cpu::position::{DirectExit, NativeReason, Span};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::link::{self, Link};

crate::requests! {
    first: link::Request::END;
    /// Translate the guest code in the link's bytes, which starts at guest `address`; `exact` is 1 when the
    /// process asks for exact replay, so the count's adds are written.
    Translate { address, exact } -> Translated;
    /// The words the inside fills to fill entry slots and link exits.
    Linking {} -> LinkWords;
}

/// A word with one immediate field: the word with the field clear, and the field's bits.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Field {
    pub clear: u32,
    pub mask: u32,
}

impl Field {
    /// The word with the low bits of `value` in the field (a signed value as its two's complement).
    pub fn with(&self, value: u64) -> u32 {
        self.clear | ((value as u32) << self.mask.trailing_zeros()) & self.mask
    }

    /// Whether the signed `value` fits the field.
    pub fn fits(&self, value: i64) -> bool {
        let half = 1i64 << (self.mask.count_ones() - 1);
        (-half..half).contains(&value)
    }
}

/// The answer to [`Linking`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct LinkWords {
    /// A slot's moves of its piece's address into the scratch register, from the lowest part up.
    pub moves: [Field; SLOT_MOVES],
    /// The slot's jump through the scratch register.
    pub jump: u32,
    /// A direct branch, its field in instructions.
    pub branch: Field,
    /// The first word of an empty slot, which raises an exception as a fresh page's zeros do.
    pub empty: u32,
}

/// The answer to [`Translate`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Translated {
    /// Bytes of host code at the start of the link's bytes; none when no instruction starts at the address.
    pub code: u64,
    /// Bytes of the position map and the direct exits after them.
    pub map: u64,
    /// The sites of each slow path in the piece, in [`NativeReason::ALL`]'s order.
    pub sites: [u64; NativeReason::ALL.len()],
}

/// One translated piece as the link carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub code: Vec<u8>,
    pub spans: Vec<Span>,
    pub exits: Vec<DirectExit>,
    pub sites: [u64; NativeReason::ALL.len()],
}

/// A piece too large for the link's bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooLarge;

/// Writes the answer to a translate request: the piece, or none when no instruction starts there.
pub fn answer(link: &mut Link, piece: Option<&Piece>) -> Result<(), TooLarge> {
    let Some(piece) = piece else {
        link.put(&Translated::default());
        link.head.length = 0;
        return Ok(());
    };
    let code = piece.code.len();
    let (host, rest) = link.bytes.split_at_mut_checked(code).ok_or(TooLarge)?;
    host.copy_from_slice(&piece.code);
    let map = postcard::to_slice(&(&piece.spans, &piece.exits), rest)
        .map_err(|_| TooLarge)?
        .len();
    link.head.length = (code + map) as u64;
    link.put(&Translated {
        code: code as u64,
        map: map as u64,
        sites: piece.sites,
    });
    Ok(())
}

/// Reads the answer to a translate request: none when no instruction starts at the address. An answer that
/// does not read as one is the monitor's fault, never the guest's, and panics.
pub fn piece(link: &Link) -> Option<Piece> {
    let answer: Translated = link.get();
    let code = core::num::NonZeroU64::new(answer.code)?;
    let code = usize::try_from(code.get())
        .unwrap_or_else(|_| panic!("the monitor's translate answer is broken: code length"));
    let map = usize::try_from(answer.map)
        .unwrap_or_else(|_| panic!("the monitor's translate answer is broken: map length"));
    let bytes = code
        .checked_add(map)
        .and_then(|end| link.bytes.get(..end))
        .unwrap_or_else(|| {
            panic!("the monitor's translate answer is broken: longer than the link")
        });
    let (spans, exits) = postcard::from_bytes(&bytes[code..])
        .unwrap_or_else(|_| panic!("the monitor's translate answer is broken: position map"));
    Some(Piece {
        code: bytes[..code].to_vec(),
        spans,
        exits,
        sites: answer.sites,
    })
}
