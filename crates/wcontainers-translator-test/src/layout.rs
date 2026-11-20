// Layer 1's memory layout and wire format, and the starting stack of a flat blob. The native reference
// runner includes this file too, so both sides always agree.

/// The exit status of a process that a signal ended is this plus the signal, as the shell and Docker give it.
pub const SIGNALLED: i32 = 128;

/// The reference runner's mode that runs blobs one after another, each in a child of its own, for requests on
/// stdin: a blob's file, load address, stack top, stack bytes and optional program headers and
/// count. Each answer is the status as a shell reports it (`i32`), then stdout and stderr, each as a `u32`
/// length and its bytes.
pub const SERVE_BLOBS: &str = "blobs";
/// What separates a request's arguments, and what ends a request.
pub const REQUEST_SEPARATOR: u8 = b' ';
pub const REQUEST_END: u8 = b'\n';

/// `AT_HWCAP` as the guest is told it; a test checks it against `wcontainers_abi::HWCAP`.
pub const BLOB_HWCAP: u64 = 0x183;

/// How long a program may run on Linux, in seconds, before the reference stops it.
pub const BLOB_SECONDS: u32 = 20;

/// The name a flat blob is started under, its `argv[0]`.
pub const BLOB_NAME: &[u8] = b"blob\0";

/// Bytes of one ELF64 program header.
pub const PROGRAM_HEADER_BYTES: u64 = 56;

/// What Linux puts at the top of a new program's stack, the way both runners give it to a flat blob:
/// argc, argv, an empty environment, the auxiliary vector, then the name and the 16 `AT_RANDOM` bytes.
/// `headers` is where the blob's program headers are in guest memory and how many there are, given as
/// `AT_PHDR` and `AT_PHNUM`, through which a position independent C program finds where it was placed.
/// Returns the stack pointer and the bytes to place from it up to `top`.
pub fn initial_stack(top: u64, headers: Option<(u64, u64)>) -> (u64, Vec<u8>) {
    const AT_NULL: u64 = 0;
    const AT_PHDR: u64 = 3;
    const AT_PHENT: u64 = 4;
    const AT_PHNUM: u64 = 5;
    const AT_PAGESZ: u64 = 6;
    const AT_HWCAP: u64 = 16;
    const AT_RANDOM: u64 = 25;
    const AT_HWCAP2: u64 = 26;
    const AT_EXECFN: u64 = 31;
    let random = top - 16;
    let name = (random - BLOB_NAME.len() as u64) & !15;
    let mut words = vec![1, name, 0, 0];
    if let Some((at, count)) = headers {
        words.extend([AT_PHDR, at, AT_PHENT, PROGRAM_HEADER_BYTES, AT_PHNUM, count]);
    }
    words.extend([
        AT_PAGESZ, 4096, AT_HWCAP, BLOB_HWCAP, AT_HWCAP2, 0, AT_RANDOM, random, AT_EXECFN, name,
        AT_NULL, 0,
    ]);
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

/// Where the instruction under test sits. Everything from `CODE - CODE_SPAN` to `CODE + CODE_SPAN` may be read
/// and run, and is zero apart from the instruction; zero is `udf #0`, so control stops wherever it lands.
pub const CODE: u64 = 0x5000_0000;
pub const CODE_SPAN: u64 = 128 << 20;

/// A case: the instruction word, the data seed, then x0 to x30, sp, the flags (at their `pstate` bits) and
/// the thread pointer, then v0 to v31, then FPSR and FPCR (4 bytes each), all little endian.
pub const REQUEST_BYTES: usize = 4 + 8 + 34 * 8 + 32 * 16 + 8;

/// A result: signal, `si_code`, `si_addr`, then x0 to x30, sp, pc, the flags and the thread pointer, then v0
/// to v31, then FPSR and FPCR, then the number of changed data bytes and each one as a 4 byte offset from
/// `DATA` and its new value.
pub const RESULT_HEADER_BYTES: usize = 4 + 4 + 8 + 35 * 8 + 32 * 16 + 8 + 4;

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
