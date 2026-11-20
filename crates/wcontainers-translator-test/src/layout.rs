// Layer 1's memory layout and wire format, and the starting stack of a flat blob. The native reference
// runner includes this file too, so both sides always agree.

/// Auxiliary vector keys a guest's own entries use.
pub const AT_HWCAP: u64 = 16;
pub const AT_HWCAP2: u64 = 26;

/// The name a flat blob is started under, its `argv[0]`.
pub const BLOB_NAME: &[u8] = b"blob\0";

/// Bytes of one ELF64 program header.
pub const PROGRAM_HEADER_BYTES: u64 = 56;

/// What Linux puts at the top of a new program's stack, the way both runners give it to a flat blob:
/// argc, argv, an empty environment, the auxiliary vector, then the name and the 16 `AT_RANDOM` bytes.
/// `headers` is where the blob's program headers are in guest memory and how many there are, given as
/// `AT_PHDR` and `AT_PHNUM`, through which a position independent C program finds where it was placed.
/// `auxiliary` holds the guest's own entries (what the CPU offers), given after `AT_PAGESZ`.
/// Returns the stack pointer and the bytes to place from it up to `top`.
pub fn initial_stack(
    top: u64,
    headers: Option<(u64, u64)>,
    auxiliary: &[(u64, u64)],
) -> (u64, Vec<u8>) {
    const AT_NULL: u64 = 0;
    const AT_PHDR: u64 = 3;
    const AT_PHENT: u64 = 4;
    const AT_PHNUM: u64 = 5;
    const AT_PAGESZ: u64 = 6;
    const AT_RANDOM: u64 = 25;
    const AT_EXECFN: u64 = 31;
    let random = top - 16;
    let name = (random - BLOB_NAME.len() as u64) & !15;
    let mut words = vec![1, name, 0, 0];
    if let Some((at, count)) = headers {
        words.extend([AT_PHDR, at, AT_PHENT, PROGRAM_HEADER_BYTES, AT_PHNUM, count]);
    }
    words.extend([AT_PAGESZ, 4096]);
    words.extend(auxiliary.iter().flat_map(|&(key, value)| [key, value]));
    words.extend([AT_RANDOM, random, AT_EXECFN, name, AT_NULL, 0]);
    let sp = (name - 8 * words.len() as u64) & !15;
    let mut bytes = vec![0u8; (top - sp) as usize];
    for (i, word) in words.iter().enumerate() {
        bytes[8 * i..8 * i + 8].copy_from_slice(&word.to_le_bytes());
    }
    let at = |address: u64| (address - sp) as usize;
    bytes[at(name)..at(name) + BLOB_NAME.len()].copy_from_slice(BLOB_NAME);
    for (i, byte) in bytes[at(random)..].iter_mut().enumerate() {
        *byte = 0xa5 ^ i as u8;
    }
    (sp, bytes)
}

/// Both areas sit above 1 GiB, out of reach of the randomised start of the reference's own heap.
///
/// Start of the data area: writable pages, then one read-only page. The pages either side allow nothing.
pub const DATA: u64 = 0x4400_0000;
pub const DATA_WRITABLE: u64 = 3 * 4096;
pub const DATA_READ_ONLY: u64 = 4096;
pub const DATA_BYTES: u64 = DATA_WRITABLE + DATA_READ_ONLY;

/// Where the instruction under test sits. The code area around it, `CODE - CODE_SPAN` to `CODE + CODE_SPAN`
/// of the guest's, may be read and run and holds the guest's `FILLER` apart from the instruction, so that
/// control stops wherever it lands.
pub const CODE: u64 = 0x5000_0000;

/// ARM64's code area: zero is `udf #0`; a branch reaches 128 MiB.
pub mod aarch64 {
    pub const FILLER: u8 = 0;
    pub const CODE_SPAN: u64 = 128 << 20;
    /// Where the reference runner's own memory is: its image, linked at the start (the image base in
    /// `reference/.cargo/config.toml`), and the heap Linux places up to 1 GiB after it, with room to grow;
    /// the runner checks it is there at start. No case may reach it.
    pub const RUNNER: core::ops::Range<u64> = 0x8000_0000..0xd000_0000;
}

/// x86-64's code area: `int3`, 32 KiB either side (a branch further out stops by the trap flag at its
/// target before anything there is fetched).
pub mod x86_64 {
    pub const FILLER: u8 = 0xcc;
    pub const CODE_SPAN: u64 = 32 << 10;
    /// As ARM64's.
    pub const RUNNER: core::ops::Range<u64> = 0x6000_0000..0xb000_0000;
    /// The state's field of ds' selector; es', fs' and gs' follow it.
    pub const SELECTORS: usize = 64;
}

/// The reference's wire: records both ways, each a little-endian u32 length and that many bytes. The runner
/// first sends its hello: the number of state fields, each field's name (a byte of length, then the name),
/// then the CPU's model name (a byte of length, then the name). A request starts with its kind.
pub const RECORD_CASE: u8 = 0;
pub const RECORD_BLOB: u8 = 1;
/// An ordinary static executable, its bytes after the kind; its result is a blob's, with no stop record.
pub const RECORD_PROGRAM: u8 = 2;
/// A result starts with its status: the request ran, the runner could not run it, the CPU's Linux refused to
/// set up the case's starting state, or the runner itself failed and ends (for the last three a message
/// follows).
pub const RESULT_RAN: u8 = 0;
pub const RESULT_FAILED: u8 = 1;
pub const RESULT_REFUSED: u8 = 2;
pub const RESULT_RUNNER_FAILED: u8 = 3;

/// Put `placed`'s bytes over the data from its offset, as far as the data reaches.
pub fn place(placed: &(u32, Vec<u8>), into: &mut [u8]) {
    let (at, bytes) = placed;
    for (to, &byte) in into.iter_mut().skip(*at as usize).zip(bytes) {
        *to = byte;
    }
}

/// The data area's starting contents for a seed.
pub fn fill(seed: u64, into: &mut [u8]) {
    let mut state = seed;
    for chunk in into.chunks_mut(8) {
        // splitmix64
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        chunk.copy_from_slice(&z.to_le_bytes()[..chunk.len()]);
    }
}
