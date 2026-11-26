//! C programs built as flat blobs, run through the translator and on real Linux of the guest's CPU.

use super::runner::{Fault, Machine, Stopped, Syscalls};
use super::{Cpu, part};
use crate::layout::initial_stack;
use crate::reference::{
    BlobRan, BlobRequest, DOCKER, REFERENCE_CONTAINER, RUNNER_EXECUTABLE, container_name,
    target_folder, with_reference,
};
use linux_raw_sys::general::{SIGALRM, SIGBUS, SIGFPE, SIGILL, SIGSEGV, SIGTRAP};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use wcontainers_abi::{Exit, PAGE_SIZE, Permissions};
use wcontainers_guest_cpu::Guest as _;
use wcontainers_guest_cpu::Personality as _;
use wcontainers_translator::{LookAhead, Omit, Optimise};

/// Where every blob is linked, and loaded unless it is moved.
pub const LOAD: u64 = 0x40_0000;
/// Where a position independent blob is loaded, away from where it is linked.
pub const MOVED: u64 = 0x2345_6000;
/// The top of the stack, above the reach of the reference's own randomised heap.
pub const STACK_TOP: u64 = 0x4810_0000;
/// The stack a blob gets unless it asks for more.
pub const STACK_BYTES: u64 = 1 << 20;

/// How the code is generated: for the fixed address it is linked and loaded at, or position independent
/// (`-fPIE`, a static position independent program that relocates itself at start), loaded away from where
/// it is linked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Fixed,
    Independent,
}

/// The C compiler's flags for code at a fixed address.
const FIXED_C_FLAGS: [&str; 3] = ["-static", "-fno-pic", "-no-pie"];
/// The C compiler's flags for position independent code.
const INDEPENDENT_C_FLAGS: [&str; 3] = ["-static-pie", "-fPIE", "-Wl,-z,norelro"];
/// rustc's flags for code at a fixed address.
const FIXED_RUST_FLAGS: [&str; 2] = ["-C", "relocation-model=static"];
/// rustc's flags for position independent code.
const INDEPENDENT_RUST_FLAGS: [&str; 4] =
    ["-C", "relocation-model=pie", "-C", "link-arg=-znorelro"];
/// What the name of a blob of position independent code ends with.
const INDEPENDENT_SUFFIX: &str = "-pic";

impl Position {
    /// The C compiler's flags for it.
    pub fn flags(self) -> &'static [&'static str] {
        match self {
            Position::Fixed => &FIXED_C_FLAGS,
            Position::Independent => &INDEPENDENT_C_FLAGS,
        }
    }

    /// rustc's flags for it.
    fn rust_flags(self) -> &'static [&'static str] {
        match self {
            Position::Fixed => &FIXED_RUST_FLAGS,
            Position::Independent => &INDEPENDENT_RUST_FLAGS,
        }
    }

    /// What a blob's name ends with.
    pub fn suffix(self) -> &'static str {
        match self {
            Position::Fixed => "",
            Position::Independent => INDEPENDENT_SUFFIX,
        }
    }

    /// Where a blob of it is loaded.
    pub fn load(self) -> u64 {
        match self {
            Position::Fixed => LOAD,
            Position::Independent => MOVED,
        }
    }
}

/// A flat blob and how much of its start is code.
pub struct Blob {
    /// The program and optimisation level, such as "hello-O2".
    pub name: String,
    pub bytes: Vec<u8>,
    pub code_bytes: u64,
    /// Where it is placed in guest memory.
    pub load: u64,
    /// Where its first byte is linked, when its code names its own addresses absolutely (see
    /// `wcontainers_translator::translate`).
    pub link_address: Option<u64>,
    /// The blob offset of its program's `main`, from the ELF's symbols.
    pub main: Option<u64>,
    /// Where in it its program's ELF program headers are, and how many: the runners tell the program.
    pub program_headers: Option<(u64, u64)>,
    /// How much stack it gets, below [`STACK_TOP`].
    pub stack_bytes: u64,
    /// The key (`Guest::offer_key`) of what its program is told of its processor, the default when `None`.
    pub offer: Option<u64>,
}

impl Blob {
    /// Whether the program's code reads the float status (ARM64's FPSR), so that its flags are kept exact
    /// from its start.
    pub fn reads_float_status(&self) -> bool {
        part::reads_float_status(&self.bytes[..self.code_bytes as usize])
    }

    /// Where the program headers are in guest memory, and how many.
    pub fn headers_in_memory(&self) -> Option<(u64, u64)> {
        self.program_headers
            .map(|(offset, count)| (self.load + offset, count))
    }
}

/// The link script that places a program as a flat blob, in the programs' source folder.
pub(crate) const FLAT_SCRIPT: &str = "flat.ld";

/// The folder of the test programs' sources beside this crate's manifest, also their folder under `target`.
const PROGRAMS: &str = "programs";

pub(crate) fn out_directory() -> PathBuf {
    target_folder(PROGRAMS)
}

/// A file name no other build or run, in this process or another, uses.
pub(crate) fn unique(stem: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{stem}.{}.{n}", std::process::id())
}

pub fn source_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PROGRAMS)
}

/// The folder of the guest's own program sources and headers, `programs/<guest>`.
fn guest_directory() -> PathBuf {
    source_directory().join(part::NAME)
}

/// The source of a program: `<name>.c` (or `.cpp`, `.rs`, `.S`) in the guest's own folder, else among the
/// shared ones.
pub fn source(name: &str) -> PathBuf {
    [guest_directory(), source_directory()]
        .iter()
        .flat_map(|directory| {
            ["c", "cpp", "rs", "S"].map(|extension| directory.join(format!("{name}.{extension}")))
        })
        .find(|source| source.exists())
        .unwrap_or_else(|| panic!("no program {name} for {}", part::NAME))
}

/// The longest one zig command may take. A build takes seconds; past this it is stuck, not slow.
const ZIG_LIMIT: Duration = Duration::from_secs(600);

/// Run zig with a cache of this checkout's own, so no zig process of another checkout holds its locks, and
/// fail with a message when it does not end.
pub(crate) fn zig(arguments: &[&str]) {
    use wait_timeout::ChildExt;
    let cache = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/zig-cache");
    let mut child = Command::new("zig")
        .args(arguments)
        .env("ZIG_GLOBAL_CACHE_DIR", &cache)
        .env("ZIG_LOCAL_CACHE_DIR", &cache)
        .spawn()
        .expect("run zig (from the Nix shell)");
    let Some(status) = child.wait_timeout(ZIG_LIMIT).expect("wait for zig") else {
        let _ = child.kill();
        let _ = child.wait();
        panic!("zig {arguments:?} did not end in {ZIG_LIMIT:?}");
    };
    assert!(status.success(), "zig {arguments:?} failed");
}

/// An ordinary static executable of a C program for zig's target `c_target` (such as "x86_64-linux-musl"),
/// built at -O1 with the C library; `name` names its files while it is built.
pub fn executable(c_target: &str, name: &str, source: &str) -> Vec<u8> {
    let own = unique(name);
    std::fs::create_dir_all(out_directory()).unwrap();
    let (c, elf) = (
        out_directory().join(format!("{own}.c")),
        out_directory().join(format!("{own}.elf")),
    );
    std::fs::write(&c, source).unwrap();
    let (c_path, elf_path) = (c.display().to_string(), elf.display().to_string());
    zig(&[
        "cc", "-target", c_target, "-O1", "-static", "-o", &elf_path, &c_path,
    ]);
    let bytes = std::fs::read(&elf).unwrap();
    std::fs::remove_file(&c).unwrap();
    std::fs::remove_file(&elf).unwrap();
    bytes
}

/// Check that `run` runs ordinary executables for `c_target` as Linux starts a program: hello world's output
/// (its name a blob's) and status, a write through a null pointer ended by SIGSEGV, and a loop that does not
/// end ended by the time limit's SIGALRM.
pub fn check_ordinary_programs(c_target: &str, mut run: impl FnMut(&[u8]) -> Ran) {
    let hello = std::fs::read_to_string(source_directory().join("hello.c")).unwrap();
    for (name, source, stdout, status) in [
        (
            "hello",
            hello.as_str(),
            &b"hello, world\n1 blob beef ! -1234567890123\n"[..],
            3,
        ),
        (
            "null",
            "int main(void) { *(volatile int *)0 = 1; return 0; }",
            b"",
            SIGNALLED + SIGSEGV as i32,
        ),
        (
            "forever",
            "int main(void) { volatile int n = 0; for (;;) n++; }",
            b"",
            SIGNALLED + SIGALRM as i32,
        ),
    ] {
        let ran = run(&executable(c_target, name, source));
        assert_eq!(
            (ran.stdout.as_slice(), ran.status),
            (stdout, status),
            "{name} on the reference, stderr {:?}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }
}

/// What a program is built against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Library {
    /// Nothing: the program brings its own `_start` and system calls, and uses only general registers.
    None,
    /// The C library (musl, as zig builds it), started by its own `_start`.
    C,
}

/// The linker option that links only what the program reaches, as from an archive: zig otherwise keeps C
/// library code nothing calls, such as `fmal`, which reads FPSR and so would keep every program's flags exact.
pub(crate) const LINK_ONLY_WHAT_IS_REACHED: &str = "-Wl,--gc-sections";

/// zig's flag that names the target to build C for, which `part::C_TARGET` follows.
pub const ZIG_TARGET_FLAG: &str = "-target";
/// zig's commands for C and for C++.
pub const C_DRIVER: &str = "cc";
const CPP_DRIVER: &str = "c++";
/// The compiler's flag that leaves out every sanitizer (zig turns some on by default).
pub(crate) const NO_SANITIZERS: &str = "-fno-sanitize=all";
/// The linker option, given through the compiler, that leaves out the build id.
pub(crate) const NO_BUILD_ID: &str = "-Wl,--build-id=none";
/// The linker option, given through the compiler, that names the link script, joined to its path.
pub(crate) const LINK_SCRIPT: &str = "-Wl,-T,";
/// The compiler's flag that names the file it writes.
pub const OUTPUT_FLAG: &str = "-o";
/// The compiler's flag that adds an include folder, joined to its path.
pub(crate) const INCLUDE_FLAG: &str = "-I";
/// The compiler's flags for a program without a C library: no hosted environment, no start files or libraries.
const FREESTANDING_FLAGS: [&str; 2] = ["-ffreestanding", "-nostdlib"];
/// The compiler's flags that keep such a program from needing the library: no stack protector, and signed
/// arithmetic that wraps.
const NO_LIBRARY_NEEDED_FLAGS: [&str; 2] = ["-fno-stack-protector", "-fwrapv"];
/// What joins a blob's name to each of its definitions.
const NAME_SEPARATOR: char = '-';
/// What joins a definition's name to its value.
const DEFINITION_ASSIGNMENT: char = '=';
/// The extension of a linked program.
pub(crate) const ELF_EXTENSION: &str = "elf";

/// What a test program's source is written in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    C,
    Cpp,
    Rust,
    Assembly,
}

/// The extension of each language's source, in the order they are looked for.
const SOURCES: [(&str, Language); 4] = [
    ("c", Language::C),
    ("cpp", Language::Cpp),
    ("rs", Language::Rust),
    ("S", Language::Assembly),
];

/// The language of a source found by [`source`], from its extension.
fn language(source: &Path) -> Language {
    let extension = source.extension().and_then(|e| e.to_str());
    SOURCES
        .iter()
        .find(|(known, _)| Some(*known) == extension)
        .map(|(_, language)| *language)
        .expect("a source of a known language")
}

/// Build a program (see [`source`]: C, C++ with its library, Rust with its standard library, or assembly) at
/// an optimisation level, such as "-O2", into a flat blob. The guest's folder is on the include path.
pub fn build(name: &str, optimisation: &str, library: Library) -> Blob {
    build_with(name, optimisation, library, &[], Position::Fixed)
}

/// [`build`] with preprocessor definitions, such as "SCALE=1", for the code's position.
pub fn build_with(
    name: &str,
    optimisation: &str,
    library: Library,
    defines: &[&str],
    position: Position,
) -> Blob {
    let mut stem = format!("{name}{optimisation}{}", position.suffix());
    for define in defines {
        stem.push(NAME_SEPARATOR);
        stem.push_str(&define.replace(DEFINITION_ASSIGNMENT, ""));
    }
    let defines: Vec<String> = defines.iter().map(|d| format!("-D{d}")).collect();
    build_flagged(
        (name, stem),
        optimisation,
        library,
        &defines,
        position,
        None,
    )
}

/// [`build`] for the processor named by a compiler flag, such as x86's [`super::part::V3_CPU`], whose
/// instructions the compiler may then use anywhere, in a program without a C library too.
pub fn build_for(
    name: &str,
    optimisation: &str,
    library: Library,
    position: Position,
    processor: &str,
) -> Blob {
    let stem = format!("{name}{optimisation}{}{processor}", position.suffix());
    build_flagged(
        (name, stem),
        optimisation,
        library,
        &[],
        position,
        Some(processor),
    )
}

/// Build the program `name` into the blob named `stem`, with these flags of preprocessor definitions, for the
/// processor named by a compiler flag or, when `None`, the guest's baseline (a program without a C library for
/// its general registers only).
fn build_flagged(
    (name, stem): (&str, String),
    optimisation: &str,
    library: Library,
    defines: &[String],
    position: Position,
    processor: Option<&str>,
) -> Blob {
    let out = out_directory();
    std::fs::create_dir_all(&out).unwrap();
    // Tests build the same program at once, so each build works under its own names.
    let own = unique(&stem);
    let elf = out.join(&own).with_added_extension(ELF_EXTENSION);
    let source = source(name);
    let language = language(&source);
    let script = source_directory().join(FLAT_SCRIPT);
    let text = |p: &Path| p.to_str().unwrap().to_string();
    if language == Language::Rust {
        assert!(
            processor.is_none(),
            "a Rust program is built for the baseline"
        );
        rust(&source, &elf, &script, optimisation, position);
        return flatten(&elf, &own, stem);
    }
    let script = [LINK_SCRIPT, &text(&script)].concat();
    let include = [INCLUDE_FLAG, &text(&guest_directory())].concat();
    let driver = if language == Language::Cpp {
        CPP_DRIVER
    } else {
        C_DRIVER
    };
    let mut arguments = vec![driver, ZIG_TARGET_FLAG, part::C_TARGET, optimisation];
    arguments.extend(position.flags());
    arguments.extend([NO_SANITIZERS, &script, &include, NO_BUILD_ID]);
    if library == Library::C {
        arguments.push(LINK_ONLY_WHAT_IS_REACHED);
    }
    if library == Library::None {
        arguments.extend(FREESTANDING_FLAGS);
        if processor.is_none() {
            arguments.extend(part::GENERAL_ONLY);
        }
        arguments.extend(NO_LIBRARY_NEEDED_FLAGS);
    }
    arguments.extend(processor);
    arguments.extend(defines.iter().map(String::as_str));
    let (elf_text, source_text) = (text(&elf), text(&source));
    arguments.extend([OUTPUT_FLAG, &elf_text, &source_text]);
    zig(&arguments);
    flatten(&elf, &own, stem)
}

/// The function a Rust program starts at, which the C library calls as `main`.
const RUST_ENTRY: &str = "entry";
/// The crate name a Rust program is built under, as the library its program links.
const RUST_CRATE: &str = "program";
/// The root of the program that links that library and holds nothing else.
const RUST_ROOT: &str = "#![no_main]\nextern crate program;\n";
/// The flags rustc takes for the library.
const RUST_LIBRARY_FLAGS: &[&str] = &["--crate-type", "rlib", "--crate-name", RUST_CRATE];
/// The flags rustc takes for the root.
const RUST_ROOT_FLAGS: &[&str] = &["--crate-type", "bin", "--crate-name", "start"];
/// The flag rustc takes for a library a crate links, followed by the crate's name, `=` and its file.
const RUST_EXTERN_FLAG: &str = "--extern";
/// The flag rustc takes for a code generation option.
const RUST_CODEGEN_FLAG: &str = "-C";
/// The code generation option that defines a linker symbol, followed by its name, `=` and its value.
const RUST_DEFINE_SYMBOL: &str = "link-arg=--defsym=";
/// What the file name of a library rustc takes starts with.
const RUST_LIBRARY_PREFIX: &str = "lib";
/// What the file name of a library rustc takes ends with.
const RUST_LIBRARY_SUFFIX: &str = ".rlib";
/// What a symbol in Rust's own mangling starts with (https://doc.rust-lang.org/rustc/symbol-mangling/v0.html).
const RUST_MANGLED_PREFIX: &str = "_R";

/// Build a Rust program with its standard library, statically, linked with `script`.
///
/// The source is built as a library, whose public `entry` keeps a global symbol, and a root that links it is
/// linked with that symbol as `main`, so the program needs no unmangled name of its own.
fn rust(source: &Path, elf: &Path, script: &Path, optimisation: &str, position: Position) {
    let level = optimisation.trim_start_matches("-O");
    let rustc = || {
        let mut command = Command::new("rustc");
        command
            .args(["--edition", "2024", "--target", part::REFERENCE.rust_target])
            .args([RUST_CODEGEN_FLAG, &format!("opt-level={level}")])
            .args([RUST_CODEGEN_FLAG, "panic=unwind"])
            .args(position.rust_flags());
        command
    };
    let run = |command: &mut Command| {
        let status = command.status().expect("run rustc (from the Nix shell)");
        assert!(status.success(), "rustc {} failed", source.display());
    };
    let stem = elf.file_stem().unwrap().to_str().unwrap();
    let library = elf.with_file_name([RUST_LIBRARY_PREFIX, stem, RUST_LIBRARY_SUFFIX].concat());
    run(rustc()
        .args(RUST_LIBRARY_FLAGS)
        .arg(OUTPUT_FLAG)
        .arg(&library)
        .arg(source));
    let entry = library_symbol(&library, RUST_CRATE, RUST_ENTRY);
    let extension = SOURCES
        .iter()
        .find(|(_, language)| *language == Language::Rust)
        .unwrap()
        .0;
    let root = elf.with_extension(extension);
    std::fs::write(&root, RUST_ROOT).unwrap();
    let assignment = DEFINITION_ASSIGNMENT.to_string();
    let library_text = library.to_str().unwrap();
    run(rustc()
        .args(RUST_ROOT_FLAGS)
        .arg(RUST_EXTERN_FLAG)
        .arg([RUST_CRATE, &assignment, library_text].concat())
        .args([
            RUST_CODEGEN_FLAG,
            "linker-flavor=ld.lld",
            RUST_CODEGEN_FLAG,
            "linker=rust-lld",
        ])
        .args([
            RUST_CODEGEN_FLAG,
            "link-self-contained=yes",
            RUST_CODEGEN_FLAG,
            "link-arg=--build-id=none",
        ])
        .arg(RUST_CODEGEN_FLAG)
        .arg([RUST_DEFINE_SYMBOL, MAIN, &assignment, &entry].concat())
        .arg(RUST_CODEGEN_FLAG)
        .arg(format!("link-arg=-T{}", script.display()))
        .arg(OUTPUT_FLAG)
        .arg(elf)
        .arg(&root));
}

/// Finds the global symbol of the function `name` at the root of the Rust library `crate_name` at `library`.
fn library_symbol(library: &Path, crate_name: &str, name: &str) -> String {
    use object::read::archive::ArchiveFile;
    use object::{Object, ObjectSymbol};
    // In Rust's own mangling the path ends in the crate's name and then the function's, each after its length.
    let ending = [
        crate_name.len().to_string(),
        crate_name.to_string(),
        name.len().to_string(),
        name.to_string(),
    ]
    .concat();
    let bytes = std::fs::read(library).unwrap();
    let archive = ArchiveFile::parse(&*bytes).expect("an rlib");
    let mut found = Vec::new();
    for member in archive.members() {
        let member = member.unwrap();
        // The library's metadata is a member too, and is not an object file.
        let Ok(file) = object::File::parse(member.data(&*bytes).unwrap()) else {
            continue;
        };
        found.extend(
            file.symbols()
                .filter(|s| s.is_global() && s.is_definition())
                .filter_map(|s| s.name().ok().map(String::from))
                .filter(|s| s.starts_with(RUST_MANGLED_PREFIX) && s.ends_with(&ending)),
        );
    }
    assert_eq!(
        found.len(),
        1,
        "one {name} in {}: {found:?}",
        library.display()
    );
    found.pop().unwrap()
}

/// The symbol of a program's main function.
const MAIN: &str = "main";
/// The section of a program's instructions.
const TEXT_SECTION: &str = ".text";

/// The flat blob of a linked program, `own` being the unique name its build works under, loaded at `MOVED`
/// if the linker made it position independent. The ELF file is kept, under the blob's name, for looking at.
pub(crate) fn flatten(elf: &Path, own: &str, name: String) -> Blob {
    use object::read::elf::FileHeader;
    use object::{Object, ObjectSection, ObjectSymbol};
    let out = out_directory();
    let text = |p: &Path| p.to_str().unwrap().to_string();
    let built = out.join(format!("{own}.bin"));
    let code = out.join(format!("{own}.text"));
    zig(&["objcopy", "-O", "binary", &text(elf), &text(&built)]);
    zig(&[
        "objcopy",
        "-O",
        "binary",
        "--only-section=.text",
        &text(elf),
        &text(&code),
    ]);
    // The program headers go after the image, as the image does not hold them.
    let file = std::fs::read(elf).unwrap();
    let parsed =
        object::read::elf::ElfFile64::<object::Endianness>::parse(&*file).expect("an ELF file");
    let headers = parsed.elf_program_headers();
    let position = if parsed.elf_header().e_type(parsed.endian()) == object::elf::ET_DYN {
        Position::Independent
    } else {
        Position::Fixed
    };
    // The image starts with its instructions.
    let text = parsed
        .section_by_name(TEXT_SECTION)
        .expect("a text section")
        .address();
    let main = parsed
        .symbols()
        .find(|s| s.name() == Ok(MAIN))
        .map(|s| s.address() - text);
    let mut bytes = std::fs::read(&built).unwrap();
    bytes.resize(bytes.len().next_multiple_of(align_of_val(headers)), 0);
    let at = bytes.len() as u64;
    bytes.extend_from_slice(object::pod::bytes_of_slice(headers));
    let blob = Blob {
        bytes,
        code_bytes: std::fs::metadata(&code).unwrap().len(),
        load: position.load(),
        link_address: (position == Position::Fixed).then_some(text),
        main,
        program_headers: Some((at, headers.len() as u64)),
        name,
        stack_bytes: STACK_BYTES,
        offer: None,
    };
    std::fs::remove_file(&built).unwrap();
    std::fs::remove_file(&code).unwrap();
    std::fs::rename(elf, elf_path(&blob.name)).unwrap();
    blob
}

/// Where a built blob's linked program is kept, by the blob's name.
pub fn elf_path(name: &str) -> PathBuf {
    out_directory()
        .join(name)
        .with_added_extension(ELF_EXTENSION)
}

/// The offsets of the instructions in a range of code, decoding forward from its start; where nothing
/// decodes, the next slot.
pub fn boundaries(code: &[u8]) -> Vec<u64> {
    let mut at = 0;
    let mut all = Vec::new();
    while at < code.len() {
        all.push(at as u64);
        at +=
            Cpu::decode(&code[at..]).map_or(Cpu::ALIGN as usize, |(_, length)| usize::from(length));
    }
    all
}

/// What a program did: its output and exit status.
#[derive(Debug, PartialEq, Eq)]
pub struct Ran {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,
}

pub use crate::layout::SIGNALLED;

/// The page after the first page of a blob: the data of the hand-made programs whose code fits one page.
pub const PAGE_AFTER_LOAD: u64 = LOAD + PAGE_SIZE;

/// Where a flat blob and its stack go in guest memory, the same for every runner.
pub struct Placement {
    /// Size of guest memory: everything up to the top of the stack.
    pub memory_bytes: u64,
    /// Page aligned ranges and what they allow: the blob, then the stack.
    pub regions: Vec<(Range<u64>, Permissions)>,
    /// The stack pointer to start with, and the bytes to place from it up to the top of the stack.
    pub sp: u64,
    pub stack: Vec<u8>,
}

pub fn placement(blob: &Blob) -> Placement {
    let end = blob.load + (blob.bytes.len() as u64).next_multiple_of(PAGE_SIZE);
    let (sp, stack) = initial_stack(STACK_TOP, blob.headers_in_memory(), part::AUXILIARY);
    Placement {
        memory_bytes: STACK_TOP,
        regions: vec![
            (blob.load..end, Permissions::all()),
            (
                STACK_TOP - blob.stack_bytes..STACK_TOP,
                Permissions::READ | Permissions::WRITE,
            ),
        ],
        sp,
        stack,
    }
}

/// Run a blob through the translator in wasmtime, starting at its first byte, translated for speed.
pub fn translated(
    blob: &Blob,
    look_ahead: LookAhead,
) -> (
    Ran,
    Machine,
    <Cpu as wcontainers_guest_cpu::Personality>::Block,
) {
    translated_for(blob, (look_ahead, Optimise::Speed))
}

/// [`translated`] for speed or for size.
pub fn translated_for(
    blob: &Blob,
    how: (LookAhead, Optimise),
) -> (
    Ran,
    Machine,
    <Cpu as wcontainers_guest_cpu::Personality>::Block,
) {
    let (ran, machine, registers, _) = translated_omitting(blob, how, Omit::default());
    (ran, machine, registers)
}

/// [`translated`] without some checks, also giving how long the guest ran (translating and compiling not
/// included).
pub fn translated_omitting(
    blob: &Blob,
    how: (LookAhead, Optimise),
    omit: Omit,
) -> (
    Ran,
    Machine,
    <Cpu as wcontainers_guest_cpu::Personality>::Block,
    Duration,
) {
    run_blob(blob, how, omit, false)
}

/// [`translated_for`] with the thread stopping at every stop check it reaches and carrying on each time.
pub fn translated_stopping_everywhere(
    blob: &Blob,
    how: (LookAhead, Optimise),
) -> (
    Ran,
    Machine,
    <Cpu as wcontainers_guest_cpu::Personality>::Block,
) {
    let (ran, machine, registers, _) = run_blob(blob, how, Omit::default(), true);
    (ran, machine, registers)
}

fn run_blob(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    stop_everywhere: bool,
) -> (
    Ran,
    Machine,
    <Cpu as wcontainers_guest_cpu::Personality>::Block,
    Duration,
) {
    let mut machine = ready_for(blob, look_ahead, optimise, omit);
    if stop_everywhere {
        machine.stop_at_every_check();
    }
    let started = Instant::now();
    let (stopped, registers) = machine.run();
    let ran_for = started.elapsed();
    let ran = ran(blob, &machine, stopped, &registers);
    (ran, machine, registers, ran_for)
}

/// A machine with a blob placed, translated and ready to run from its first byte.
pub fn ready(blob: &Blob, look_ahead: LookAhead, omit: Omit) -> Machine {
    ready_for(blob, look_ahead, Optimise::Speed, omit)
}

/// [`ready`], translated for `optimise`.
pub fn ready_for(blob: &Blob, look_ahead: LookAhead, optimise: Optimise, omit: Omit) -> Machine {
    ready_on(blob, (look_ahead, optimise), omit, super::runner::probe())
}

/// [`ready`], translated for a host with these facts instead of the probe's.
pub fn ready_on(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    host: <Cpu as wcontainers_guest_cpu::Guest>::Host,
) -> Machine {
    ready_with(blob, host, |machine| {
        machine.optimise_for(optimise);
        machine.load_omitting(
            &blob.bytes,
            0..blob.code_bytes,
            (blob.load, blob.link_address),
            &[0],
            look_ahead,
            omit,
        );
        // The reference stays at none throughout.
        if look_ahead == LookAhead::None {
            machine.translate_at_run_time(|| LookAhead::None);
        }
    })
}

/// A machine with the blob's memory and permissions, its code put in place by `load`, and the stack and
/// registers ready to run from its first byte.
pub fn ready_with(
    blob: &Blob,
    host: <Cpu as wcontainers_guest_cpu::Guest>::Host,
    load: impl FnOnce(&mut Machine),
) -> Machine {
    // The table covers the whole blob, every page of it that may run code, so code the program writes runs.
    let slots = (blob.bytes.len() as u64).div_ceil(Cpu::ALIGN) as u32;
    let place = placement(blob);
    let mut machine = Machine::new(place.memory_bytes, blob.load, slots, Syscalls::StandIn);
    machine.set_host(host);
    if let Some(key) = blob.offer {
        machine.set_offer(Cpu::offer_from_key(key).expect("an offer's key"));
    }
    for (range, permissions) in place.regions {
        machine.set_permissions(range, permissions);
    }
    load(&mut machine);
    machine.write(place.sp, &place.stack);
    machine.set_registers(&Cpu::initial_block(blob.load, place.sp));
    if blob.reads_float_status() {
        machine.track_float_flags();
    }
    machine
}

/// What a program did, from why its thread stopped. Panics if it stopped without exiting or faulting.
pub fn ran(
    blob: &Blob,
    machine: &Machine,
    stopped: Stopped,
    registers: &<Cpu as wcontainers_guest_cpu::Personality>::Block,
) -> Ran {
    let finished = |status| Ran {
        stdout: machine.stdout().to_vec(),
        stderr: machine.stderr().to_vec(),
        status,
    };
    let signal = match stopped {
        // Linux reports the low 8 bits of the status.
        Stopped::Finished(status) => return finished(i32::from(status as u8)),
        Stopped::Exit(Exit::MemoryFault { .. }) => SIGSEGV,
        Stopped::Exit(Exit::AlignmentFault { .. }) => SIGBUS,
        Stopped::Exit(Exit::IllegalInstruction) => SIGILL,
        Stopped::Exit(Exit::Breakpoint { .. }) | Stopped::Fault(Fault::SingleStep) => SIGTRAP,
        Stopped::Fault(Fault::DivideError | Fault::FloatException(_)) => SIGFPE,
        Stopped::Fault(Fault::GeneralProtection | Fault::Overflow) => SIGSEGV,
        Stopped::Fault(Fault::Stack) => SIGBUS,
        other => {
            let pc = u64::from_le_bytes(
                zerocopy::IntoBytes::as_bytes(registers)[Cpu::PC_OFFSET..][..size_of::<u64>()]
                    .try_into()
                    .unwrap(),
            );
            let at = (pc.wrapping_sub(blob.load) as usize).min(blob.bytes.len());
            let code = &blob.bytes[at..(at + Cpu::MAX_LENGTH as usize).min(blob.bytes.len())];
            panic!(
                "the program stopped without exiting: {other:?} at {pc:#x}, bytes {code:02x?} ({})",
                part::disassemble(code)
            )
        }
    };
    // A process killed by a signal is reported as a shell does, `SIGNALLED` plus the signal's number.
    finished(SIGNALLED + signal as i32)
}

/// Every byte the guest may reach: the blob's pages, then the stack.
pub fn reachable_memory(machine: &mut Machine, blob: &Blob) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (range, _) in placement(blob).regions {
        let mut part = vec![0; (range.end - range.start) as usize];
        machine.read(range.start, &mut part);
        bytes.extend(part);
    }
    bytes
}

/// This machine's load averages, recorded beside every time.
pub use crate::jobs::load;

/// The shortest of `runs` native runs of a blob on the reference, in seconds, each timed by the reference
/// from making a child process to its end (so the reference's start-up is not counted).
pub fn native_seconds(blob: &Blob, runs: u32) -> f64 {
    on_reference(blob, runs.max(1)).seconds
}

/// Run a Linux container for `purpose` to its end, with the further `docker run` arguments `add` gives.
/// Fails when Docker itself failed (it made no container, or the container did not start), so that
/// Docker's own failure is never taken for the program's ending.
pub fn run_container(purpose: &str, add: impl FnOnce(&mut Command)) -> std::process::Output {
    let name = container_name(purpose);
    let directory = out_directory().join("containers");
    std::fs::create_dir_all(&directory).unwrap();
    // Docker writes the container's id here once it has made the container.
    let made = directory.join(&name);
    let mut command = Command::new(DOCKER);
    command
        .args(REFERENCE_CONTAINER)
        .arg(&name)
        .arg("--cidfile")
        .arg(&made);
    add(&mut command);
    let output = command.output().expect("run docker");
    let was_made = std::fs::remove_file(&made).is_ok();
    let status = output.status.code().expect("an exit status");
    let did_not_start =
        DOCKER_FAILED.contains(&status) && output.stderr.starts_with(DOCKER_FAILED_SAYS);
    assert!(
        was_made && !did_not_start,
        "Docker failed to run {name} (status {status}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Docker's own exit statuses for a container it could not run (its daemon's error, a command that cannot
/// be invoked, a command not found), and what it writes on stderr with them.
const DOCKER_FAILED: std::ops::RangeInclusive<i32> = 125..=127;
const DOCKER_FAILED_SAYS: &[u8] = b"docker: Error response from daemon: ";

/// Run a blob natively on the reference.
pub fn native(blob: &Blob) -> Ran {
    let ran = on_reference(blob, 0);
    Ran {
        stdout: ran.stdout,
        stderr: ran.stderr,
        status: ran.status,
    }
}

/// A blob run natively on the reference, with the signal, `si_code`, pc and general registers of the signal
/// that ended it, if one did (`BlobRan::stop`).
pub fn native_with_stop(blob: &Blob) -> (Ran, Option<(u32, u32, Vec<u64>)>) {
    let ran = on_reference(blob, 0);
    let stop = ran.stop;
    let ran = Ran {
        stdout: ran.stdout,
        stderr: ran.stderr,
        status: ran.status,
    };
    (ran, stop)
}

fn on_reference(blob: &Blob, runs: u32) -> BlobRan {
    let request = BlobRequest {
        bytes: &blob.bytes,
        load: blob.load,
        stack_top: STACK_TOP,
        stack_bytes: blob.stack_bytes,
        headers: blob.headers_in_memory(),
        auxiliary: part::AUXILIARY,
        stdin: &[],
        runs,
    };
    runner_did_not_fail(with_reference(&part::REFERENCE, |reference| {
        reference.blob(&request)
    }))
}

/// Run an ordinary static executable on the reference, as Linux starts a program.
pub fn native_program(executable: &[u8]) -> Ran {
    let ran = runner_did_not_fail(with_reference(&part::REFERENCE, |reference| {
        reference.program(executable)
    }));
    Ran {
        stdout: ran.stdout,
        stderr: ran.stderr,
        status: ran.status,
    }
}

/// The result of a blob or a program. The runner's own failure, in its child or before, is no result of
/// the guest's: it fails the case, never compares.
fn runner_did_not_fail(ran: Result<BlobRan, String>) -> BlobRan {
    let ran = ran.unwrap_or_else(|e| panic!("the reference runner failed: {e}"));
    let says = format!("{RUNNER_EXECUTABLE}{RUNNER_FAILED_AFTER_NAME}");
    assert!(
        !(ran.status == RUNNER_FAILED
            && ran.stderr.windows(says.len()).any(|w| w == says.as_bytes())),
        "the reference runner failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    ran
}

/// How the reference runner ends when it fails itself, after writing its name (`RUNNER_EXECUTABLE`) on stderr.
const RUNNER_FAILED: i32 = 101;
const RUNNER_FAILED_AFTER_NAME: &str = ": ";
