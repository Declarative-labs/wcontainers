//! The Advanced SIMD loads and stores of structures: [LD1 to LD4 and ST1 to ST4 of multiple structures](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-advsimd#:~:text=LDSTMULTIPLE),
//! of one lane, and LD1R to LD4R, each decoded from its word as Arm's shared decode of the encoding class does.

use core::convert::Infallible;

use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};

use crate::behavior::{FaultAddressTag, MisalignedStackPointer, StoreAcrossFault};
use crate::branch::to_untagged_address;
use crate::encoding::{Field, FixedBits, RM, RN, RT};
use crate::load_store::{load, store};
use crate::operand::RegisterThirtyOne;
use crate::registers::{read_register, write_register};
use crate::{
    Exception, Instruction, PagePermission, PreciseInstruction, Processor, VECTOR_REGISTERS,
};

/// The class of the encoding: op0 0b0x00 with bits 29 to 25 0b00110, the structure loads and stores.
const STRUCTURE_SPACE: FixedBits = FixedBits {
    mask: 0xBE00_0000,
    value: 0x0C00_0000,
};
/// The feature of the structure loads and stores of the base Advanced SIMD extension.
const ADVANCED_SIMD: &str = "FEAT_AdvSIMD";
const Q: Field = Field::new(30, 1);
/// L: a load, not a store.
const LOAD: Field = Field::new(22, 1);
/// Whether the instruction writes its base back, after the access.
const POST_INDEX: Field = Field::new(23, 1);
/// Whether the instruction moves one lane of each register, or replicates one, rather than whole registers.
const SINGLE: Field = Field::new(24, 1);
const MULTIPLE_OPCODE: Field = Field::new(12, 4);
const SINGLE_OPCODE: Field = Field::new(13, 3);
const REPEAT: Field = Field::new(21, 1);
const LANE_S: Field = Field::new(12, 1);
const SIZE: Field = Field::new(10, 2);
/// The bits of a single-structure encoding that may hold the lane's index, below Q.
const INDEX_BITS: u32 = 0x1C00;
/// The register number that makes a post-index the size of the access.
const IMMEDIATE_POST_INDEX: u32 = 31;
/// The alignment [CheckSPAlignment](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-memory#impl-aarch64.CheckSPAlignment.0) requires.
const STACK_POINTER_ALIGNMENT: u64 = 16;
/// The most bytes one instruction moves: four whole registers.
const MAXIMUM_BYTES: usize = 4 * size_of::<u128>();

/// A load or store of structures whose behavior is modeled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StructureInstruction(Instruction<Infallible>);

/// What the instruction does in each case the manual leaves open or a chip does its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StructureChoices {
    pub misaligned_stack_pointer: MisalignedStackPointer,
    /// None for a load.
    pub across_fault: Option<StoreAcrossFault>,
    pub fault_address_tag: FaultAddressTag,
}

/// How an encoding takes its elements: whole registers, one lane of each, or one element replicated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Form {
    Multiple { repeats: u32 },
    Lane { index: u32 },
    Replicate,
}

/// What the word's fields decode to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Decoded {
    form: Form,
    /// The registers of each structure.
    structure_elements: u32,
    element_bytes: u32,
    /// The bytes of each register a whole-register or replicating form writes: 8 or 16.
    register_bytes: u32,
}

impl Decoded {
    const fn to_access_bytes(self) -> u32 {
        let elements = match self.form {
            Form::Multiple { repeats } => repeats * (self.register_bytes / self.element_bytes),
            Form::Lane { .. } | Form::Replicate => 1,
        };
        elements * self.structure_elements * self.element_bytes
    }
}

/// The shared decode of [LD1 (multiple structures)](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/LD1--multiple-structures---Load-multiple-single-element-structures-to-one--two--three--or-four-registers-)
/// and [LD1 (single structure)](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/LD1--single-structure---Load-one-single-element-structure-to-one-lane-of-one-register-)
/// and their families, or the undefined instruction of an unallocated encoding.
fn decode(word: u32) -> Result<Decoded, Exception> {
    let q = Q.read(word);
    let size = SIZE.read(word);
    let register_bytes = 8 << q;
    if SINGLE.read(word) == 0 {
        let (repeats, structure_elements) = match MULTIPLE_OPCODE.read(word) {
            0b0000 => (1, 4),
            0b0010 => (4, 1),
            0b0100 => (1, 3),
            0b0110 => (3, 1),
            0b0111 => (1, 1),
            0b1000 => (1, 2),
            0b1010 => (2, 1),
            _ => return Err(Exception::UndefinedInstruction),
        };
        // The .1D arrangement only for LD1 and ST1.
        if size == 0b11 && q == 0 && structure_elements != 1 {
            return Err(Exception::UndefinedInstruction);
        }
        return Ok(Decoded {
            form: Form::Multiple { repeats },
            structure_elements,
            element_bytes: 1 << size,
            register_bytes,
        });
    }
    let opcode = SINGLE_OPCODE.read(word);
    let s = LANE_S.read(word);
    let structure_elements = (((opcode & 1) << 1) | REPEAT.read(word)) + 1;
    let (form, scale) = match opcode >> 1 {
        3 => {
            if LOAD.read(word) == 0 || s == 1 {
                return Err(Exception::UndefinedInstruction);
            }
            (Form::Replicate, size)
        }
        0 => (
            Form::Lane {
                index: ((u32::from(!crate::planted::is(714)) * q) << 3) | (s << 2) | size,
            },
            0,
        ),
        1 if size & 1 == 1 => return Err(Exception::UndefinedInstruction),
        1 => (
            Form::Lane {
                index: (q << 2) | (s << 1) | (size >> 1),
            },
            1,
        ),
        _ if size & 0b10 != 0 => return Err(Exception::UndefinedInstruction),
        _ if size == 0 => (
            Form::Lane {
                index: (q << 1) | s,
            },
            2,
        ),
        _ if s == 1 => return Err(Exception::UndefinedInstruction),
        _ => (Form::Lane { index: q }, 3),
    };
    Ok(Decoded {
        form,
        structure_elements,
        element_bytes: 1 << scale,
        register_bytes,
    })
}

impl StructureInstruction {
    /// None for an instruction that is not a structure load or store of the base Advanced SIMD extension.
    pub fn new(instruction: Instruction<Infallible>) -> Option<Self> {
        let record = instruction.to_record()?;
        (record.feature == ADVANCED_SIMD
            && record.fixed_bits.mask & STRUCTURE_SPACE.mask == STRUCTURE_SPACE.mask
            && STRUCTURE_SPACE.is_matching(record.fixed_bits.value))
        .then_some(Self(instruction))
    }

    pub const fn instruction(self) -> Instruction<Infallible> {
        self.0
    }

    fn to_fixed_bits(self) -> FixedBits {
        self.0.to_fixed_bits()
    }

    pub fn is_store(self) -> bool {
        LOAD.read(self.to_fixed_bits().value) == 0
    }

    /// Whether it writes back a register's value rather than the access's size.
    pub fn has_register_offset(self) -> bool {
        POST_INDEX.read(self.to_fixed_bits().value) == 1
            && self.to_fixed_bits().mask & RM.to_mask() == 0
    }

    /// The registers of the list the encoding names, from Rt.
    pub fn to_register_count(self) -> u32 {
        let value = self.to_fixed_bits().value;
        match decode(value) {
            Ok(Decoded {
                form: Form::Multiple { repeats },
                structure_elements,
                ..
            }) => repeats * structure_elements,
            Ok(decoded) => decoded.structure_elements,
            Err(_) => 0,
        }
    }

    /// The field below Q that selects the arrangement or the lane, none where the encoding fixes it.
    pub fn to_index_field(self) -> Option<Field> {
        let free = INDEX_BITS & !self.to_fixed_bits().mask;
        (free != 0).then(|| Field::new(free.trailing_zeros(), free.count_ones()))
    }

    /// The bytes the encoding accesses from its base, none for an unallocated one.
    pub fn to_access_bytes(self, word: u32) -> Option<u64> {
        decode(word)
            .ok()
            .map(|decoded| u64::from(decoded.to_access_bytes()))
    }

    /// Applies the instruction encoded in the word; changes no register when it returns an exception.
    pub(crate) fn simulate<LevelSpecific, Architecture>(
        self,
        choices: StructureChoices,
        processor: &mut Processor<LevelSpecific>,
        memory: &[MemoryDelta<'_, Architecture>],
        word: u32,
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> Result<(), Exception>
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    {
        let decoded = decode(word)?;
        let n = RN.read(word);
        let address = read_register(processor, n, RegisterThirtyOne::StackPointer);
        if n as usize == crate::GENERAL_PURPOSE_REGISTERS
            && !address.is_multiple_of(STACK_POINTER_ALIGNMENT)
            && choices.misaligned_stack_pointer == MisalignedStackPointer::Faults
        {
            return Err(Exception::StackPointerAlignmentFault);
        }
        let location = to_untagged_address(address);
        let total = decoded.to_access_bytes() as usize;
        let accesses = list_accesses(decoded, RT.read(word));
        let element_bytes = decoded.element_bytes as usize;
        let mut data = [0_u8; MAXIMUM_BYTES];
        if LOAD.read(word) == 0 {
            for (place, (register, lane)) in accesses.iter().take(total / element_bytes).enumerate()
            {
                let value = processor.vector_registers[*register as usize].to_le_bytes();
                let lane = *lane as usize * element_bytes;
                data[place * element_bytes..(place + 1) * element_bytes]
                    .copy_from_slice(&value[lane..lane + element_bytes]);
            }
            store(
                memory,
                address,
                location,
                choices.fault_address_tag,
                &data[..total],
                choices
                    .across_fault
                    .unwrap_or(StoreAcrossFault::WritesNothing),
                mutation_hook,
            )?;
        } else {
            load(
                memory,
                address,
                location,
                choices.fault_address_tag,
                &mut data[..total],
            )?;
            write_loaded(processor, decoded, &accesses, &data[..total]);
        }
        if POST_INDEX.read(word) == 1 {
            let m = RM.read(word);
            let offset = if m == IMMEDIATE_POST_INDEX {
                total as u64
            } else {
                read_register(processor, m, RegisterThirtyOne::ZeroRegister)
            };
            let offset = if crate::planted::is(715) {
                offset / 2
            } else {
                offset
            };
            write_register(
                processor,
                n,
                address.wrapping_add(offset),
                RegisterThirtyOne::StackPointer,
            );
        }
        Ok(())
    }
}

/// The register and element of each access, in the order of the pseudocode's loops, which is the order of the bytes.
fn list_accesses(decoded: Decoded, first: u32) -> [(u32, u32); MAXIMUM_BYTES] {
    let mut accesses = [(0, 0); MAXIMUM_BYTES];
    let registers = VECTOR_REGISTERS as u32;
    let mut place = 0;
    match decoded.form {
        Form::Multiple { repeats } => {
            for repeat in 0..repeats {
                for element in 0..decoded.register_bytes / decoded.element_bytes {
                    for structure_element in 0..decoded.structure_elements {
                        let register = if crate::planted::is(716) {
                            (first + structure_element) % registers
                        } else {
                            (first + repeat + structure_element) % registers
                        };
                        accesses[place] = (register, element);
                        place += 1;
                    }
                }
            }
        }
        Form::Lane { index } => {
            for structure_element in 0..decoded.structure_elements {
                accesses[place] = ((first + structure_element) % registers, index);
                place += 1;
            }
        }
        Form::Replicate => {
            for structure_element in 0..decoded.structure_elements {
                accesses[place] = ((first + structure_element) % registers, 0);
                place += 1;
            }
        }
    }
    accesses
}

/// Writes what a load read: each element into its lane; a whole-register form zeroes the bytes above the register's
/// bytes, a lane keeps the other lanes, and a replicating form fills every element of the register's bytes.
fn write_loaded<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: Decoded,
    accesses: &[(u32, u32)],
    data: &[u8],
) {
    let element_bytes = decoded.element_bytes as usize;
    let mut values = processor.vector_registers;
    if matches!(decoded.form, Form::Multiple { .. }) {
        for (place, _) in data.chunks(element_bytes).enumerate() {
            values[accesses[place].0 as usize] = 0;
        }
    }
    for (place, element) in data.chunks(element_bytes).enumerate() {
        let (register, lane) = accesses[place];
        let mut bytes = values[register as usize].to_le_bytes();
        match decoded.form {
            Form::Multiple { .. } | Form::Lane { .. } => {
                let start = lane as usize * element_bytes;
                bytes[start..start + element_bytes].copy_from_slice(element);
            }
            Form::Replicate => {
                bytes = [0; size_of::<u128>()];
                let filled = if crate::planted::is(717) {
                    size_of::<u128>()
                } else {
                    decoded.register_bytes as usize
                };
                for chunk in bytes[..filled].chunks_mut(element_bytes) {
                    chunk.copy_from_slice(element);
                }
            }
        }
        values[register as usize] = u128::from_le_bytes(bytes);
    }
    processor.vector_registers = values;
}

/// Each combination of the choices the instruction has, with the behavior of a chip without Advanced SIMD left to the
/// caller.
pub(crate) fn list_structure_behaviors(
    instruction: StructureInstruction,
) -> impl Iterator<Item = PreciseInstruction<Infallible>> {
    // A store of one byte never lies across a fault.
    let across: &[Option<StoreAcrossFault>] = if instruction.is_store()
        && instruction
            .to_access_bytes(instruction.to_fixed_bits().value)
            .is_some_and(|bytes| bytes > 1)
    {
        &[
            Some(StoreAcrossFault::WritesNothing),
            Some(StoreAcrossFault::WritesBytesBelowFault),
        ]
    } else {
        &[None]
    };
    [
        MisalignedStackPointer::Faults,
        MisalignedStackPointer::Accesses,
    ]
    .into_iter()
    .flat_map(move |misaligned_stack_pointer| {
        across.iter().flat_map(move |across_fault| {
            [
                FaultAddressTag::KeepsTopByte,
                FaultAddressTag::ClearsTopNibble,
            ]
            .into_iter()
            .map(move |fault_address_tag| PreciseInstruction::Structure {
                instruction,
                choices: StructureChoices {
                    misaligned_stack_pointer,
                    across_fault: *across_fault,
                    fault_address_tag,
                },
            })
        })
    })
}
