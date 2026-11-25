//! MRS and MSR (register) at EL0, as Linux configures EL1: which system registers they reach, which EL1 traps and
//! which are undefined, from the [system register index](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers)
//! and its EL0 access rules.

use crate::Processor;
use crate::condition_flags::ConditionFlags;
use crate::encoding::{Field, RT};
use crate::exception::{AccessDirection, Exception, SystemAccess};
use crate::operand::RegisterThirtyOne;
use crate::registers::{read_register, write_register};

/// The encoding of the register an MRS or MSR names: o0, op1, CRn, CRm and op2, op0 being two more than o0.
pub const SYSTEM_REGISTER: Field = Field::new(5, 15);
const OP0_BASE: u32 = 2;
const OP1_SHIFT: u32 = 11;
const CRN_SHIFT: u32 = 7;
const CRM_SHIFT: u32 = 3;
const OP0_SHIFT: u32 = 14;
const OP1_BITS: u32 = 3;
const CR_BITS: u32 = 4;
const OP2_BITS: u32 = 3;

/// The fields of a system register's encoding, as the manual names it `S<op0>_<op1>_C<n>_C<m>_<op2>`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemRegisterEncoding {
    pub op0: u32,
    pub op1: u32,
    pub crn: u32,
    pub crm: u32,
    pub op2: u32,
}

impl SystemRegisterEncoding {
    pub const fn new(op0: u32, op1: u32, crn: u32, crm: u32, op2: u32) -> Self {
        Self {
            op0,
            op1,
            crn,
            crm,
            op2,
        }
    }

    /// The value of the encoding's 15-bit field.
    pub const fn to_field(self) -> u32 {
        ((self.op0 - OP0_BASE) << OP0_SHIFT)
            | (self.op1 << OP1_SHIFT)
            | (self.crn << CRN_SHIFT)
            | (self.crm << CRM_SHIFT)
            | self.op2
    }

    pub const fn from_field(field: u32) -> Self {
        Self {
            op0: OP0_BASE + (field >> OP0_SHIFT),
            op1: (field >> OP1_SHIFT) & ((1 << OP1_BITS) - 1),
            crn: (field >> CRN_SHIFT) & ((1 << CR_BITS) - 1),
            crm: (field >> CRM_SHIFT) & ((1 << CR_BITS) - 1),
            op2: field & ((1 << OP2_BITS) - 1),
        }
    }
}

/// The part of the state a system register is, where the processor state holds it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeldRegister {
    /// [NZCV](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/NZCV--Condition-Flags).
    ConditionFlags,
    /// [FPCR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPCR--Floating-point-Control-Register).
    FloatingPointControl,
    /// [FPSR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FPSR--Floating-point-Status-Register).
    FloatingPointStatus,
    /// [TPIDR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TPIDR-EL0--EL0-Read-Write-Software-Thread-ID-Register).
    SoftwareThreadId,
}

/// A value a register holds that the processor state does not, fixed by the chip or by the executor's guest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FixedValue {
    /// [CTR_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register).
    CacheType,
    /// [DCZID_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/DCZID-EL0--Data-Cache-Zero-ID-Register).
    ZeroBlockSize,
    /// [TPIDRRO_EL0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TPIDRRO-EL0--EL0-Read-Only-Software-Thread-ID-Register), which EL1 writes.
    ReadOnlyThreadId,
}

/// What an MRS or MSR of a register does at EL0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemRegisterAccess {
    Held(HeldRegister),
    /// Reads a fixed value; a write is undefined.
    Fixed(FixedValue),
    /// Reads zero, and a write changes nothing the state holds: PSTATE's DIT, SSBS and TCO, which the executor's
    /// guest enters EL0 with clear, and the debug communication channel with no debugger.
    ReadsZero,
    /// Reads zero; a write is undefined.
    ReadOnlyZero,
    /// Reads a random number where the chip has the random number registers, and sets NZCV to say whether it gave one;
    /// a write is undefined.
    RandomNumber,
    /// EL1 traps it.
    Trapped,
    /// EL1 traps a read; a write is undefined.
    ReadTrapped,
    Undefined,
}

/// Each register EL0 can reach, or that EL1 traps there, with what an access does.
pub const SYSTEM_REGISTERS: [(&str, SystemRegisterEncoding, SystemRegisterAccess); 48] = {
    use SystemRegisterAccess as A;
    const fn at(op0: u32, op1: u32, crn: u32, crm: u32, op2: u32) -> SystemRegisterEncoding {
        SystemRegisterEncoding::new(op0, op1, crn, crm, op2)
    }
    [
        (
            "NZCV",
            at(3, 3, 4, 2, 0),
            A::Held(HeldRegister::ConditionFlags),
        ),
        // SCTLR_EL1.UMA is clear.
        ("DAIF", at(3, 3, 4, 2, 1), A::Trapped),
        ("DIT", at(3, 3, 4, 2, 5), A::ReadsZero),
        ("SSBS", at(3, 3, 4, 2, 6), A::ReadsZero),
        ("TCO", at(3, 3, 4, 2, 7), A::ReadsZero),
        (
            "FPCR",
            at(3, 3, 4, 4, 0),
            A::Held(HeldRegister::FloatingPointControl),
        ),
        (
            "FPSR",
            at(3, 3, 4, 4, 1),
            A::Held(HeldRegister::FloatingPointStatus),
        ),
        (
            "TPIDR_EL0",
            at(3, 3, 13, 0, 2),
            A::Held(HeldRegister::SoftwareThreadId),
        ),
        (
            "TPIDRRO_EL0",
            at(3, 3, 13, 0, 3),
            A::Fixed(FixedValue::ReadOnlyThreadId),
        ),
        // SCTLR_EL1.TSCXT is set.
        ("SCXTNUM_EL0", at(3, 3, 13, 0, 7), A::Trapped),
        // SCTLR_EL1.UCT and DZE are set.
        (
            "CTR_EL0",
            at(3, 3, 0, 0, 1),
            A::Fixed(FixedValue::CacheType),
        ),
        (
            "DCZID_EL0",
            at(3, 3, 0, 0, 7),
            A::Fixed(FixedValue::ZeroBlockSize),
        ),
        // MDSCR_EL1.TDCC is clear.
        ("MDCCSR_EL0", at(2, 3, 0, 1, 0), A::ReadOnlyZero),
        ("DBGDTR_EL0", at(2, 3, 0, 4, 0), A::ReadsZero),
        ("DBGDTRRX_EL0", at(2, 3, 0, 5, 0), A::ReadsZero),
        // CNTKCTL_EL1 gives EL0 no timer.
        ("CNTFRQ_EL0", at(3, 3, 14, 0, 0), A::ReadTrapped),
        ("CNTPCT_EL0", at(3, 3, 14, 0, 1), A::ReadTrapped),
        ("CNTVCT_EL0", at(3, 3, 14, 0, 2), A::ReadTrapped),
        ("CNTP_TVAL_EL0", at(3, 3, 14, 2, 0), A::Trapped),
        ("CNTP_CTL_EL0", at(3, 3, 14, 2, 1), A::Trapped),
        ("CNTP_CVAL_EL0", at(3, 3, 14, 2, 2), A::Trapped),
        ("CNTV_TVAL_EL0", at(3, 3, 14, 3, 0), A::Trapped),
        ("CNTV_CTL_EL0", at(3, 3, 14, 3, 1), A::Trapped),
        ("CNTV_CVAL_EL0", at(3, 3, 14, 3, 2), A::Trapped),
        // AMUSERENR_EL0.EN is clear: the activity monitors trap a read and leave a write undefined.
        ("AMCR_EL0", at(3, 3, 13, 2, 0), A::ReadTrapped),
        ("AMCFGR_EL0", at(3, 3, 13, 2, 1), A::ReadTrapped),
        ("AMCGCR_EL0", at(3, 3, 13, 2, 2), A::ReadTrapped),
        ("AMCNTENCLR0_EL0", at(3, 3, 13, 2, 4), A::ReadTrapped),
        ("AMCNTENSET0_EL0", at(3, 3, 13, 2, 5), A::ReadTrapped),
        ("AMCNTENCLR1_EL0", at(3, 3, 13, 3, 0), A::ReadTrapped),
        ("AMCNTENSET1_EL0", at(3, 3, 13, 3, 1), A::ReadTrapped),
        ("AMEVCNTR00_EL0", at(3, 3, 13, 4, 0), A::ReadTrapped),
        ("AMEVCNTR01_EL0", at(3, 3, 13, 4, 1), A::ReadTrapped),
        ("AMEVCNTR02_EL0", at(3, 3, 13, 4, 2), A::ReadTrapped),
        ("AMEVCNTR03_EL0", at(3, 3, 13, 4, 3), A::ReadTrapped),
        ("AMEVTYPER00_EL0", at(3, 3, 13, 6, 0), A::ReadTrapped),
        ("AMEVTYPER01_EL0", at(3, 3, 13, 6, 1), A::ReadTrapped),
        ("AMEVTYPER02_EL0", at(3, 3, 13, 6, 2), A::ReadTrapped),
        ("AMEVTYPER03_EL0", at(3, 3, 13, 6, 3), A::ReadTrapped),
        ("AMEVCNTR10_EL0", at(3, 3, 13, 12, 0), A::ReadTrapped),
        ("AMEVCNTR11_EL0", at(3, 3, 13, 12, 1), A::ReadTrapped),
        ("AMEVCNTR12_EL0", at(3, 3, 13, 12, 2), A::ReadTrapped),
        ("AMEVTYPER10_EL0", at(3, 3, 13, 14, 0), A::ReadTrapped),
        ("AMEVTYPER11_EL0", at(3, 3, 13, 14, 1), A::ReadTrapped),
        ("AMEVTYPER12_EL0", at(3, 3, 13, 14, 2), A::ReadTrapped),
        // One of the ID registers, which FEAT_IDST traps as it does every other EL0 cannot read.
        ("MIDR_EL1", at(3, 0, 0, 0, 0), A::ReadTrapped),
        // An EL1 register, and SVCR and TPIDR2_EL0 of SME, which this chip lacks: undefined, as every encoding the
        // table leaves out is.
        ("SCTLR_EL1", at(3, 0, 1, 0, 0), A::Undefined),
        ("SVCR", at(3, 3, 4, 2, 2), A::Undefined),
    ]
};

/// The registers that give a new random number each read: [RNDR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/RNDR--Random-Number) and [RNDRRS](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/RNDRRS--Reseeded-Random-Number).
pub const RANDOM_NUMBER_REGISTERS: [SystemRegisterEncoding; 2] = [
    SystemRegisterEncoding::new(3, 3, 2, 4, 0),
    SystemRegisterEncoding::new(3, 3, 2, 4, 1),
];

/// The ID registers' space, op0 3, op1 0, 1 or 3, CRn 0, CRm 0 to 7: [FEAT_IDST](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-4-architecture-extension#:~:text=FEAT_IDST) traps a
/// read at EL0 where it reaches no register EL0 may read, and a write there is undefined; with CRm 8 to 15 an access
/// is undefined, as a Neoverse V2 shows on each of them.
fn is_identification_register(encoding: SystemRegisterEncoding) -> bool {
    const ID_OP0: u32 = 3;
    const ID_OP1: [u32; 3] = [0, 1, 3];
    /// The first CRm past the ID space.
    const ID_CRM_END: u32 = 8;
    encoding.op0 == ID_OP0
        && (encoding.op1 == ID_OP1[0]
            || encoding.op1 == ID_OP1[1]
            || encoding.op1 == ID_OP1[2]
            || (crate::planted::is(429) && encoding.op1 == 2))
        && encoding.crn == 0
        && (encoding.crm < ID_CRM_END || crate::planted::is(507))
}

/// What an access to the register of the 15-bit field does.
pub fn to_system_register_access(field: u32) -> SystemRegisterAccess {
    let encoding = SystemRegisterEncoding::from_field(field);
    if let Some((_, _, access)) = SYSTEM_REGISTERS
        .iter()
        .find(|(_, listed, _)| *listed == encoding)
    {
        return *access;
    }
    if RANDOM_NUMBER_REGISTERS.contains(&encoding) {
        return SystemRegisterAccess::RandomNumber;
    }
    if is_identification_register(encoding) {
        SystemRegisterAccess::ReadTrapped
    } else {
        SystemRegisterAccess::Undefined
    }
}

/// What a chip or its executor fixes about the system registers EL0 reaches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemRegisterChoices {
    pub cache_type: u64,
    pub zero_block_size: u64,
    pub read_only_thread_id: u64,
    /// The bits of FPCR and FPSR the chip holds; a write leaves the others zero.
    pub floating_point_control_bits: u64,
    pub floating_point_status_bits: u64,
    /// Whether the chip has the random number registers, [FEAT_RNG](https://developer.arm.com/documentation/109697/2025_09/Feature-descriptions/The-Armv8-5-architecture-extension#:~:text=FEAT_RNG); without them an access is undefined.
    pub random_number_registers: bool,
}

/// What a Neoverse V2 under KVM shows, MEASURED by the measured set's `system_registers` survey on a Graviton4: its
/// cache type and zero block size, the TPIDRRO_EL0 its guest holds, and FPCR's AHP, DN, FZ, RMode and FZ16 and FPSR's
/// N, Z, C, V, QC and cumulative flags; and the random number registers, MEASURED by the whole-set run.
pub const NEOVERSE_V2_SYSTEM_REGISTERS: SystemRegisterChoices = SystemRegisterChoices {
    cache_type: 0xB444_C004,
    zero_block_size: 0x4,
    read_only_thread_id: 0x1DE7_EC7E_DBAD_C0DE,
    floating_point_control_bits: 0x07C8_0000,
    floating_point_status_bits: 0xF800_009F,
    random_number_registers: true,
};

/// Another chip's choices, every value different, so that each is told apart.
pub const OTHER_SYSTEM_REGISTERS: SystemRegisterChoices = SystemRegisterChoices {
    cache_type: 0,
    zero_block_size: 0,
    read_only_thread_id: 0,
    floating_point_control_bits: u64::MAX,
    floating_point_status_bits: u64::MAX,
    random_number_registers: false,
};

/// [MRS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MRS--Move-system-register-to-general-purpose-register-)
/// and [MSR (register)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MSR--register---Move-general-purpose-register-to-system-register-).
pub fn simulate_system_register_move<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    direction: AccessDirection,
    choices: SystemRegisterChoices,
) -> Result<(), Exception> {
    use SystemRegisterAccess as A;
    let field = SYSTEM_REGISTER.read(word);
    let number = RT.read(word);
    let trapped = || {
        let encoding = SystemRegisterEncoding::from_field(field);
        Exception::SystemAccessTrapped(SystemAccess {
            op0: encoding.op0 as u8,
            op1: encoding.op1 as u8,
            crn: encoding.crn as u8,
            crm: encoding.crm as u8,
            op2: encoding.op2 as u8,
            rt: number as u8,
            direction,
        })
    };
    let access = to_system_register_access(field);
    let read = match (access, direction) {
        (A::RandomNumber, AccessDirection::Write) if crate::planted::is(510) => return Ok(()),
        (A::Undefined, _)
        | (
            A::Fixed(_) | A::ReadOnlyZero | A::ReadTrapped | A::RandomNumber,
            AccessDirection::Write,
        ) => {
            return Err(Exception::UndefinedInstruction);
        }
        (A::RandomNumber, AccessDirection::Read)
            if !choices.random_number_registers || crate::planted::is(509) =>
        {
            return Err(Exception::UndefinedInstruction);
        }
        // The value and NZCV are undecided; the expectation compares neither.
        (A::RandomNumber, AccessDirection::Read) => {
            processor.condition_flags = ConditionFlags::empty();
            0
        }
        (A::Trapped, _) | (A::ReadTrapped, AccessDirection::Read) => return Err(trapped()),
        (A::ReadsZero | A::ReadOnlyZero, AccessDirection::Read) => 0,
        (A::ReadsZero, AccessDirection::Write) => return Ok(()),
        (A::Fixed(value), AccessDirection::Read) => match value {
            FixedValue::CacheType => choices.cache_type,
            FixedValue::ZeroBlockSize => choices.zero_block_size,
            FixedValue::ReadOnlyThreadId => choices.read_only_thread_id,
        },
        (A::Held(register), AccessDirection::Read) => match register {
            HeldRegister::ConditionFlags if crate::planted::is(428) => {
                u64::from(processor.condition_flags.bits() >> 28)
            }
            HeldRegister::ConditionFlags => u64::from(processor.condition_flags.bits()),
            HeldRegister::FloatingPointControl => processor.floating_point_control,
            HeldRegister::FloatingPointStatus => processor.floating_point_status,
            HeldRegister::SoftwareThreadId => processor.software_thread_id,
        },
        (A::Held(register), AccessDirection::Write) => {
            let value = read_register(processor, number, RegisterThirtyOne::ZeroRegister);
            match register {
                HeldRegister::ConditionFlags => {
                    processor.condition_flags = ConditionFlags::from_bits_truncate(value as u32);
                }
                HeldRegister::FloatingPointControl => {
                    processor.floating_point_control = value & choices.floating_point_control_bits;
                }
                HeldRegister::FloatingPointStatus => {
                    processor.floating_point_status = if crate::planted::is(430) {
                        value
                    } else {
                        value & choices.floating_point_status_bits
                    };
                }
                HeldRegister::SoftwareThreadId if crate::planted::is(431) => {
                    processor.software_thread_id = value & 0xFFFF_FFFF;
                }
                HeldRegister::SoftwareThreadId => processor.software_thread_id = value,
            }
            return Ok(());
        }
    };
    write_register(processor, number, read, RegisterThirtyOne::ZeroRegister);
    Ok(())
}

/// The register a completed MRS or MSR leaves undecided, and whether it leaves NZCV undecided: a read of a random number
/// register gives either a random value and NZCV clear, or zero and Z set where no random number came in time.
pub fn to_undecided_registers(
    word: u32,
    direction: AccessDirection,
    choices: SystemRegisterChoices,
) -> Option<(u32, bool)> {
    let random = direction == AccessDirection::Read
        && choices.random_number_registers
        && to_system_register_access(SYSTEM_REGISTER.read(word))
            == SystemRegisterAccess::RandomNumber;
    (random && !crate::planted::is(511)).then(|| (RT.read(word), true))
}
