//! Real ARM64 Linux binaries, for measuring the translator at scale: each file's code and every start address
//! the file offers.

use object::elf;
use object::read::elf::{Dyn, ElfFile64, FileHeader, ProgramHeader, Rela, SectionHeader, Sym};
use object::{Endianness, Object, ObjectSection, ObjectSymbol, SectionKind, SymbolKind};
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::PathBuf;
use std::process::Command;

/// A binary to measure: its name, the image it comes from, the Alpine packages to add first, and its path
/// there.
pub struct Source {
    pub name: &'static str,
    image: &'static str,
    packages: &'static str,
    path: &'static str,
}

/// The binaries M5 measures: busybox, the C library, ripgrep and git from Alpine, Node from its own image on
/// Alpine (with its libraries built in, the largest), and runtimes of other kinds: Python (its library holds
/// the interpreter), Bun, a Java runtime and a Go program (Go's own tool).
pub const SOURCES: [Source; 9] = [
    Source::alpine("busybox", "", "/bin/busybox"),
    Source::alpine("musl", "", "/lib/ld-musl-aarch64.so.1"),
    Source::alpine("rg", "ripgrep", "/usr/bin/rg"),
    Source::alpine("git", "git", "/usr/bin/git"),
    Source {
        name: "node",
        image: "node:24-alpine3.22",
        packages: "",
        path: "/usr/local/bin/node",
    },
    Source {
        name: "python",
        image: "python:3.13-alpine3.22",
        packages: "",
        path: "/usr/local/lib/libpython3.13.so.1.0",
    },
    Source {
        name: "bun",
        image: "oven/bun:1.3-alpine",
        packages: "",
        path: "/usr/local/bin/bun",
    },
    Source {
        name: "java",
        image: "eclipse-temurin:21-jre-alpine",
        packages: "",
        path: "/opt/java/openjdk/lib/server/libjvm.so",
    },
    Source {
        name: "go",
        image: "golang:1.25-alpine3.22",
        packages: "",
        path: "/usr/local/go/bin/go",
    },
];

impl Source {
    const fn alpine(name: &'static str, packages: &'static str, path: &'static str) -> Self {
        Source {
            name,
            image: "alpine:3.22",
            packages,
            path,
        }
    }

    /// The file, taken from its image once (with Docker) and kept under `target/binaries`.
    pub fn file(&self) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/binaries");
        let file = directory.join(self.name);
        if file.exists() {
            return file;
        }
        std::fs::create_dir_all(&directory).unwrap();
        let container = crate::reference::container_name(&format!("binary-{}", self.name));
        let mut create = Command::new("docker");
        if self.packages.is_empty() {
            create.args(["create", "--name", &container, self.image, "true"]);
        } else {
            let add = format!("apk add --no-cache {}", self.packages);
            create.args(["run", "--name", &container, self.image, "sh", "-c", &add]);
        }
        let made = create.output().expect("run docker");
        assert!(
            made.status.success(),
            "docker could not make {}: {}",
            self.name,
            String::from_utf8_lossy(&made.stderr)
        );
        let partial = directory.join(format!("{}.partial", self.name));
        let copied = Command::new("docker")
            .args(["cp", "-L", &format!("{container}:{}", self.path)])
            .arg(&partial)
            .status()
            .expect("run docker");
        Command::new("docker")
            .args(["rm", "-f", &container])
            .output()
            .expect("run docker");
        assert!(copied.success(), "docker could not copy {}", self.name);
        std::fs::rename(&partial, &file).unwrap();
        file
    }
}

/// One binary as the translator sees it.
pub struct Binary {
    pub name: String,
    /// The loaded image, from the file's lowest address: every loadable segment in place, zero between and
    /// after. Image offsets are addresses less that lowest one.
    pub image: Vec<u8>,
    /// Where the code is in the image: from the start of the first section of instructions to the end of the
    /// last (the whole runnable segment if the file has no sections).
    pub code: Range<u64>,
    /// Start addresses (image offsets in the code) by where the file says they are.
    pub starts: Starts,
}

/// Start addresses by the means that found them. One address may be found by several.
#[derive(Default)]
pub struct Starts {
    /// The entry point and the initialisation and finishing functions the dynamic table names.
    pub entry: BTreeSet<u64>,
    /// Function symbols, of both symbol tables.
    pub symbols: BTreeSet<u64>,
    /// The functions the unwind tables describe.
    pub unwind: BTreeSet<u64>,
    /// The initialisation and finishing lists.
    pub init_fini: BTreeSet<u64>,
    /// Addresses in data that point into the code: what relocations put there, or what is there already.
    pub data: BTreeSet<u64>,
}

impl Starts {
    /// Every start, once each, in order.
    pub fn all(&self) -> Vec<u64> {
        let mut all: BTreeSet<u64> = BTreeSet::new();
        for set in self.by_means() {
            all.extend(set.1);
        }
        all.into_iter().collect()
    }

    pub fn by_means(&self) -> [(&'static str, &BTreeSet<u64>); 5] {
        [
            ("entry", &self.entry),
            ("symbols", &self.symbols),
            ("unwind tables", &self.unwind),
            ("init and fini lists", &self.init_fini),
            ("addresses in data", &self.data),
        ]
    }
}

type File<'a> = ElfFile64<'a, Endianness>;

impl Binary {
    pub fn read(name: &str, bytes: &[u8]) -> Binary {
        let file = File::parse(bytes).expect("an ARM64 ELF file");
        let endian = file.endian();
        let loads: Vec<_> = file
            .elf_program_headers()
            .iter()
            .filter(|h| h.p_type(endian) == elf::PT_LOAD)
            .collect();
        let base = loads.iter().map(|h| h.p_vaddr(endian)).min().unwrap();
        let end = loads
            .iter()
            .map(|h| h.p_vaddr(endian) + h.p_memsz(endian))
            .max()
            .unwrap();
        let mut image = vec![0u8; (end - base) as usize];
        for h in &loads {
            let data = h.data(endian, bytes).expect("a segment's bytes");
            let at = (h.p_vaddr(endian) - base) as usize;
            image[at..at + data.len()].copy_from_slice(data);
        }
        let instructions: Vec<Range<u64>> = file
            .sections()
            .filter(|s| s.kind() == SectionKind::Text && s.size() > 0)
            .map(|s| s.address() - base..s.address() + s.size() - base)
            .collect();
        let code = if instructions.is_empty() {
            let runnable = loads
                .iter()
                .find(|h| h.p_flags(endian).0 & elf::PF_X.0 != 0)
                .expect("a runnable segment");
            let start = runnable.p_vaddr(endian) - base;
            start..start + runnable.p_filesz(endian)
        } else {
            let start = instructions.iter().map(|r| r.start).min().unwrap();
            start..instructions.iter().map(|r| r.end).max().unwrap()
        };
        let code = code.start.next_multiple_of(4)..code.end / 4 * 4;
        let mut binary = Binary {
            name: name.to_string(),
            image,
            code,
            starts: Starts::default(),
        };
        binary.find_starts(&file, base);
        binary
    }

    /// An image offset for a guest address, if it is a whole instruction in the code.
    fn in_code(&self, base: u64, address: u64) -> Option<u64> {
        let offset = address.checked_sub(base)?;
        (self.code.contains(&offset) && offset.is_multiple_of(4)).then_some(offset)
    }

    fn word(&self, base: u64, address: u64) -> Option<u64> {
        let at = usize::try_from(address.checked_sub(base)?).ok()?;
        let bytes = self.image.get(at..at + 8)?;
        Some(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn find_starts(&mut self, file: &File<'_>, base: u64) {
        let endian = file.endian();
        let mut starts = Starts::default();
        let code = |address| self.in_code(base, address);

        starts.entry.extend(code(file.entry()));
        let dynamic: Vec<(elf::DynamicTag, u64)> = file
            .elf_program_headers()
            .iter()
            .find_map(|h| h.dynamic(endian, file.data()).ok().flatten())
            .map(|table| {
                table
                    .iter()
                    .map(|d| (d.d_tag(endian), d.d_val(endian)))
                    .collect()
            })
            .unwrap_or_default();
        for &(tag, value) in &dynamic {
            if tag == elf::DT_INIT || tag == elf::DT_FINI {
                starts.entry.extend(code(value));
            }
        }

        for symbol in file.symbols().chain(file.dynamic_symbols()) {
            if symbol.kind() == SymbolKind::Text && symbol.is_definition() {
                starts.symbols.extend(code(symbol.address()));
            }
        }

        // What each relocated place holds once the program is loaded at its own addresses: the word a
        // relative relocation in RELA form gives, a defined symbol's address, or what a RELR one leaves there.
        let symbols: HashMap<u32, u64> = file
            .elf_dynamic_symbol_table()
            .symbols()
            .iter()
            .enumerate()
            .filter(|(_, s)| s.st_shndx(endian) != elf::SHN_UNDEF)
            .map(|(i, s)| (i as u32, s.st_value(endian)))
            .collect();
        let mut relocated: HashMap<u64, u64> = HashMap::new();
        let sections = file.elf_section_table();
        for section in sections.iter() {
            match section.sh_type(endian) {
                elf::SHT_RELA => {
                    let Ok(Some((relas, _))) = section.rela(endian, file.data()) else {
                        continue;
                    };
                    for rela in relas {
                        let place = rela.r_offset(endian);
                        let addend = rela.r_addend(endian) as u64;
                        let value = match rela.r_type(endian, false) {
                            elf::R_AARCH64_RELATIVE => Some(addend),
                            elf::R_AARCH64_ABS64
                            | elf::R_AARCH64_GLOB_DAT
                            | elf::R_AARCH64_JUMP_SLOT => symbols
                                .get(&rela.r_sym(endian, false))
                                .map(|v| v.wrapping_add(addend)),
                            _ => None,
                        };
                        if let Some(value) = value {
                            relocated.insert(place, value);
                        }
                    }
                }
                elf::SHT_RELR => {
                    let Ok(Some(relrs)) = section.relr(endian, file.data()) else {
                        continue;
                    };
                    for place in relrs {
                        if let Some(value) = self.word(base, place) {
                            relocated.insert(place, value);
                        }
                    }
                }
                _ => {}
            }
        }
        // A file that is not moved when loaded holds its addresses in its data as they are.
        let fixed = file.elf_header().e_type(endian) == elf::ET_EXEC;
        let value_at = |place: u64| -> Option<u64> {
            relocated
                .get(&place)
                .copied()
                .or_else(|| fixed.then(|| self.word(base, place)).flatten())
        };

        for section in file.sections() {
            let name = section.name().unwrap_or_default();
            if matches!(name, ".init_array" | ".fini_array" | ".preinit_array") {
                let address = section.address();
                for place in (address..address + section.size()).step_by(8) {
                    starts.init_fini.extend(value_at(place).and_then(code));
                }
            }
        }

        starts
            .data
            .extend(relocated.values().filter_map(|&v| code(v)));
        if fixed {
            for h in file.elf_program_headers() {
                if h.p_type(endian) != elf::PT_LOAD || h.p_flags(endian).0 & elf::PF_X.0 != 0 {
                    continue;
                }
                let start = h.p_vaddr(endian).next_multiple_of(8);
                for place in (start..h.p_vaddr(endian) + h.p_filesz(endian)).step_by(8) {
                    starts.data.extend(self.word(base, place).and_then(code));
                }
            }
        }

        starts.unwind = self.unwind_starts(file, base);
        self.starts = starts;
    }

    /// The first address of every function the unwind tables (`.eh_frame`) describe.
    fn unwind_starts(&self, file: &File<'_>, base: u64) -> BTreeSet<u64> {
        use gimli::UnwindSection;
        let mut found = BTreeSet::new();
        let Some(section) = file.section_by_name(".eh_frame") else {
            return found;
        };
        let Ok(data) = section.data() else {
            return found;
        };
        let eh_frame = gimli::EhFrame::new(data, gimli::LittleEndian);
        let mut bases = gimli::BaseAddresses::default().set_eh_frame(section.address());
        if let Some(text) = file.section_by_name(".text") {
            bases = bases.set_text(text.address());
        }
        if let Some(got) = file.section_by_name(".got") {
            bases = bases.set_got(got.address());
        }
        let mut entries = eh_frame.entries(&bases);
        while let Ok(Some(entry)) = entries.next() {
            if let gimli::CieOrFde::Fde(partial) = entry
                && let Ok(fde) =
                    partial.parse(|section, bases, offset| section.cie_from_offset(bases, offset))
                && let Some(offset) = self.in_code(base, fde.initial_address())
            {
                found.insert(offset);
            }
        }
        found
    }
}
