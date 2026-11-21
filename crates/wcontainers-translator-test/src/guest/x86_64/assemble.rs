//! x86-64's layer 2 code: the model's programs assembled with iced's `CodeAssembler`. rbp holds the area,
//! r15 counts loops, r14 recursion, r13 holds a moving base or a jump target, and r12 is each operation's
//! scratch. Beside the model's operations it uses what x86 has: read-change-write memory operands, string
//! instructions with and without a repeat prefix, push and pop of registers and memory, calls and jumps
//! through memory, partial register and flag writes, red-zone saves in leaf functions, both compilers'
//! position-independent switch tables, and `pushfq` for the dump.

use crate::guest::GuestTest;
use crate::layer2::*;
use crate::programs::{Blob, LOAD, STACK_BYTES};
use iced_x86::code_asm::*;
use iced_x86::{BlockEncoderOptions, ConditionCode};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_x86::{R12, R13, R14, R15, RAX, RBX, RDX, X86_64};

type Asm = Result<(), IcedError>;

const REGISTERS: &[u8] = <X86_64 as GuestTest>::MODEL.registers;
const OUTPUT: u32 = output::<X86_64>();
const AREA_BYTES: usize = area_bytes::<X86_64>();
const WRITE: u64 = <X86_64 as GuestTest>::STAND_IN.write;
const EXIT_GROUP: u64 = <X86_64 as GuestTest>::STAND_IN.exit_group;
/// The descriptor the model's system call writes to.
const STANDARD_OUTPUT: u32 = 1;

/// One in this many of the model's moves of a constant writes an address in the code instead (`mov` of a
/// 32-bit immediate, or `push` and `pop`), which the translator, given the link address, follows as an entry.
const CODE_ADDRESS_EVERY: u16 = 4;
/// What such an immediate holds until the code is placed: wide enough that iced encodes 32 bits.
const PLACEHOLDER: u32 = i32::MAX as u32;

/// A walk's string forms by its `rt` (`string_step`): one element; then with a repeat prefix `lods` or
/// `stos`; `scas`, or `movs` onto the next element; and `cmps` against it, or `movs` from it.
const STRING_FORMS: u8 = 4;
const ONE_ELEMENT: u8 = 0;
const REPEATED: u8 = 1;
const SCAN_OR_COPY_ONTO_NEXT: u8 = 2;

/// Bytes of a general register, of an area slot and of a stack push.
pub(super) const WORD: i32 = wcontainers_abi::WORD as i32;
/// Bytes of the model's double-word accesses, and of a pair of general registers.
const DOUBLE: u8 = size_of::<u32>() as u8;
const PAIR: u8 = 2 * WORD as u8;
/// Bytes of a vector register.
const VECTOR: i32 = (u128::BITS / u8::BITS) as i32;
/// Pairs of vector registers an access through sp may start at, each `VECTOR` bytes up from `STACK_SLOTS`.
const PAIR_SLOTS: u8 = ((FRAME_FLOOR - STACK_SLOTS) as i32 / VECTOR - 1) as u8;
/// Registers each function saves above its frame: the kept base, the loop counter and the moving base.
const SAVED: [u8; 3] = [RBX, R15, R13];

const GPR64: &[AsmRegister64] = &[
    rax, rcx, rdx, rbx, rsp, rbp, rsi, rdi, r8, r9, r10, r11, r12, r13, r14, r15,
];
const GPR32: &[AsmRegister32] = &[
    eax, ecx, edx, ebx, esp, ebp, esi, edi, r8d, r9d, r10d, r11d, r12d, r13d, r14d, r15d,
];
const GPR16: &[AsmRegister16] = &[
    ax, cx, dx, bx, sp, bp, si, di, r8w, r9w, r10w, r11w, r12w, r13w, r14w, r15w,
];
const GPR8: &[AsmRegister8] = &[
    al, cl, dl, bl, spl, bpl, sil, dil, r8b, r9b, r10b, r11b, r12b, r13b, r14b, r15b,
];
/// The second bytes, of the first four registers.
const HIGH8: &[AsmRegister8] = &[ah, ch, dh, bh];
const XMM: &[AsmRegisterXmm] = &[
    xmm0, xmm1, xmm2, xmm3, xmm4, xmm5, xmm6, xmm7, xmm8, xmm9, xmm10, xmm11, xmm12, xmm13, xmm14,
    xmm15,
];

fn q(n: u8) -> AsmRegister64 {
    GPR64[usize::from(n)]
}

fn d(n: u8) -> AsmRegister32 {
    GPR32[usize::from(n)]
}

fn x(n: u8) -> AsmRegisterXmm {
    XMM[usize::from(n) % XMM.len()]
}

/// Run `$body` with `$r` naming a register by number at 64 bits when `$wide`, else at 32.
macro_rules! sized {
    ($wide:expr, $r:ident => $body:expr) => {
        if $wide {
            let $r = q;
            $body
        } else {
            let $r = d;
            $body
        }
    };
}

type Jump = fn(&mut CodeAssembler, CodeLabel) -> Asm;
type Move64 = fn(&mut CodeAssembler, AsmRegister64, AsmRegister64) -> Asm;
type Move32 = fn(&mut CodeAssembler, AsmRegister32, AsmRegister32) -> Asm;
type Set = fn(&mut CodeAssembler, AsmRegister8) -> Asm;

/// The conditions in the order of their encoding, as iced's `ConditionCode` from `o` on.
const JCC: &[Jump] = &[
    CodeAssembler::jo,
    CodeAssembler::jno,
    CodeAssembler::jb,
    CodeAssembler::jae,
    CodeAssembler::je,
    CodeAssembler::jne,
    CodeAssembler::jbe,
    CodeAssembler::ja,
    CodeAssembler::js,
    CodeAssembler::jns,
    CodeAssembler::jp,
    CodeAssembler::jnp,
    CodeAssembler::jl,
    CodeAssembler::jge,
    CodeAssembler::jle,
    CodeAssembler::jg,
];
const CMOV64: &[Move64] = &[
    CodeAssembler::cmovo,
    CodeAssembler::cmovno,
    CodeAssembler::cmovb,
    CodeAssembler::cmovae,
    CodeAssembler::cmove,
    CodeAssembler::cmovne,
    CodeAssembler::cmovbe,
    CodeAssembler::cmova,
    CodeAssembler::cmovs,
    CodeAssembler::cmovns,
    CodeAssembler::cmovp,
    CodeAssembler::cmovnp,
    CodeAssembler::cmovl,
    CodeAssembler::cmovge,
    CodeAssembler::cmovle,
    CodeAssembler::cmovg,
];
const CMOV32: &[Move32] = &[
    CodeAssembler::cmovo,
    CodeAssembler::cmovno,
    CodeAssembler::cmovb,
    CodeAssembler::cmovae,
    CodeAssembler::cmove,
    CodeAssembler::cmovne,
    CodeAssembler::cmovbe,
    CodeAssembler::cmova,
    CodeAssembler::cmovs,
    CodeAssembler::cmovns,
    CodeAssembler::cmovp,
    CodeAssembler::cmovnp,
    CodeAssembler::cmovl,
    CodeAssembler::cmovge,
    CodeAssembler::cmovle,
    CodeAssembler::cmovg,
];
const SETCC: &[Set] = &[
    CodeAssembler::seto,
    CodeAssembler::setno,
    CodeAssembler::setb,
    CodeAssembler::setae,
    CodeAssembler::sete,
    CodeAssembler::setne,
    CodeAssembler::setbe,
    CodeAssembler::seta,
    CodeAssembler::sets,
    CodeAssembler::setns,
    CodeAssembler::setp,
    CodeAssembler::setnp,
    CodeAssembler::setl,
    CodeAssembler::setge,
    CodeAssembler::setle,
    CodeAssembler::setg,
];

/// A condition's place in the tables above.
const fn cc(c: ConditionCode) -> usize {
    c as usize - ConditionCode::o as usize
}

/// The model draws ARM64's fourteen conditions; adding a value of the same operation reaches all sixteen of
/// x86's.
fn condition(drawn: u8, salt: u8) -> usize {
    (usize::from(drawn) + usize::from(salt)) % JCC.len()
}

/// A walk's ordered conditions going up (below, below or equal, less, less or equal) and down.
const UP: [ConditionCode; 4] = [
    ConditionCode::b,
    ConditionCode::be,
    ConditionCode::l,
    ConditionCode::le,
];
const DOWN: [ConditionCode; 4] = [
    ConditionCode::a,
    ConditionCode::ae,
    ConditionCode::g,
    ConditionCode::ge,
];
/// Where a walk's counter starts going up, and stops going down, so the bound is not 0.
const UP_FROM: u32 = 3;
const DOWN_TO: u32 = 5;

/// The model's arithmetic kinds, as x86 does them.
#[derive(Clone, Copy)]
enum Arithmetic {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Adc,
    Sbb,
    /// Flags only.
    Test,
    Shl,
    Shr,
    Sar,
    /// ror, rol, rcr or rcl, by the first source register.
    Rotate,
    Imul,
    /// shl, shr, sar or a rotation of the first source by an immediate, 0 to 64.
    ShiftImmediate,
    /// bt, bts, btr or btc of the destination by the second source, chosen by the first.
    BitTest,
    /// clc, stc or cmc, chosen by the first source; no register changes.
    Carry,
}

const ARITHMETIC: &[Arithmetic] = &[
    Arithmetic::Add,
    Arithmetic::Sub,
    Arithmetic::And,
    Arithmetic::Or,
    Arithmetic::Xor,
    Arithmetic::Adc,
    Arithmetic::Sbb,
    Arithmetic::Test,
    Arithmetic::Shl,
    Arithmetic::Shr,
    Arithmetic::Sar,
    Arithmetic::Rotate,
    Arithmetic::Imul,
    Arithmetic::ShiftImmediate,
    Arithmetic::BitTest,
    Arithmetic::Carry,
];

/// What a shift by an immediate does: a shift, or the rotation the second source chooses.
const BY_IMMEDIATE: &[Arithmetic] = &[
    Arithmetic::Shl,
    Arithmetic::Shr,
    Arithmetic::Sar,
    Arithmetic::Rotate,
];

#[derive(Clone, Copy)]
enum BitTest {
    Bt,
    Bts,
    Btr,
    Btc,
}

const BIT_TESTS: &[BitTest] = &[BitTest::Bt, BitTest::Bts, BitTest::Btr, BitTest::Btc];

/// clc, stc and cmc.
const CARRY: &[fn(&mut CodeAssembler) -> Asm] =
    &[CodeAssembler::clc, CodeAssembler::stc, CodeAssembler::cmc];

#[derive(Clone, Copy)]
enum Rotation {
    Ror,
    Rol,
    Rcr,
    Rcl,
}

const ROTATES: &[Rotation] = &[Rotation::Ror, Rotation::Rol, Rotation::Rcr, Rotation::Rcl];

/// What a select's second value is: the register, one more, its complement or its negation.
#[derive(Clone, Copy)]
enum Second {
    Same,
    Increment,
    Invert,
    Negate,
}

const SECONDS: &[Second] = &[
    Second::Same,
    Second::Increment,
    Second::Invert,
    Second::Negate,
];

/// What a keeping move writes: a word, the low byte, the second byte (of the first four registers, else the
/// low byte), or a double word, which clears the top.
#[derive(Clone, Copy)]
enum Partial {
    Word,
    Low,
    High,
    Double,
}

const PARTIALS: &[Partial] = &[Partial::Word, Partial::Low, Partial::High, Partial::Double];

/// How a store to the area changes it: a plain move, or read-change-write.
#[derive(Clone, Copy)]
enum Store {
    Move,
    Add,
    Xor,
    Sub,
}

const STORES: &[Store] = &[Store::Move, Store::Add, Store::Move, Store::Xor, Store::Sub];

/// One flag-setting add or subtract of an immediate in this many becomes inc or dec, which keep CF.
const PARTIAL_FLAGS_EVERY: u16 = 4;

/// A label of ours: iced's, and the index of the instruction it was placed on.
#[derive(Clone, Copy)]
struct Label(usize);

/// What a table entry holds: the address of a label, or its distance from the table's start as 32 bits.
#[derive(Clone, Copy)]
enum Entry {
    Absolute(Label),
    Relative(Label),
}

struct Table {
    label: Label,
    bytes: Vec<u8>,
    entries: Vec<(usize, Entry)>,
}

struct Emitter {
    a: CodeAssembler,
    labels: Vec<(CodeLabel, Option<usize>)>,
    functions: Vec<Label>,
    writer: Label,
    area: Label,
    /// The last byte of the blob, which a page that allows nothing follows.
    last: Label,
    data: Label,
    tables: Vec<Table>,
    /// Instructions whose 32-bit immediate becomes an address in the code once it is placed: the index of
    /// each, and the drawn number that picks the address.
    code_addresses: Vec<(usize, u64)>,
}

impl Emitter {
    fn label(&mut self) -> Label {
        self.labels.push((self.a.create_label(), None));
        Label(self.labels.len() - 1)
    }

    fn to(&self, l: Label) -> CodeLabel {
        self.labels[l.0].0
    }

    /// Place a label at the next instruction; a nop keeps two labels apart, as iced allows one per instruction.
    fn place(&mut self, l: Label) -> Asm {
        let next = self.a.instructions().len();
        if self.labels.iter().any(|&(_, at)| at == Some(next)) {
            self.a.nop()?;
        }
        let (mut label, _) = self.labels[l.0];
        self.a.set_label(&mut label)?;
        self.labels[l.0] = (label, Some(self.a.instructions().len()));
        Ok(())
    }

    fn jcc(&mut self, c: usize, to: Label) -> Asm {
        let to = self.to(to);
        JCC[c](&mut self.a, to)
    }

    fn jmp(&mut self, to: Label) -> Asm {
        let to = self.to(to);
        self.a.jmp(to)
    }

    fn call(&mut self, to: Label) -> Asm {
        let to = self.to(to);
        self.a.call(to)
    }

    /// `lea r, [rip + l]`.
    fn address(&mut self, r: u8, l: Label) -> Asm {
        let to = self.to(l);
        self.a.lea(q(r), ptr(to))
    }

    /// A table of zeros now, its entries filled in once the code is placed.
    fn table(&mut self, bytes: Vec<u8>, entries: Vec<(usize, Entry)>) -> Label {
        let label = self.label();
        self.tables.push(Table {
            label,
            bytes,
            entries,
        });
        label
    }

    /// A table of one absolute address.
    fn absolute(&mut self, target: Label) -> Label {
        self.table(vec![0; WORD as usize], vec![(0, Entry::Absolute(target))])
    }

    /// A position-independent switch table: each target's distance from the table, in 32 bits.
    fn relative(&mut self, targets: &[Label]) -> Label {
        let entry = size_of::<i32>();
        let entries = (0..targets.len())
            .map(|k| (k * entry, Entry::Relative(targets[k])))
            .collect();
        self.table(vec![0; targets.len() * entry], entries)
    }

    fn target(&self, callee: Callee) -> Label {
        match callee {
            Callee::Function(f) => self.functions[f],
            Callee::Writer => self.writer,
        }
    }

    /// Point `r` at a place in the area or before the end of the blob.
    fn point(&mut self, r: u8, start: GroupBase) -> Asm {
        match start {
            GroupBase::Area(at) => self.a.lea(q(r), rbp + i32::from(at)),
            GroupBase::Edge(by) => {
                self.address(r, self.last)?;
                self.a.lea(q(r), q(r) + (1 - i32::from(by)))
            }
        }
    }

    fn syscall(&mut self) -> Asm {
        self.a.mov(edi, STANDARD_OUTPUT)?;
        self.a.mov(rsi, rbp)?;
        self.a.xor(edx, edx)?;
        self.a.mov(eax, WRITE as u32)?;
        self.a.syscall()
    }

    fn op(&mut self, op: &Op) -> Asm {
        let t = R12;
        match *op {
            Op::Arithmetic {
                kind,
                wide,
                rd,
                rn,
                rm,
            } => {
                let kind = ARITHMETIC[usize::from(kind) % ARITHMETIC.len()];
                self.arithmetic(kind, wide, rd, rn, rm)?;
            }
            Op::AddImmediate {
                subtract,
                flags,
                wide,
                rd,
                rn,
                imm,
            } => {
                let by = if subtract {
                    -i32::from(imm)
                } else {
                    i32::from(imm)
                };
                if !flags {
                    sized!(wide, r => self.a.lea(r(rd), q(rn) + by))?;
                } else {
                    if rd != rn {
                        sized!(wide, r => self.a.mov(r(rd), r(rn)))?;
                    }
                    match (imm % PARTIAL_FLAGS_EVERY == 0, subtract) {
                        (true, false) => sized!(wide, r => self.a.inc(r(rd)))?,
                        (true, true) => sized!(wide, r => self.a.dec(r(rd)))?,
                        (false, false) => sized!(wide, r => self.a.add(r(rd), i32::from(imm)))?,
                        (false, true) => sized!(wide, r => self.a.sub(r(rd), i32::from(imm)))?,
                    }
                }
            }
            Op::Move { keep, rd, imm, hw } => {
                let shift = u32::from(hw) * u16::BITS;
                if !keep && imm % CODE_ADDRESS_EVERY == 0 {
                    // A constant that only looks like an address, at any byte of the code.
                    self.code_addresses.push((
                        self.a.instructions().len(),
                        u64::from(imm / CODE_ADDRESS_EVERY),
                    ));
                    if rd % 2 == 0 {
                        self.a.mov(d(rd), PLACEHOLDER)?;
                    } else {
                        self.a.push(PLACEHOLDER as i32)?;
                        self.a.pop(q(rd))?;
                    }
                } else if !keep {
                    let value = u64::from(imm) << shift;
                    match u32::try_from(value) {
                        Ok(small) => self.a.mov(d(rd), small)?,
                        Err(_) => self.a.mov(q(rd), value)?,
                    }
                } else {
                    match PARTIALS[usize::from(hw) % PARTIALS.len()] {
                        Partial::Word => self.a.mov(GPR16[usize::from(rd)], u32::from(imm))?,
                        Partial::High if usize::from(rd) < HIGH8.len() => {
                            self.a.mov(HIGH8[usize::from(rd)], u32::from(imm as u8))?
                        }
                        Partial::Low | Partial::High => {
                            self.a.mov(GPR8[usize::from(rd)], u32::from(imm as u8))?
                        }
                        Partial::Double => self.a.mov(d(rd), u32::from(imm) << u16::BITS)?,
                    }
                }
            }
            Op::Select {
                kind,
                wide,
                rd,
                rn,
                rm,
                cond,
            } => {
                let c = condition(cond, rd);
                let second = SECONDS[usize::from(kind) % SECONDS.len()];
                if matches!(second, Second::Same) && rn == rm {
                    SETCC[c](&mut self.a, GPR8[usize::from(rd)])?;
                } else {
                    sized!(wide, r => self.a.mov(r(t), r(rm)))?;
                    match second {
                        Second::Same => {}
                        Second::Increment => sized!(wide, r => self.a.lea(r(t), q(t) + 1))?,
                        Second::Invert => sized!(wide, r => self.a.not(r(t)))?,
                        Second::Negate => {
                            sized!(wide, r => self.a.not(r(t)))?;
                            sized!(wide, r => self.a.lea(r(t), q(t) + 1))?;
                        }
                    }
                    if wide {
                        CMOV64[c](&mut self.a, q(t), q(rn))?;
                    } else {
                        CMOV32[c](&mut self.a, d(t), d(rn))?;
                    }
                    sized!(wide, r => self.a.mov(r(rd), r(t)))?;
                }
            }
            Op::Compare { wide, rn, rm } => sized!(wide, r => self.a.cmp(r(rn), r(rm)))?,
            Op::CondCompare {
                wide,
                rn,
                rm,
                nzcv,
                cond,
            } => {
                // Compare when the condition holds, else test against the constant.
                let (compare, done) = (self.label(), self.label());
                self.jcc(condition(cond, rn), compare)?;
                sized!(wide, r => self.a.test(r(rn), i32::from(nzcv)))?;
                self.jmp(done)?;
                self.place(compare)?;
                sized!(wide, r => self.a.cmp(r(rn), r(rm)))?;
                self.place(done)?;
            }
            Op::Divide {
                signed,
                wide,
                rd,
                rn,
                rm,
            } => self.divide(signed, wide, rd, rn, rm)?,
            Op::Flags { write: false, r } => {
                self.a.pushfq()?;
                self.a.pop(q(r))?;
            }
            Op::Flags { write: true, r } => {
                self.a.mov(q(t), q(r))?;
                self.a.neg(q(t))?;
            }
            Op::Load { bytes, rt, slot } => {
                let at = rbp + i32::from(slot) * WORD;
                let odd = slot % 2 == 1;
                match (bytes, odd) {
                    (1, false) => self.a.movzx(d(rt), byte_ptr(at))?,
                    (1, true) => self.a.movsx(q(rt), byte_ptr(at))?,
                    (2, false) => self.a.movzx(d(rt), word_ptr(at))?,
                    (2, true) => self.a.mov(GPR16[usize::from(rt)], word_ptr(at))?,
                    (DOUBLE, false) => self.a.mov(d(rt), dword_ptr(at))?,
                    (DOUBLE, true) => self.a.movsxd(q(rt), dword_ptr(at))?,
                    _ => self.a.mov(q(rt), qword_ptr(at))?,
                }
            }
            Op::Store { bytes, rt, slot } => {
                let at = rbp + i32::from(slot) * WORD;
                let how = STORES[usize::from(slot) % STORES.len()];
                self.store(how, bytes, at, rt)?;
            }
            Op::Pair {
                load,
                rt,
                rt2,
                slot,
            } => {
                let at = i32::from(slot) * WORD;
                // The model makes a pair loaded into one register two by taking the next number, which on
                // x86 may be rsp or rbp: the next of the model's registers instead.
                let rt2 = if REGISTERS.contains(&rt2) {
                    rt2
                } else {
                    *REGISTERS
                        .iter()
                        .cycle()
                        .skip_while(|&&r| r != rt)
                        .nth(1)
                        .unwrap()
                };
                if load {
                    self.a.push(qword_ptr(rbp + (at + WORD)))?;
                    self.a.push(qword_ptr(rbp + at))?;
                    self.a.pop(q(rt))?;
                    self.a.pop(q(rt2))?;
                } else {
                    self.a.push(q(rt2))?;
                    self.a.push(q(rt))?;
                    self.a.pop(qword_ptr(rbp + at))?;
                    self.a.pop(qword_ptr(rbp + (at + WORD)))?;
                }
            }
            Op::Stack {
                load,
                rt,
                slot,
                vector,
            } => match vector {
                StackVector::No => {
                    let at = rsp + (STACK_SLOTS as i32 + i32::from(slot) * WORD);
                    if load {
                        self.a.mov(q(rt), qword_ptr(at))?;
                    } else {
                        self.a.mov(qword_ptr(at), q(rt))?;
                    }
                }
                StackVector::Pair => {
                    let at = STACK_SLOTS as i32 + i32::from(slot % PAIR_SLOTS) * VECTOR;
                    for (k, v) in [rt, rt + 1].into_iter().enumerate() {
                        let m = xmmword_ptr(rsp + (at + k as i32 * VECTOR));
                        if load {
                            self.a.movdqu(x(v), m)?;
                        } else {
                            self.a.movdqu(m, x(v))?;
                        }
                    }
                }
                StackVector::Structure => {
                    // The frame's first bytes, read only.
                    for k in 0..u8::try_from(WORD / 2).unwrap() {
                        let m = xmmword_ptr(rsp + i32::from(k) * VECTOR);
                        self.a.movdqu(x(rt + k), m)?;
                    }
                }
            },
            Op::Group {
                base,
                start,
                ref accesses,
                syscall,
            } => {
                self.point(base, start)?;
                for (i, access) in accesses.iter().enumerate() {
                    if syscall == Some(i) {
                        self.syscall()?;
                    }
                    self.group_access(base, access)?;
                }
                if syscall == Some(accesses.len()) {
                    self.syscall()?;
                }
            }
            Op::Syscall => self.syscall()?,
        }
        Ok(())
    }

    fn ops(&mut self, ops: &[Op]) -> Asm {
        ops.iter().try_for_each(|op| self.op(op))
    }

    fn arithmetic(&mut self, kind: Arithmetic, wide: bool, rd: u8, rn: u8, rm: u8) -> Asm {
        use Arithmetic::*;
        let t = R12;
        match kind {
            Test => return sized!(wide, r => self.a.test(r(rn), r(rm))),
            Carry => return CARRY[usize::from(rn) % CARRY.len()](&mut self.a),
            ShiftImmediate => {
                // Counts of 0, of 1 and above, and of the width, from the two sources.
                let count = u32::from(rn) * u32::from(rm) % (u64::BITS + 1);
                if rd != rn {
                    sized!(wide, r => self.a.mov(r(rd), r(rn)))?;
                }
                let kind = BY_IMMEDIATE[usize::from(rn) % BY_IMMEDIATE.len()];
                let rotate = ROTATES[usize::from(rm) % ROTATES.len()];
                return sized!(wide, r => match (kind, rotate) {
                    (Shl, _) => self.a.shl(r(rd), count),
                    (Shr, _) => self.a.shr(r(rd), count),
                    (Sar, _) => self.a.sar(r(rd), count),
                    (_, Rotation::Ror) => self.a.ror(r(rd), count),
                    (_, Rotation::Rol) => self.a.rol(r(rd), count),
                    (_, Rotation::Rcr) => self.a.rcr(r(rd), count),
                    (_, Rotation::Rcl) => self.a.rcl(r(rd), count),
                });
            }
            BitTest => {
                return sized!(wide, r => match BIT_TESTS[usize::from(rn) % BIT_TESTS.len()] {
                    self::BitTest::Bt => self.a.bt(r(rd), r(rm)),
                    self::BitTest::Bts => self.a.bts(r(rd), r(rm)),
                    self::BitTest::Btr => self.a.btr(r(rd), r(rm)),
                    self::BitTest::Btc => self.a.btc(r(rd), r(rm)),
                });
            }
            Shl | Shr | Sar | Rotate => {
                // The count goes in cl: rcx is swapped with the count's register around the shift.
                let rcx_number = wcontainers_x86::RCX;
                sized!(wide, r => self.a.mov(r(t), r(rn)))?;
                if rm != rcx_number {
                    self.a.xchg(rcx, q(rm))?;
                }
                let rotate = ROTATES[usize::from(rn) % ROTATES.len()];
                sized!(wide, r => match (kind, rotate) {
                    (Shl, _) => self.a.shl(r(t), cl),
                    (Shr, _) => self.a.shr(r(t), cl),
                    (Sar, _) => self.a.sar(r(t), cl),
                    (_, Rotation::Ror) => self.a.ror(r(t), cl),
                    (_, Rotation::Rol) => self.a.rol(r(t), cl),
                    (_, Rotation::Rcr) => self.a.rcr(r(t), cl),
                    (_, Rotation::Rcl) => self.a.rcl(r(t), cl),
                })?;
                if rm != rcx_number {
                    self.a.xchg(rcx, q(rm))?;
                }
                return sized!(wide, r => self.a.mov(r(rd), r(t)));
            }
            _ => {}
        }
        // Two operands: straight into rd when it is the first source, else through it or the scratch.
        let into = if rd == rn {
            rd
        } else if rd != rm {
            sized!(wide, r => self.a.mov(r(rd), r(rn)))?;
            rd
        } else {
            sized!(wide, r => self.a.mov(r(t), r(rn)))?;
            t
        };
        sized!(wide, r => match kind {
            Add => self.a.add(r(into), r(rm)),
            Sub => self.a.sub(r(into), r(rm)),
            And => self.a.and(r(into), r(rm)),
            Or => self.a.or(r(into), r(rm)),
            Xor => self.a.xor(r(into), r(rm)),
            Adc => self.a.adc(r(into), r(rm)),
            Sbb => self.a.sbb(r(into), r(rm)),
            _ => self.a.imul_2(r(into), r(rm)),
        })?;
        if into != rd {
            sized!(wide, r => self.a.mov(r(rd), r(into)))?;
        }
        Ok(())
    }

    /// rd = rn / rm, the divisor made odd (and positive when signed) so the division never traps.
    fn divide(&mut self, signed: bool, wide: bool, rd: u8, rn: u8, rm: u8) -> Asm {
        let t = R12;
        let bits = if wide { u64::BITS } else { u32::BITS };
        sized!(wide, r => self.a.mov(r(t), r(rm)))?;
        sized!(wide, r => self.a.or(r(t), 1))?;
        if signed {
            sized!(wide, r => self.a.btr(r(t), (bits - 1) as i32))?;
        }
        self.a.push(rax)?;
        self.a.push(rdx)?;
        sized!(wide, r => self.a.mov(r(RAX), r(rn)))?;
        match (signed, wide) {
            (true, true) => self.a.cqo()?,
            (true, false) => self.a.cdq()?,
            (false, _) => self.a.xor(d(RDX), d(RDX))?,
        }
        if signed {
            sized!(wide, r => self.a.idiv(r(t)))?;
        } else {
            sized!(wide, r => self.a.div(r(t)))?;
        }
        sized!(wide, r => self.a.mov(r(t), r(RAX)))?;
        self.a.pop(rdx)?;
        self.a.pop(rax)?;
        sized!(wide, r => self.a.mov(r(rd), r(t)))
    }

    fn store(&mut self, how: Store, bytes: u8, at: AsmMemoryOperand, rt: u8) -> Asm {
        let rt = usize::from(rt);
        macro_rules! each {
            ($m:expr, $r:expr) => {
                match how {
                    Store::Move => self.a.mov($m, $r),
                    Store::Add => self.a.add($m, $r),
                    Store::Xor => self.a.xor($m, $r),
                    Store::Sub => self.a.sub($m, $r),
                }
            };
        }
        match bytes {
            1 => each!(byte_ptr(at), GPR8[rt]),
            2 => each!(word_ptr(at), GPR16[rt]),
            DOUBLE => each!(dword_ptr(at), GPR32[rt]),
            _ => each!(qword_ptr(at), GPR64[rt]),
        }
    }

    /// One access of a group through `b`; x86 moves the base by `lea` where ARM64 indexes.
    fn group_access(&mut self, b: u8, access: &GroupAccess) -> Asm {
        let by = i32::from(access.offset);
        let unit = match (access.bytes, access.vector) {
            (_, true) if access.bytes > VECTOR as u8 * 2 => 0,
            (_, true) => VECTOR,
            (PAIR, false) => WORD,
            (bytes, false) => i32::from(bytes),
        };
        let at = match access.indexing {
            Indexing::Offset => by * unit,
            Indexing::Pre => {
                self.a.lea(q(b), q(b) + by)?;
                0
            }
            Indexing::Post => 0,
        };
        let m = q(b) + at;
        if access.vector {
            for k in 0..access.bytes / VECTOR as u8 {
                let m = xmmword_ptr(m + i32::from(k) * VECTOR);
                if access.load {
                    self.a.movdqu(x(access.rt + k), m)?;
                } else {
                    self.a.movdqu(m, x(access.rt + k))?;
                }
            }
        } else {
            let (rt, rt2) = (access.rt, access.rt2);
            match (access.bytes, access.load) {
                (1, true) => self.a.movzx(d(rt), byte_ptr(m))?,
                (2, true) => self.a.movzx(d(rt), word_ptr(m))?,
                (DOUBLE, true) => self.a.mov(d(rt), dword_ptr(m))?,
                (bytes, true) if i32::from(bytes) == WORD => self.a.mov(q(rt), qword_ptr(m))?,
                // Both halves from the base as it was, as the pair's one load reads them.
                (_, true) if rt == b => {
                    self.a.mov(q(rt2), qword_ptr(m + WORD))?;
                    self.a.mov(q(rt), qword_ptr(m))?;
                }
                (_, true) => {
                    self.a.mov(q(rt), qword_ptr(m))?;
                    self.a.mov(q(rt2), qword_ptr(m + WORD))?;
                }
                (PAIR, false) => {
                    self.a.mov(qword_ptr(m), q(rt))?;
                    self.a.mov(qword_ptr(m + WORD), q(rt2))?;
                }
                (bytes, false) => self.store(Store::Move, bytes, m, rt)?,
            }
        }
        if let Indexing::Post = access.indexing {
            self.a.lea(q(b), q(b) + by)?;
        }
        Ok(())
    }

    fn chunks(&mut self, chunks: &[Chunk]) -> Asm {
        for chunk in chunks {
            match chunk {
                Chunk::Ops(list) => self.ops(list)?,
                Chunk::Skip { cond, ops } => {
                    let after = self.label();
                    self.jcc(condition(*cond, ops.len() as u8), after)?;
                    self.ops(ops)?;
                    self.place(after)?;
                }
                Chunk::Loop { count, ops, call } => {
                    let top = self.label();
                    self.a.mov(d(R15), u32::from(*count))?;
                    self.place(top)?;
                    self.ops(ops)?;
                    if let Some(callee) = *call {
                        self.call(self.target(callee))?;
                    }
                    if count % 2 == 1 {
                        self.a.dec(d(R15))?;
                    } else {
                        self.a.sub(d(R15), 1)?;
                    }
                    self.jcc(cc(ConditionCode::ne), top)?;
                }
                Chunk::Walk(w) => self.walk(w)?,
                Chunk::Lookup(l) => self.lookup(l)?,
                Chunk::Call {
                    callee,
                    through_register,
                    around,
                } => {
                    if let Some(a) = around {
                        self.point(RBX, a.start)?;
                        for access in &a.before {
                            self.group_access(RBX, access)?;
                        }
                    }
                    let target = self.target(*callee);
                    match (through_register, around) {
                        (false, _) => self.call(target)?,
                        (true, Some(_)) => {
                            self.address(R13, target)?;
                            self.a.call(r13)?;
                        }
                        (true, None) => {
                            let entry = self.absolute(target);
                            let entry = self.to(entry);
                            self.a.call(qword_ptr(entry))?;
                        }
                    }
                    if let Some(a) = around {
                        for access in &a.after {
                            self.group_access(RBX, access)?;
                        }
                    }
                }
                &Chunk::Below {
                    frame,
                    below,
                    load,
                    rt,
                    between,
                } => {
                    let frame = i32::from(frame);
                    let page_start = !(PAGE_SIZE as i64 - 1) as i32;
                    self.a.mov(r13, rsp)?;
                    self.a.and(rsp, page_start)?;
                    self.a.sub(rsp, frame)?;
                    self.a.mov(qword_ptr(rsp), r13)?;
                    self.a.add(rsp, frame)?;
                    let at = qword_ptr(rsp - i32::from(below));
                    let access = |e: &mut Self| {
                        if load {
                            e.a.mov(q(rt), at)
                        } else {
                            e.a.mov(at, q(rt))
                        }
                    };
                    match between {
                        Between::Nothing => access(self)?,
                        Between::Syscall => {
                            self.syscall()?;
                            access(self)?;
                        }
                        Between::Loop(turns) => {
                            let top = self.label();
                            self.a.mov(d(R15), u32::from(turns))?;
                            self.place(top)?;
                            access(self)?;
                            self.a.sub(d(R15), 1)?;
                            self.jcc(cc(ConditionCode::ne), top)?;
                        }
                    }
                    self.a.mov(rsp, r13)?;
                }
                Chunk::Switch { index, cases } => self.switch(*index, cases)?,
                Chunk::Jump {
                    through_register,
                    ops,
                } => {
                    let after = self.label();
                    match (through_register, ops.len() % 2 == 1) {
                        (false, _) => self.jmp(after)?,
                        (true, false) => {
                            self.address(R13, after)?;
                            self.a.jmp(r13)?;
                        }
                        (true, true) => {
                            let entry = self.absolute(after);
                            let entry = self.to(entry);
                            self.a.jmp(qword_ptr(entry))?;
                        }
                    }
                    self.ops(ops)?;
                    self.place(after)?;
                }
            }
        }
        Ok(())
    }

    /// A switch on the low bits of a register through a position-independent table of 32-bit distances,
    /// reversed so the table is not just the index, after the guard on the 32-bit index the way the compilers
    /// write it for `switch (unsigned)`: gcc's form (the table's address, then `mov t32, t32`, `notrack`) for
    /// an index in an even register, clang's (`mov u32, t32` into the register the entry is loaded into,
    /// then the table's address) for one in an odd.
    fn switch(&mut self, index: u8, cases: &[Vec<Op>]) -> Asm {
        let (t, u) = (R12, R13);
        let n = cases.len() as u32;
        let end = self.label();
        let labels: Vec<Label> = cases.iter().map(|_| self.label()).collect();
        let reversed: Vec<Label> = labels.iter().rev().copied().collect();
        let table = self.relative(&reversed);
        let mask = (n.next_power_of_two() - 1).max(1);
        self.a.mov(d(t), d(index))?;
        self.a.and(d(t), mask)?;
        self.a.cmp(d(t), n - 1)?;
        self.jcc(cc(ConditionCode::a), end)?;
        let entry = |base: u8, at: u8| dword_ptr(q(base) + q(at) * size_of::<i32>() as u32);
        if index.is_multiple_of(2) {
            self.address(u, table)?;
            self.a.mov(d(t), d(t))?;
            self.a.movsxd(q(t), entry(u, t))?;
            self.a.add(q(t), q(u))?;
            self.a.notrack().jmp(q(t))?;
        } else {
            self.a.mov(d(u), d(t))?;
            self.address(t, table)?;
            self.a.movsxd(q(u), entry(t, u))?;
            self.a.add(q(u), q(t))?;
            self.a.jmp(q(u))?;
        }
        for (&label, case) in labels.iter().zip(cases) {
            self.place(label)?;
            self.ops(case)?;
            self.jmp(end)?;
        }
        self.place(end)
    }

    fn walk(&mut self, w: &Walk) -> Asm {
        let (t, base, counter) = (R12, R13, R15);
        let top = self.label();
        self.point(base, w.at)?;
        let step = i32::from(w.step);
        let scale_shift = WORD.trailing_zeros() as i32;
        if let WalkMove::Index {
            reg,
            extend,
            shift,
            start,
            step_in,
            ..
        } = w.moves
        {
            // The base moves back by the index's first value, so the first access is at the chosen place.
            let first = if extend == 2 {
                start as u32 as i32 as u64
            } else {
                start
            };
            self.a.mov(q(reg), first)?;
            self.extended(t, reg, extend)?;
            if shift {
                self.a.shl(q(t), scale_shift)?;
            }
            self.a.sub(q(base), q(t))?;
            if let Some(r) = step_in {
                self.a.mov(q(r), i64::from(step))?;
            }
        }
        let turns = u32::from(w.turns);
        let by = u32::from(w.by);
        let (first, bound) = match w.counter {
            COUNT_DOWN | DECREMENT => (turns, 0),
            COUNT_UP => (0, turns),
            UP_TO_BOUND => (UP_FROM, UP_FROM + turns * by),
            _ => (DOWN_TO + turns * by, DOWN_TO),
        };
        let bound = bound as i32;
        self.a.mov(d(counter), first)?;
        self.place(top)?;
        // A loaded value must not replace the index or the step.
        let rt = match w.moves {
            WalkMove::Index { reg, step_in, .. } if w.load => *REGISTERS
                .iter()
                .cycle()
                .skip_while(|&&r| r != w.rt)
                .find(|&&r| r != reg && Some(r) != step_in)
                .unwrap(),
            _ => w.rt,
        };
        match w.moves {
            WalkMove::Base(Indexing::Post) if !w.vector => self.string_step(w)?,
            WalkMove::Base(indexing) => {
                let access = GroupAccess {
                    load: w.load,
                    bytes: w.bytes,
                    vector: w.vector,
                    rt,
                    rt2: rt,
                    indexing,
                    offset: if matches!(indexing, Indexing::Offset) {
                        0
                    } else {
                        i16::from(w.step)
                    },
                };
                self.group_access(base, &access)?;
                if let Indexing::Offset = indexing {
                    self.a.lea(q(base), q(base) + step)?;
                }
            }
            WalkMove::Index {
                reg,
                extend,
                shift,
                first,
                ..
            } => {
                if first {
                    self.index_step(w)?;
                }
                let index = if extend == 2 {
                    reg
                } else {
                    self.extended(t, reg, extend)?;
                    t
                };
                let scale = if shift { WORD as u32 } else { 1 };
                let m = q(base) + q(index) * scale;
                match (w.vector, w.bytes, w.load) {
                    (true, _, true) => self.a.movdqu(x(rt), xmmword_ptr(m))?,
                    (true, _, false) => self.a.movdqu(xmmword_ptr(m), x(rt))?,
                    (_, 1, true) => self.a.movzx(d(rt), byte_ptr(m))?,
                    (_, 1, false) => self.a.mov(byte_ptr(m), GPR8[usize::from(rt)])?,
                    (_, _, true) => self.a.mov(q(rt), qword_ptr(m))?,
                    (_, _, false) => self.a.mov(qword_ptr(m), q(rt))?,
                }
                if !first {
                    self.index_step(w)?;
                }
            }
        }
        if w.syscall {
            self.syscall()?;
        }
        if let Some(callee) = w.call {
            self.call(self.target(callee))?;
        }
        // The counter: changed, compared and branched on in the form chosen. A comparison before the change
        // needs a change that keeps the flags, so it is a lea.
        let wide = w.wide;
        let by = by as i32;
        match w.counter {
            COUNT_DOWN => {
                sized!(wide, r => self.a.sub(r(counter), 1))?;
                self.jcc(cc(ConditionCode::ne), top)?;
            }
            COUNT_UP => {
                sized!(wide, r => self.a.add(r(counter), 1))?;
                sized!(wide, r => self.a.cmp(r(counter), bound))?;
                self.jcc(cc(ConditionCode::ne), top)?;
            }
            UP_TO_BOUND | DOWN_TO_BOUND => {
                let up = w.counter == UP_TO_BOUND;
                let by = if up { by } else { -by };
                if w.compare_first {
                    sized!(wide, r => self.a.cmp(r(counter), bound))?;
                    sized!(wide, r => self.a.lea(r(counter), q(counter) + by))?;
                } else {
                    sized!(wide, r => self.a.add(r(counter), by))?;
                    sized!(wide, r => self.a.cmp(r(counter), bound))?;
                }
                let conditions = if up { UP } else { DOWN };
                self.jcc(cc(conditions[usize::from(w.cond) % conditions.len()]), top)?;
            }
            _ => {
                sized!(wide, r => self.a.dec(r(counter)))?;
                self.jcc(cc(ConditionCode::ne), top)?;
            }
        }
        Ok(())
    }

    /// `t` = `reg` read as `extend`: 0 zero-extends its low half, 1 sign-extends it, 2 takes all of it.
    fn extended(&mut self, t: u8, reg: u8, extend: u8) -> Asm {
        match extend {
            0 => self.a.mov(d(t), d(reg)),
            1 => self.a.movsxd(q(t), d(reg)),
            _ => self.a.mov(q(t), q(reg)),
        }
    }

    fn index_step(&mut self, w: &Walk) -> Asm {
        if let WalkMove::Index {
            reg, wide, step_in, ..
        } = w.moves
        {
            match step_in {
                Some(r) => sized!(wide, x => self.a.add(x(reg), x(r)))?,
                None => sized!(wide, x => self.a.lea(x(reg), q(reg) + i32::from(w.step)))?,
            }
        }
        Ok(())
    }

    /// A walk that moves its base after each access, by a string instruction (with the direction flag set for
    /// a step down) and a lea for the rest of the step. The walk's `rt`, which no string instruction names,
    /// picks the form: one element (`lods` to rax, `stos` from it), keeping rsi and rdi; or with a repeat
    /// prefix as many elements as the step holds, which leaves rcx, rsi and rdi as it ends: `rep lods`,
    /// `repne scas` against rax and `repe cmps` against the next element for loads; `rep stos`, `rep movs`
    /// from the next element (the bulk pass going up) and `rep movs` onto it (one element at a time) for
    /// stores.
    fn string_step(&mut self, w: &Walk) -> Asm {
        let base = R13;
        let down = w.step < 0;
        let form = w.rt % STRING_FORMS;
        if form == ONE_ELEMENT {
            let pointer = if w.load { rsi } else { rdi };
            self.a.xchg(q(base), pointer)?;
            if down {
                self.a.std()?;
            }
            match (w.load, w.bytes) {
                (true, 1) => self.a.lodsb()?,
                (true, _) => self.a.lodsq()?,
                (false, 1) => self.a.stosb()?,
                (false, _) => self.a.stosq()?,
            }
            if down {
                self.a.cld()?;
            }
            self.a.xchg(q(base), pointer)?;
            let moved = if down {
                -i32::from(w.bytes)
            } else {
                i32::from(w.bytes)
            };
            return self.a.lea(q(base), q(base) + (i32::from(w.step) - moved));
        }
        let bytes = i32::from(w.bytes);
        let in_step = i32::from(w.step).unsigned_abs() / bytes.unsigned_abs();
        // A form with the next element needs a step of two at least, and then reaches one element further.
        let form = if form != REPEATED && in_step < 2 {
            REPEATED
        } else {
            form
        };
        let elements = if form == REPEATED {
            in_step.max(1)
        } else {
            in_step - 1
        };
        // The element after the base, the way the walk goes.
        let next = q(base) + if down { -bytes } else { bytes };
        let wide = w.bytes != 1;
        match (w.load, form) {
            (true, REPEATED) | (false, REPEATED) => {
                self.a.mov(if w.load { rsi } else { rdi }, q(base))?
            }
            (true, SCAN_OR_COPY_ONTO_NEXT) => self.a.mov(rdi, q(base))?,
            (true, _) | (false, SCAN_OR_COPY_ONTO_NEXT) => {
                self.a.mov(rsi, q(base))?;
                self.a.lea(rdi, next)?;
            }
            (false, _) => {
                self.a.mov(rdi, q(base))?;
                self.a.lea(rsi, next)?;
            }
        }
        self.a.mov(ecx, elements)?;
        if down {
            self.a.std()?;
        }
        match (w.load, form, wide) {
            (true, REPEATED, false) => self.a.rep().lodsb()?,
            (true, REPEATED, true) => self.a.rep().lodsq()?,
            (true, SCAN_OR_COPY_ONTO_NEXT, false) => self.a.repne().scasb()?,
            (true, SCAN_OR_COPY_ONTO_NEXT, true) => self.a.repne().scasq()?,
            (true, _, false) => self.a.repe().cmpsb()?,
            (true, _, true) => self.a.repe().cmpsq()?,
            (false, REPEATED, false) => self.a.rep().stosb()?,
            (false, REPEATED, true) => self.a.rep().stosq()?,
            (false, _, false) => self.a.rep().movsb()?,
            (false, _, true) => self.a.rep().movsq()?,
        }
        if down {
            self.a.cld()?;
        }
        self.a.lea(q(base), q(base) + i32::from(w.step))
    }

    fn lookup(&mut self, l: &Lookup) -> Asm {
        let (t, counter) = (R13, R15);
        let (base, index, rt, tab) = (l.base, l.index, l.rt, l.table);
        self.point(base, l.at)?;
        let top = self.label();
        let skip = self.label();
        // The byte table: the switch's two entries, a choice for each turn, an index for each turn.
        let mut bytes = vec![1, 0];
        let choices_at = bytes.len() as i32 - 1;
        bytes.extend(&l.choices);
        let indexes_at = bytes.len() as i32 - 1;
        bytes.extend(&l.values);
        let table = self.table(bytes, Vec::new());
        self.address(tab, table)?;
        self.a.mov(d(index), u32::from(l.start))?;
        self.a.mov(d(counter), u32::from(l.turns))?;
        self.place(top)?;
        if l.loaded {
            self.a
                .movzx(d(index), byte_ptr(q(tab) + q(counter) + indexes_at))?;
        } else {
            self.a.xor(d(index), d(counter))?;
        }
        if let Some((bound, inclusive, wide)) = l.guard {
            sized!(wide, r => self.a.cmp(r(index), i32::from(bound)))?;
            let skip_when = if inclusive {
                ConditionCode::a
            } else {
                ConditionCode::ae
            };
            self.jcc(cc(skip_when), skip)?;
        }
        let at = if l.extend == 2 {
            index
        } else {
            self.extended(R12, index, l.extend)?;
            R12
        };
        let scale = if l.shift { WORD as u32 } else { 1 };
        let m = q(base) + q(at) * scale;
        match (l.bytes, l.load) {
            (1, true) => self.a.movzx(d(rt), byte_ptr(m))?,
            (1, false) => self.a.mov(byte_ptr(m), GPR8[usize::from(rt)])?,
            (_, true) => self.a.mov(q(rt), qword_ptr(m))?,
            (_, false) => self.a.mov(qword_ptr(m), q(rt))?,
        }
        let entries = 2;
        if l.switch {
            // Two cases chosen by the turn's choice through the byte table's first two bytes, then a
            // position-independent table, with the index register holding the table.
            let (one, two) = (self.label(), self.label());
            let cases = self.relative(&[one, two]);
            self.a
                .movzx(d(t), byte_ptr(q(tab) + q(counter) + choices_at))?;
            self.a.cmp(d(t), entries - 1)?;
            self.jcc(cc(ConditionCode::a), skip)?;
            self.a.movzx(d(t), byte_ptr(q(tab) + q(t)))?;
            self.address(index, cases)?;
            self.a
                .movsxd(q(t), dword_ptr(q(index) + q(t) * size_of::<i32>() as u32))?;
            self.a.add(q(t), q(index))?;
            self.a.jmp(q(t))?;
            self.place(one)?;
            self.a.lea(q(rt), q(rt) + 1)?;
            self.jmp(skip)?;
            self.place(two)?;
            self.a.xor(q(rt), q(counter))?;
        }
        self.place(skip)?;
        self.a.sub(q(counter), 1)?;
        if l.back {
            // Entry 0 is the head, entry 1 the way out, chosen by whether the count has reached 0.
            let out = self.label();
            let back = self.relative(&[top, out]);
            self.a.sete(GPR8[usize::from(t)])?;
            self.a.movzx(d(t), GPR8[usize::from(t)])?;
            self.a.cmp(d(t), entries - 1)?;
            self.jcc(cc(ConditionCode::a), out)?;
            self.address(index, back)?;
            self.a
                .movsxd(q(t), dword_ptr(q(index) + q(t) * size_of::<i32>() as u32))?;
            self.a.add(q(t), q(index))?;
            self.a.jmp(q(t))?;
            self.place(out)?;
        } else {
            self.jcc(cc(ConditionCode::ne), top)?;
        }
        Ok(())
    }

    fn function(&mut self, i: usize, function: &Function) -> Asm {
        let again = self.label();
        let done = self.label();
        let saved = (SAVED.len() as i32) * WORD;
        let frame = function.frame as i32;
        // A function that calls nothing keeps the registers in the red zone instead of pushing them.
        let leaf = function.recursion == 0 && !function.body.iter().any(calls);
        self.place(self.functions[i])?;
        self.a.mov(d(R14), u32::from(function.recursion))?;
        self.place(again)?;
        if leaf {
            for (k, &r) in SAVED.iter().enumerate() {
                self.a.mov(qword_ptr(rsp - (k as i32 + 1) * WORD), q(r))?;
            }
            self.a.sub(rsp, frame + saved)?;
        } else {
            for &r in &SAVED {
                self.a.push(q(r))?;
            }
            self.a.sub(rsp, frame)?;
        }
        self.chunks(&function.body)?;
        // Bounded recursion: call itself again while r14 counts down.
        self.a.test(d(R14), d(R14))?;
        self.jcc(cc(ConditionCode::e), done)?;
        self.a.dec(d(R14))?;
        self.call(again)?;
        self.place(done)?;
        if leaf {
            self.a.add(rsp, frame + saved)?;
            for (k, &r) in SAVED.iter().enumerate() {
                self.a.mov(q(r), qword_ptr(rsp - (k as i32 + 1) * WORD))?;
            }
        } else {
            self.a.add(rsp, frame)?;
            for &r in SAVED.iter().rev() {
                self.a.pop(q(r))?;
            }
        }
        match function.tail {
            Some(t) => self.jmp(self.functions[t]),
            None => self.a.ret(),
        }
    }
}

/// Whether a chunk makes a call.
fn calls(chunk: &Chunk) -> bool {
    match chunk {
        Chunk::Loop { call, .. } => call.is_some(),
        Chunk::Walk(w) => w.call.is_some(),
        Chunk::Call { .. } => true,
        _ => false,
    }
}

/// The program's bytes with `pad` bytes before the area, and where its code ends and its area starts.
fn emit(program: &Program, pad: usize) -> Result<(Vec<u8>, u64, u64), IcedError> {
    let mut e = Emitter {
        a: CodeAssembler::new(u64::BITS)?,
        labels: Vec::new(),
        functions: Vec::new(),
        writer: Label(0),
        area: Label(0),
        last: Label(0),
        data: Label(0),
        tables: Vec::new(),
        code_addresses: Vec::new(),
    };
    e.functions = program.functions.iter().map(|_| e.label()).collect();
    e.writer = e.label();
    e.area = e.label();
    e.last = e.label();
    e.data = e.label();
    // The area is in the blob, so it is no function's frame; main still makes a frame of its size for its
    // accesses through sp. The registers start from the program's values.
    e.a.sub(rsp, AREA_BYTES as i32)?;
    e.address(wcontainers_x86::RBP, e.area)?;
    for (&r, &value) in REGISTERS.iter().zip(&program.registers) {
        e.a.mov(q(r), value)?;
    }
    e.chunks(&program.main)?;
    // Store the registers and the flags after the area, write area and registers out, exit 0.
    for (i, &r) in REGISTERS.iter().enumerate() {
        e.a.mov(qword_ptr(rbp + (AREA as i32 + i as i32 * WORD)), q(r))?;
    }
    let flags_at = AREA as i32 + REGISTERS.len() as i32 * WORD;
    e.a.pushfq()?;
    e.a.pop(r12)?;
    e.a.mov(qword_ptr(rbp + flags_at), r12)?;
    e.a.mov(edi, STANDARD_OUTPUT)?;
    e.a.mov(rsi, rbp)?;
    e.a.mov(edx, OUTPUT)?;
    e.a.mov(eax, WRITE as u32)?;
    e.a.syscall()?;
    e.a.xor(edi, edi)?;
    e.a.mov(eax, EXIT_GROUP as u32)?;
    e.a.syscall()?;
    for (i, function) in program.functions.iter().enumerate() {
        e.function(i, function)?;
    }
    e.place(e.writer)?;
    e.syscall()?;
    e.a.ret()?;
    // A word of zeros marks where the code ends, then the tables, zeros up to where the area ends on a page
    // boundary, the area, and a page of zeros, so accesses near the end reach no code, table or area.
    e.place(e.data)?;
    e.a.db(&[0; WORD as usize])?;
    let tables = std::mem::take(&mut e.tables);
    for table in &tables {
        e.place(table.label)?;
        e.a.db(&table.bytes)?;
    }
    if pad > 0 {
        e.a.db(&vec![0; pad])?;
    }
    e.place(e.area)?;
    e.a.db(&vec![0; AREA_BYTES + PAGE_SIZE as usize - 1])?;
    e.place(e.last)?;
    e.a.db(&[0])?;
    let result =
        e.a.assemble_options(LOAD, BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)?;
    let offset = |l: Label| {
        let at = e.labels[l.0].1.expect("every label is placed");
        u64::from(result.inner.new_instruction_offsets[at])
    };
    let mut bytes = result.inner.code_buffer.clone();
    for table in &tables {
        let start = offset(table.label);
        for &(at, entry) in &table.entries {
            let at = start as usize + at;
            match entry {
                Entry::Absolute(target) => {
                    let address = LOAD + offset(target);
                    bytes[at..at + size_of::<u64>()].copy_from_slice(&address.to_le_bytes());
                }
                Entry::Relative(target) => {
                    let distance = (offset(target) as i64 - start as i64) as i32;
                    bytes[at..at + size_of::<i32>()].copy_from_slice(&distance.to_le_bytes());
                }
            }
        }
    }
    let code_end = offset(e.data);
    for &(index, drawn) in &e.code_addresses {
        let at = result.inner.new_instruction_offsets[index] as usize;
        let mut decoder =
            iced_x86::Decoder::new(u64::BITS, &bytes[at..], iced_x86::DecoderOptions::NONE);
        let instruction = decoder.decode();
        let immediate = at
            + decoder
                .get_constant_offsets(&instruction)
                .immediate_offset();
        let address =
            u32::try_from(LOAD + drawn % code_end).expect("an address the immediate holds");
        bytes[immediate..immediate + size_of::<u32>()].copy_from_slice(&address.to_le_bytes());
    }
    Ok((bytes, code_end, offset(e.area)))
}

/// Assemble a program into a flat blob that starts at its first byte: once to find where its area falls,
/// once more with the area's end moved to a page boundary and its start onto a page of its own (the code
/// before it is the same).
pub(super) fn assemble(program: &Program) -> Blob {
    let page = PAGE_SIZE as usize;
    let (_, _, area) = emit(program, 0).expect("assembles");
    let mut pad = (page - (area as usize + AREA_BYTES) % page) % page;
    // No page holds both the area and code or a table: such a page is locked as translated code, and every
    // store to the area would drop its translations.
    if (area as usize + pad) / page * page < area as usize {
        pad += page;
    }
    let (bytes, code_bytes, area) = emit(program, pad).expect("assembles");
    assert_eq!((area as usize + AREA_BYTES) % page, 0);
    Blob {
        name: BLOB_NAME.into(),
        bytes,
        code_bytes,
        load: LOAD,
        link_address: Some(LOAD),
        main: None,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    }
}

/// The checks test's programs, in the model's terms: a switch of each compiler's form taken through its
/// table, which full must recognise, and a group of accesses failing across the end of the blob on a
/// function's first instruction.
pub(super) fn checks() -> Vec<crate::guest::Check> {
    use crate::guest::{Check, Counted};
    use wcontainers_translator::contract::Reason;
    const SWITCHES: &str = "switches";
    const GROUP_AT_ENTRY: &str = "group-at-entry";
    let cases = |k: usize| -> Vec<Vec<Op>> {
        (0..k)
            .map(|c| {
                if c % 2 == 1 {
                    vec![Op::Syscall]
                } else {
                    Vec::new()
                }
            })
            .collect()
    };
    let switch_cases = PARTIALS.len();
    let mut registers = vec![0; REGISTERS.len()];
    // Each index takes its switch through the table: the last case, then the one before.
    registers[0] = (switch_cases - 1) as u64;
    registers[1] = (switch_cases - 2) as u64;
    let switches = Program {
        registers: registers.clone(),
        main: vec![
            Chunk::Switch {
                index: REGISTERS[0],
                cases: cases(switch_cases),
            },
            Chunk::Switch {
                index: REGISTERS[1],
                cases: cases(switch_cases),
            },
        ],
        functions: Vec::new(),
    };
    let access = GroupAccess {
        load: true,
        bytes: WORD as u8,
        vector: false,
        rt: REGISTERS[0],
        rt2: REGISTERS[1],
        indexing: Indexing::Offset,
        offset: 1,
    };
    let group = Program {
        registers,
        main: vec![Chunk::Call {
            callee: Callee::Function(0),
            through_register: false,
            around: None,
        }],
        functions: vec![Function {
            frame: FRAME_FLOOR,
            body: vec![Chunk::Ops(vec![Op::Group {
                base: REGISTERS[2],
                start: GroupBase::Edge(WORD as u16),
                accesses: vec![access],
                syscall: None,
            }])],
            recursion: 0,
            tail: None,
        }],
    };
    let named = |program: &Program, name: &str| Blob {
        name: name.into(),
        ..assemble(program)
    };
    vec![
        Check {
            blob: named(&switches, SWITCHES),
            counted: Some((
                Reason::JumpThroughRegisterNotRecognised,
                Counted::Exactly(0),
            )),
            at_entry: false,
        },
        Check {
            blob: named(&group, GROUP_AT_ENTRY),
            counted: None,
            at_entry: true,
        },
    ]
}
