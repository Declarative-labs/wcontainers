//! A simulated weaker engine for the litmus shapes (XF3): every guest access of translated code calls the runner.
//!
//! `instrument` makes each guest load, store, compare and exchange and fence a call carrying its kind, width and
//! sign; the rest of the code is the translation's own. A [`Script`] chooses what each load of a shared word sees.

use wasm_encoder::reencode::{Error, Reencode, utils};
use wasm_encoder::{CodeSection, EntityType, ImportSection, Instruction, TypeSection, ValType};
use wasmparser::{MemArg, MemArgTarget, Operator};
use wcontainers_x86::GUEST_MEMORY;

/// The module instrumented code imports its accesses from.
pub const SIMULATED: &str = "simulated";
/// The imports, by what they take and give.
///
/// Each takes an address, the operands, the memory argument's offset and the access (`Access::encode`).
pub const LOAD_32: &str = "load_32";
pub const LOAD_64: &str = "load_64";
pub const STORE_32: &str = "store_32";
pub const STORE_64: &str = "store_64";
pub const EXCHANGE_32: &str = "exchange_32";
pub const EXCHANGE_64: &str = "exchange_64";
pub const FENCE: &str = "fence";

/// One import of the simulated engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Import {
    Load32,
    Load64,
    Store32,
    Store64,
    Exchange32,
    Exchange64,
    Fence,
}

/// Every import, in the order they are added to a module.
const IMPORTS: &[Import] = &[
    Import::Load32,
    Import::Load64,
    Import::Store32,
    Import::Store64,
    Import::Exchange32,
    Import::Exchange64,
    Import::Fence,
];

impl Import {
    fn name(self) -> &'static str {
        match self {
            Import::Load32 => LOAD_32,
            Import::Load64 => LOAD_64,
            Import::Store32 => STORE_32,
            Import::Store64 => STORE_64,
            Import::Exchange32 => EXCHANGE_32,
            Import::Exchange64 => EXCHANGE_64,
            Import::Fence => FENCE,
        }
    }

    /// Returns the import's parameters and results.
    fn signature(self) -> (Vec<ValType>, Vec<ValType>) {
        use ValType::{I32, I64};
        match self {
            Import::Load32 => (vec![I64, I64, I32], vec![I32]),
            Import::Load64 => (vec![I64, I64, I32], vec![I64]),
            Import::Store32 => (vec![I64, I32, I64, I32], vec![]),
            Import::Store64 => (vec![I64, I64, I64, I32], vec![]),
            Import::Exchange32 => (vec![I64, I32, I32, I64, I32], vec![I32]),
            Import::Exchange64 => (vec![I64, I64, I64, I64, I32], vec![I64]),
            Import::Fence => (vec![], vec![]),
        }
    }

    fn position(self) -> u32 {
        IMPORTS.iter().position(|&i| i == self).unwrap() as u32
    }
}

/// Whether an access is a plain one or an atomic one of WebAssembly's threads proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Plain,
    Atomic,
}

/// What an access does with the bytes it reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// A load, sign-extended or not.
    Load {
        signed: bool,
    },
    Store,
    /// A compare and exchange: the old value is given, and the replacement written where it was the expected one.
    Exchange,
}

/// One guest access of instrumented code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Access {
    pub kind: Kind,
    pub operation: Operation,
    /// How many bytes it reaches.
    pub bytes: u8,
}

/// The operations, by their place in an encoded access's last byte (a load's sign apart).
const OPERATIONS: &[Operation] = &[
    Operation::Load { signed: false },
    Operation::Store,
    Operation::Exchange,
];

impl Access {
    /// Encodes the access as the instrumented code passes it to an import: its bytes, sign, kind and operation.
    pub fn encode(self) -> u32 {
        let (unsigned, signed) = match self.operation {
            Operation::Load { signed } => (Operation::Load { signed: false }, signed),
            other => (other, false),
        };
        let operation = OPERATIONS.iter().position(|&o| o == unsigned).unwrap();
        u32::from_le_bytes([
            self.bytes,
            u8::from(signed),
            u8::from(self.kind == Kind::Atomic),
            operation as u8,
        ])
    }

    /// Decodes the access an import was passed.
    pub fn decode(encoded: u32) -> Access {
        let [bytes, signed, atomic, operation] = encoded.to_le_bytes();
        let operation = match OPERATIONS.get(usize::from(operation)) {
            Some(Operation::Load { .. }) => Operation::Load {
                signed: signed != 0,
            },
            Some(&other) => other,
            None => panic!("no access operation {operation}"),
        };
        Access {
            kind: if atomic != 0 {
                Kind::Atomic
            } else {
                Kind::Plain
            },
            operation,
            bytes,
        }
    }

    /// Returns what the access gives the code and what it writes, given the value its bytes held and its operands.
    ///
    /// The value is zero-extended; the operands are the value of a store, or the expected value and the replacement of
    /// an exchange.
    pub fn apply(self, old: u64, operands: &[u64]) -> (u64, Option<u64>) {
        let bits = u32::from(self.bytes) * u8::BITS;
        let mask = u64::MAX >> (u64::BITS - bits);
        let old = old & mask;
        match (self.operation, operands) {
            (Operation::Load { signed: false }, []) => (old, None),
            (Operation::Load { signed: true }, []) => {
                let shift = u64::BITS - bits;
                ((((old << shift) as i64) >> shift) as u64, None)
            }
            (Operation::Store, &[value]) => (0, Some(value & mask)),
            (Operation::Exchange, &[expected, replacement]) => {
                let written = (old == expected & mask).then_some(replacement & mask);
                (old, written)
            }
            (operation, operands) => panic!("{operation:?} with operands {operands:?}"),
        }
    }
}

/// What one thread did to a shared word, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Event {
    Read {
        word: u64,
        value: u64,
    },
    Write {
        word: u64,
        value: u64,
    },
    /// A compare and exchange that found `found` and wrote `written` if it was the expected value.
    Exchange {
        word: u64,
        found: u64,
        written: Option<u64>,
    },
    Fence,
}

/// The bytes of a shared word.
pub const WORD_BYTES: u8 = size_of::<u64>() as u8;
/// The widths of the other accesses.
const BYTE_ACCESS: u8 = size_of::<u8>() as u8;
const HALF_WORD: u8 = size_of::<u16>() as u8;
const SINGLE_WORD: u8 = size_of::<u32>() as u8;

/// One thread's run on the simulated engine: its shared words, the values its reads of them see, and what it did.
pub struct Script {
    /// The addresses of the shared words, each [`WORD_BYTES`] long.
    shared: Vec<u64>,
    /// The value each read of a shared word sees, in the order of the reads; a read past them sees 0.
    reads: Vec<u64>,
    /// How many reads of shared words were made.
    reads_made: usize,
    /// What the thread did to the shared words, with the kind of each access, in its program order.
    pub events: Vec<(Kind, Event)>,
}

impl Script {
    pub fn new(shared: Vec<u64>, reads: Vec<u64>) -> Script {
        Script {
            shared,
            reads,
            reads_made: 0,
            events: Vec::new(),
        }
    }

    /// Returns how many reads of shared words the run made.
    pub fn reads_made(&self) -> usize {
        self.reads_made
    }

    /// Tells whether an access reaches a shared word; one that reaches part of one, or more, stops the run.
    pub fn is_shared(&self, address: u64, access: Access) -> bool {
        let end = address + u64::from(access.bytes);
        let overlapping = self
            .shared
            .iter()
            .find(|&&word| address < word + u64::from(WORD_BYTES) && word < end);
        match overlapping {
            None => false,
            Some(&word) => {
                assert!(
                    word == address && access.bytes == WORD_BYTES,
                    "an access of {} bytes at {address:#x} reaches part of the shared word at {word:#x}",
                    access.bytes
                );
                true
            }
        }
    }

    /// Makes an access to a shared word: a read sees the script's next value; the access is recorded.
    pub fn shared_access(&mut self, word: u64, access: Access, operands: &[u64]) -> u64 {
        let old = match access.operation {
            Operation::Store => 0,
            Operation::Load { .. } | Operation::Exchange => {
                let value = self.reads.get(self.reads_made).copied().unwrap_or(0);
                self.reads_made += 1;
                value
            }
        };
        let (given, written) = access.apply(old, operands);
        let event = match (access.operation, written) {
            (Operation::Load { .. }, _) => Event::Read { word, value: old },
            (Operation::Store, Some(value)) => Event::Write { word, value },
            (Operation::Exchange, written) => Event::Exchange {
                word,
                found: old,
                written,
            },
            (Operation::Store, None) => unreachable!("a store writes"),
        };
        self.events.push((access.kind, event));
        given
    }

    /// Records a fence, an atomic one.
    pub fn fence(&mut self) {
        self.events.push((Kind::Atomic, Event::Fence));
    }
}

/// What a guest access becomes: the import it calls, the access, and the conversions before and after the call.
struct Replacement {
    import: Import,
    access: Access,
    memarg: MemArg,
    /// Turns a floating point value to store into its bits.
    before: Option<Instruction<'static>>,
    /// Turns loaded bits into a floating point value.
    after: Option<Instruction<'static>>,
}

/// Finds what an operator on the guest memory becomes, or none for one that does not reach it.
///
/// An operator on the guest memory the simulation does not make (a vector, bulk or other read-modify-write access)
/// stops the run.
fn replacement(operator: &Operator<'_>) -> Option<Replacement> {
    use Operator::*;
    let load = |signed| Operation::Load { signed };
    let (import, kind, operation, bytes, memarg) = match *operator {
        I32Load { memarg } | F32Load { memarg } => (
            Import::Load32,
            Kind::Plain,
            load(false),
            SINGLE_WORD,
            memarg,
        ),
        I64Load { memarg } | F64Load { memarg } => {
            (Import::Load64, Kind::Plain, load(false), WORD_BYTES, memarg)
        }
        I32Load8S { memarg } => (Import::Load32, Kind::Plain, load(true), BYTE_ACCESS, memarg),
        I32Load8U { memarg } => (
            Import::Load32,
            Kind::Plain,
            load(false),
            BYTE_ACCESS,
            memarg,
        ),
        I32Load16S { memarg } => (Import::Load32, Kind::Plain, load(true), HALF_WORD, memarg),
        I32Load16U { memarg } => (Import::Load32, Kind::Plain, load(false), HALF_WORD, memarg),
        I64Load8S { memarg } => (Import::Load64, Kind::Plain, load(true), BYTE_ACCESS, memarg),
        I64Load8U { memarg } => (
            Import::Load64,
            Kind::Plain,
            load(false),
            BYTE_ACCESS,
            memarg,
        ),
        I64Load16S { memarg } => (Import::Load64, Kind::Plain, load(true), HALF_WORD, memarg),
        I64Load16U { memarg } => (Import::Load64, Kind::Plain, load(false), HALF_WORD, memarg),
        I64Load32S { memarg } => (Import::Load64, Kind::Plain, load(true), SINGLE_WORD, memarg),
        I64Load32U { memarg } => (
            Import::Load64,
            Kind::Plain,
            load(false),
            SINGLE_WORD,
            memarg,
        ),
        I32Store { memarg } | F32Store { memarg } => (
            Import::Store32,
            Kind::Plain,
            Operation::Store,
            SINGLE_WORD,
            memarg,
        ),
        I64Store { memarg } | F64Store { memarg } => (
            Import::Store64,
            Kind::Plain,
            Operation::Store,
            WORD_BYTES,
            memarg,
        ),
        I32Store8 { memarg } => (
            Import::Store32,
            Kind::Plain,
            Operation::Store,
            BYTE_ACCESS,
            memarg,
        ),
        I32Store16 { memarg } => (
            Import::Store32,
            Kind::Plain,
            Operation::Store,
            HALF_WORD,
            memarg,
        ),
        I64Store8 { memarg } => (
            Import::Store64,
            Kind::Plain,
            Operation::Store,
            BYTE_ACCESS,
            memarg,
        ),
        I64Store16 { memarg } => (
            Import::Store64,
            Kind::Plain,
            Operation::Store,
            HALF_WORD,
            memarg,
        ),
        I64Store32 { memarg } => (
            Import::Store64,
            Kind::Plain,
            Operation::Store,
            SINGLE_WORD,
            memarg,
        ),
        I32AtomicLoad { memarg } => (
            Import::Load32,
            Kind::Atomic,
            load(false),
            SINGLE_WORD,
            memarg,
        ),
        I32AtomicLoad8U { memarg } => (
            Import::Load32,
            Kind::Atomic,
            load(false),
            BYTE_ACCESS,
            memarg,
        ),
        I32AtomicLoad16U { memarg } => {
            (Import::Load32, Kind::Atomic, load(false), HALF_WORD, memarg)
        }
        I64AtomicLoad { memarg } => (
            Import::Load64,
            Kind::Atomic,
            load(false),
            WORD_BYTES,
            memarg,
        ),
        I64AtomicLoad8U { memarg } => (
            Import::Load64,
            Kind::Atomic,
            load(false),
            BYTE_ACCESS,
            memarg,
        ),
        I64AtomicLoad16U { memarg } => {
            (Import::Load64, Kind::Atomic, load(false), HALF_WORD, memarg)
        }
        I64AtomicLoad32U { memarg } => (
            Import::Load64,
            Kind::Atomic,
            load(false),
            SINGLE_WORD,
            memarg,
        ),
        I32AtomicStore { memarg } => (
            Import::Store32,
            Kind::Atomic,
            Operation::Store,
            SINGLE_WORD,
            memarg,
        ),
        I32AtomicStore8 { memarg } => (
            Import::Store32,
            Kind::Atomic,
            Operation::Store,
            BYTE_ACCESS,
            memarg,
        ),
        I32AtomicStore16 { memarg } => (
            Import::Store32,
            Kind::Atomic,
            Operation::Store,
            HALF_WORD,
            memarg,
        ),
        I64AtomicStore { memarg } => (
            Import::Store64,
            Kind::Atomic,
            Operation::Store,
            WORD_BYTES,
            memarg,
        ),
        I64AtomicStore8 { memarg } => (
            Import::Store64,
            Kind::Atomic,
            Operation::Store,
            BYTE_ACCESS,
            memarg,
        ),
        I64AtomicStore16 { memarg } => (
            Import::Store64,
            Kind::Atomic,
            Operation::Store,
            HALF_WORD,
            memarg,
        ),
        I64AtomicStore32 { memarg } => (
            Import::Store64,
            Kind::Atomic,
            Operation::Store,
            SINGLE_WORD,
            memarg,
        ),
        I32AtomicRmwCmpxchg { memarg } => (
            Import::Exchange32,
            Kind::Atomic,
            Operation::Exchange,
            SINGLE_WORD,
            memarg,
        ),
        I32AtomicRmw8CmpxchgU { memarg } => (
            Import::Exchange32,
            Kind::Atomic,
            Operation::Exchange,
            BYTE_ACCESS,
            memarg,
        ),
        I32AtomicRmw16CmpxchgU { memarg } => (
            Import::Exchange32,
            Kind::Atomic,
            Operation::Exchange,
            HALF_WORD,
            memarg,
        ),
        I64AtomicRmwCmpxchg { memarg } => (
            Import::Exchange64,
            Kind::Atomic,
            Operation::Exchange,
            WORD_BYTES,
            memarg,
        ),
        I64AtomicRmw8CmpxchgU { memarg } => (
            Import::Exchange64,
            Kind::Atomic,
            Operation::Exchange,
            BYTE_ACCESS,
            memarg,
        ),
        I64AtomicRmw16CmpxchgU { memarg } => (
            Import::Exchange64,
            Kind::Atomic,
            Operation::Exchange,
            HALF_WORD,
            memarg,
        ),
        I64AtomicRmw32CmpxchgU { memarg } => (
            Import::Exchange64,
            Kind::Atomic,
            Operation::Exchange,
            SINGLE_WORD,
            memarg,
        ),
        MemoryCopy { dst_mem, src_mem } => {
            assert!(
                dst_mem != GUEST_MEMORY && src_mem != GUEST_MEMORY,
                "a bulk copy of the guest memory, which the simulation does not make"
            );
            return None;
        }
        MemoryFill { mem } => {
            assert!(
                mem != GUEST_MEMORY,
                "a bulk fill of the guest memory, which the simulation does not make"
            );
            return None;
        }
        ref other => {
            let reaches_guest =
                format!("{other:?}").contains(&format!("target: Memory({GUEST_MEMORY})"));
            assert!(
                !reaches_guest,
                "an access the simulation does not make: {other:?}"
            );
            return None;
        }
    };
    if memarg.target != MemArgTarget::Memory(GUEST_MEMORY) {
        return None;
    }
    let (before, after) = match *operator {
        F32Load { .. } => (None, Some(Instruction::F32ReinterpretI32)),
        F64Load { .. } => (None, Some(Instruction::F64ReinterpretI64)),
        F32Store { .. } => (Some(Instruction::I32ReinterpretF32), None),
        F64Store { .. } => (Some(Instruction::I64ReinterpretF64), None),
        _ => (None, None),
    };
    Some(Replacement {
        import,
        access: Access {
            kind,
            operation,
            bytes,
        },
        memarg,
        before,
        after,
    })
}

/// Rewrites a module so that each guest access calls the simulated engine's import for it.
///
/// The imports are added after the module's own, so the functions it defines move up by as many.
struct Instrumenter {
    /// The module's own function imports, and its own types.
    imported_functions: u32,
    types: u32,
}

impl Reencode for Instrumenter {
    type Error = std::convert::Infallible;

    fn function_index(&mut self, func: u32) -> Result<u32, Error<Self::Error>> {
        Ok(if func < self.imported_functions {
            func
        } else {
            func + IMPORTS.len() as u32
        })
    }

    fn entity_type(
        &mut self,
        type_ref: wasmparser::TypeRef,
    ) -> Result<EntityType, Error<Self::Error>> {
        if matches!(
            type_ref,
            wasmparser::TypeRef::Func(_) | wasmparser::TypeRef::FuncExact(_)
        ) {
            self.imported_functions += 1;
        }
        utils::entity_type(self, type_ref)
    }

    fn parse_type_section(
        &mut self,
        types: &mut TypeSection,
        section: wasmparser::TypeSectionReader<'_>,
    ) -> Result<(), Error<Self::Error>> {
        for group in section.clone() {
            self.types += group?.types().count() as u32;
        }
        utils::parse_type_section(self, types, section)?;
        for import in IMPORTS {
            let (parameters, results) = import.signature();
            types.ty().function(parameters, results);
        }
        Ok(())
    }

    fn parse_import_section(
        &mut self,
        imports: &mut ImportSection,
        section: wasmparser::ImportSectionReader<'_>,
    ) -> Result<(), Error<Self::Error>> {
        utils::parse_import_section(self, imports, section)?;
        for import in IMPORTS {
            imports.import(
                SIMULATED,
                import.name(),
                EntityType::Function(self.types + import.position()),
            );
        }
        Ok(())
    }

    fn parse_function_body(
        &mut self,
        code: &mut CodeSection,
        func: wasmparser::FunctionBody<'_>,
    ) -> Result<(), Error<Self::Error>> {
        let mut function = self.new_function_with_parsed_locals(&func)?;
        let mut reader = func.get_operators_reader()?;
        let first_import = self.imported_functions;
        let call = |import: Import| Instruction::Call(first_import + import.position());
        while !reader.eof() {
            let operator = reader.read()?;
            if let Operator::AtomicFence { .. } = operator {
                function.instruction(&call(Import::Fence));
                continue;
            }
            let Some(replaced) = replacement(&operator) else {
                function.instruction(&self.instruction(operator)?);
                continue;
            };
            if let Some(before) = &replaced.before {
                function.instruction(before);
            }
            function
                .instruction(&Instruction::I64Const(replaced.memarg.offset as i64))
                .instruction(&Instruction::I32Const(replaced.access.encode() as i32))
                .instruction(&call(replaced.import));
            if let Some(after) = &replaced.after {
                function.instruction(after);
            }
        }
        code.function(&function);
        Ok(())
    }
}

/// Returns a code module whose every access to the guest memory calls the simulated engine's import for it.
pub fn instrument(module: &[u8]) -> Vec<u8> {
    let mut instrumenter = Instrumenter {
        imported_functions: 0,
        types: 0,
    };
    let mut out = wasm_encoder::Module::new();
    instrumenter
        .parse_core_module(&mut out, wasmparser::Parser::new(0), module)
        .unwrap_or_else(|error| panic!("a code module the simulation reads: {error:?}"));
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that every access survives its encoding, and what each gives and writes.
    #[test]
    fn accesses_encode_and_apply() {
        for kind in [Kind::Plain, Kind::Atomic] {
            for operation in [
                Operation::Load { signed: false },
                Operation::Load { signed: true },
                Operation::Store,
                Operation::Exchange,
            ] {
                for bytes in [BYTE_ACCESS, HALF_WORD, SINGLE_WORD, WORD_BYTES] {
                    let access = Access {
                        kind,
                        operation,
                        bytes,
                    };
                    assert_eq!(Access::decode(access.encode()), access);
                }
            }
        }
        let access = |operation, bytes| Access {
            kind: Kind::Plain,
            operation,
            bytes,
        };
        assert_eq!(
            access(Operation::Load { signed: true }, 1).apply(0x80, &[]),
            (u64::MAX - 0x7f, None)
        );
        assert_eq!(
            access(Operation::Load { signed: false }, 2).apply(0x1_8000, &[]),
            (0x8000, None)
        );
        assert_eq!(
            access(Operation::Store, 4).apply(7, &[0x1_0000_0002]),
            (0, Some(2))
        );
        assert_eq!(
            access(Operation::Exchange, 8).apply(3, &[3, 9]),
            (3, Some(9))
        );
        assert_eq!(access(Operation::Exchange, 8).apply(4, &[3, 9]), (4, None));
    }
}
