//! The forms that convert between an MMX register's two doublewords and an XMM register's floating-point lanes, and
//! MASKMOVQ, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (vol. 2, and vol. 1, 11.6.5) gives them.
//!
//! Each conversion computes its two lanes as the SSE conversion of the same kind does, under MXCSR. A form that
//! names an MMX register raises #MF while an x87 exception is pending and puts the x87 state in MMX's use; CVTPI2PS
//! and CVTPI2PD from memory name none. CVTPI2PS keeps the destination's upper quadword; CVTPI2PD writes the whole
//! register. MASKMOVQ stores the bytes of its first MMX register whose byte in the second has its top bit set, at
//! DS:RDI, as MASKMOVDQU does with an XMM register.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::PreciseInstruction;
use crate::behavior::StoreAcrossFault;
use crate::float::{Conversion, Precision};
use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::sse::MaskedStoreFaults;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MmxConvertOperation {
    /// Two lanes of the source converted into the destination's two lanes.
    Convert(Conversion),
    /// MASKMOVQ.
    MaskedStore,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MmxConvertForm {
    pub instruction: Instruction,
    pub operation: MmxConvertOperation,
    /// The precision of the floating-point side.
    pub precision: Precision,
    pub place: OperandPlace,
    pub code: Code,
}

impl MmxConvertForm {
    /// Whether the form names an MMX register, so that it waits for a pending x87 exception and enters MMX's use.
    pub fn names_mmx(&self) -> bool {
        !matches!(self.operation, MmxConvertOperation::Convert(Conversion::IntegerToFloat)) || self.place == OperandPlace::Register
    }

    /// The bytes a memory source reaches: 16 for two doubles, else 8.
    pub fn memory_bytes(&self) -> u64 {
        match (self.operation, self.precision) {
            (MmxConvertOperation::Convert(Conversion::FloatToInteger { .. }), Precision::Double) => 16,
            _ => 8,
        }
    }
}

/// A form's behavior: what MASKMOVQ writes below a fault and which of its bytes its checks reach.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MmxConvertBehavior {
    pub across_fault: StoreAcrossFault,
    pub masked_faults: MaskedStoreFaults,
}

/// The row of a form of this family, or `None` for any other form.
pub fn find_mmx_convert_form(instruction: Instruction) -> Option<&'static MmxConvertForm> {
    MMX_CONVERT_FORMS.iter().find(|form| form.instruction == instruction)
}

/// Every behavior of MASKMOVQ's store, one for a conversion.
pub(crate) fn list_mmx_convert_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_mmx_convert_form(instruction) else {
        return Vec::new();
    };
    let first = MmxConvertBehavior { across_fault: StoreAcrossFault::WritesNothing, masked_faults: MaskedStoreFaults::EveryByte };
    if form.operation != MmxConvertOperation::MaskedStore {
        return alloc::vec![PreciseInstruction::MmxConvert { instruction, behavior: first }];
    }
    [MaskedStoreFaults::EveryByte, MaskedStoreFaults::SelectedBytes]
        .into_iter()
        .flat_map(|masked_faults| {
            [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault]
                .map(|across_fault| PreciseInstruction::MmxConvert { instruction, behavior: MmxConvertBehavior { across_fault, masked_faults } })
        })
        .collect()
}

const fn convert(instruction: Instruction, conversion: Conversion, precision: Precision, place: OperandPlace, code: Code) -> MmxConvertForm {
    MmxConvertForm { instruction, operation: MmxConvertOperation::Convert(conversion), precision, place, code }
}

use Conversion::{FloatToInteger, IntegerToFloat};
use OperandPlace::{Memory, Register};
use Precision::{Double, Single};

/// Every form of this family modeled.
pub static MMX_CONVERT_FORMS: &[MmxConvertForm] = &[
    convert(Instruction::Cvtpi2psXmmqMmxq, IntegerToFloat, Single, Register, Code::Cvtpi2ps_xmm_mmm64),
    convert(Instruction::Cvtpi2psXmmqMemq, IntegerToFloat, Single, Memory, Code::Cvtpi2ps_xmm_mmm64),
    convert(Instruction::Cvtpi2pdXmmpdMmxq, IntegerToFloat, Double, Register, Code::Cvtpi2pd_xmm_mmm64),
    convert(Instruction::Cvtpi2pdXmmpdMemq, IntegerToFloat, Double, Memory, Code::Cvtpi2pd_xmm_mmm64),
    convert(Instruction::Cvtps2piMmxqXmmq, FloatToInteger { truncate: false }, Single, Register, Code::Cvtps2pi_mm_xmmm64),
    convert(Instruction::Cvtps2piMmxqMemq, FloatToInteger { truncate: false }, Single, Memory, Code::Cvtps2pi_mm_xmmm64),
    convert(Instruction::Cvttps2piMmxqXmmq, FloatToInteger { truncate: true }, Single, Register, Code::Cvttps2pi_mm_xmmm64),
    convert(Instruction::Cvttps2piMmxqMemq, FloatToInteger { truncate: true }, Single, Memory, Code::Cvttps2pi_mm_xmmm64),
    convert(Instruction::Cvtpd2piMmxqXmmpd, FloatToInteger { truncate: false }, Double, Register, Code::Cvtpd2pi_mm_xmmm128),
    convert(Instruction::Cvtpd2piMmxqMempd, FloatToInteger { truncate: false }, Double, Memory, Code::Cvtpd2pi_mm_xmmm128),
    convert(Instruction::Cvttpd2piMmxqXmmpd, FloatToInteger { truncate: true }, Double, Register, Code::Cvttpd2pi_mm_xmmm128),
    convert(Instruction::Cvttpd2piMmxqMempd, FloatToInteger { truncate: true }, Double, Memory, Code::Cvttpd2pi_mm_xmmm128),
    MmxConvertForm {
        instruction: Instruction::MaskmovqMmxqMmxq,
        operation: MmxConvertOperation::MaskedStore,
        precision: Single,
        place: Register,
        code: Code::Maskmovq_rDI_mm_mm,
    },
];
