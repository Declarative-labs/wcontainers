//! Our own instruction type. Meanings are written against it, never against the decoder's types, so the
//! decoder can be replaced.

/// Operand width of an integer instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    W32,
    X64,
}

impl Width {
    pub fn bits(self) -> u32 {
        match self {
            Width::W32 => 32,
            Width::X64 => 64,
        }
    }
}

/// A general register operand, with register number 31 already resolved to what the encoding means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    /// x0 to x30.
    X(u8),
    Zero,
    Sp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

/// How a register operand is widened before use: unsigned or signed, from 8, 16, 32 or 64 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extend {
    pub signed: bool,
    pub from_bits: u8,
}

/// The second source of an arithmetic or logical instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    /// Already shifted into place.
    Imm(u64),
    Shifted {
        rm: Reg,
        shift: Shift,
        amount: u8,
    },
    Extended {
        rm: Reg,
        extend: Extend,
        amount: u8,
    },
}

/// An ARM64 condition code, 0 to 15 (eq, ne, cs, cc, mi, pl, vs, vc, hi, ls, ge, lt, gt, le, al, nv).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cond(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logic {
    And,
    Or,
    Xor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveWide {
    /// movz
    Zero,
    /// movn
    Not,
    /// movk
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bitfield {
    /// sbfm
    Signed,
    /// bfm
    Insert,
    /// ubfm
    Unsigned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// csel
    Plain,
    /// csinc
    Increment,
    /// csinv
    Invert,
    /// csneg
    Negate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Rbit,
    Rev16,
    Rev32,
    Rev,
    Clz,
    Cls,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary {
    Udiv,
    Sdiv,
    Lslv,
    Lsrv,
    Asrv,
    Rorv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulAdd {
    Madd,
    Msub,
    Smaddl,
    Smsubl,
    Umaddl,
    Umsubl,
    Smulh,
    Umulh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchRegister {
    Br,
    Blr,
    Ret,
}

/// Where a load or store goes. Offsets are in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    /// base + offset
    Offset { base: Reg, offset: i64 },
    /// base += offset, then use base
    PreIndex { base: Reg, offset: i64 },
    /// use base, then base += offset
    PostIndex { base: Reg, offset: i64 },
    /// base + (extend(index) << shift)
    Indexed {
        base: Reg,
        index: Reg,
        extend: Extend,
        shift: u8,
    },
    /// this instruction's address + offset
    Literal { offset: i64 },
}

/// A system register that unprivileged code may read or write and that we answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemRegister {
    Nzcv,
    TpidrEl0,
}

/// One decoded instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
    AddSub {
        width: Width,
        subtract: bool,
        set_flags: bool,
        rd: Reg,
        rn: Reg,
        operand: Operand,
    },
    AddSubCarry {
        width: Width,
        subtract: bool,
        set_flags: bool,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    Logical {
        width: Width,
        op: Logic,
        /// The second source is inverted first (bic, orn, eon).
        invert: bool,
        set_flags: bool,
        rd: Reg,
        rn: Reg,
        operand: Operand,
    },
    MoveWide {
        width: Width,
        op: MoveWide,
        rd: Reg,
        imm: u16,
        shift: u8,
    },
    Bitfield {
        width: Width,
        op: Bitfield,
        rd: Reg,
        rn: Reg,
        immr: u8,
        imms: u8,
    },
    Extract {
        width: Width,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        lsb: u8,
    },
    /// adr, or adrp when `page` is set.
    PcRelative {
        rd: Reg,
        page: bool,
        offset: i64,
    },
    /// ccmp, or ccmn when `negate` is set. `operand` is an immediate or a plain register.
    CondCompare {
        width: Width,
        negate: bool,
        rn: Reg,
        operand: Operand,
        nzcv: u8,
        cond: Cond,
    },
    CondSelect {
        width: Width,
        op: Select,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        cond: Cond,
    },
    Unary {
        width: Width,
        op: Unary,
        rd: Reg,
        rn: Reg,
    },
    Binary {
        width: Width,
        op: Binary,
        rd: Reg,
        rn: Reg,
        rm: Reg,
    },
    /// The long and high forms are always 64 bit; `width` is the width of the plain forms.
    MulAdd {
        width: Width,
        op: MulAdd,
        rd: Reg,
        rn: Reg,
        rm: Reg,
        ra: Reg,
    },
    /// b, or bl when `link` is set.
    Branch {
        link: bool,
        offset: i64,
    },
    BranchCond {
        cond: Cond,
        offset: i64,
    },
    CompareBranch {
        width: Width,
        nonzero: bool,
        rt: Reg,
        offset: i64,
    },
    TestBranch {
        nonzero: bool,
        rt: Reg,
        bit: u8,
        offset: i64,
    },
    BranchRegister {
        op: BranchRegister,
        rn: Reg,
    },
    /// Integer load of `bytes` bytes. `signed` names the width the value is sign extended to; otherwise
    /// it is zero extended.
    Load {
        bytes: u8,
        signed: Option<Width>,
        rt: Reg,
        address: Address,
    },
    Store {
        bytes: u8,
        rt: Reg,
        address: Address,
    },
    /// Two loads of `bytes` each from consecutive addresses; ldpsw sign extends.
    LoadPair {
        bytes: u8,
        signed: bool,
        rt: Reg,
        rt2: Reg,
        address: Address,
    },
    StorePair {
        bytes: u8,
        rt: Reg,
        rt2: Reg,
        address: Address,
    },
    /// svc
    Syscall,
    /// brk, with its comment field.
    Breakpoint {
        comment: u16,
    },
    /// Hints, barriers and prefetches: nothing happens in a single thread.
    Nothing,
    ReadSystem {
        register: SystemRegister,
        rt: Reg,
    },
    WriteSystem {
        register: SystemRegister,
        rt: Reg,
    },
    /// Not an instruction of the CPU we present.
    Undefined,
    /// An instruction of the CPU we present that is not translated yet.
    Unsupported,
}
