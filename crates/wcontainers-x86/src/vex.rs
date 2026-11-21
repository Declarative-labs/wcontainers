//! The VEX-encoded instructions of AVX and AVX2. Each 128-bit lane of one that works lane by lane runs as its
//! legacy SSE form on the lane's halves of the registers, through `Lane`; the others are written here.

use crate::decode::{Instruction, lane_form};
use crate::exact::Part;
use crate::guest::{Control, Hook, Reason, Trap};
use crate::lanes::*;
use crate::operand::*;
use crate::{Condition, Flag, Flags, Host, Offer, X86_64, XMM_BYTES};
use iced_x86::{Mnemonic, OpKind, Register};
use wasm_encoder::{BlockType, InstructionSink, ValType};
use wcontainers_guest_cpu::{Access, Emitter, Guest, Target};

/// Where a lane's operand is: a register of the second file, or a local the lane's result is built in.
#[derive(Clone, Copy)]
enum Place {
    Vector(u8),
    Local(u32),
}

/// The xmm numbers a lane's legacy form names: xmm0 (the mask a legacy blend reads), the destination, and the
/// sources after it.
const SLOT_REGISTERS: &[Register] = &[
    Register::XMM0,
    Register::XMM1,
    Register::XMM2,
    Register::XMM3,
];
const LANE_SLOTS: usize = SLOT_REGISTERS.len();
const MASK_SLOT: usize = 0;
const DESTINATION_SLOT: usize = 1;
/// VEX.vvvv's operand, the first source, which a legacy form leaves out.
const FIRST_SOURCE: u32 = 1;

/// An emitter for one lane: the legacy form's xmm registers are the lane's places, its memory operand is
/// already checked when the instruction is 256 bits wide, and the instruction's own `next` ends it.
struct Lane<'a, E: ?Sized> {
    e: &'a mut E,
    slots: [Place; LANE_SLOTS],
    checked: bool,
}

impl<E: Emitter<X86_64> + ?Sized> Emitter<X86_64> for Lane<'_, E> {
    fn sink(&mut self) -> InstructionSink<'_> {
        self.e.sink()
    }
    fn local(&mut self, ty: ValType) -> u32 {
        self.e.local(ty)
    }
    fn get(&mut self, r: u8) {
        self.e.get(r)
    }
    fn set(&mut self, r: u8) {
        self.e.set(r)
    }
    fn this_address(&mut self, by: i64) {
        self.e.this_address(by)
    }
    fn flags_live(&self) -> bool {
        self.e.flags_live()
    }
    fn set_flags(&mut self, flags: Flags) {
        self.e.set_flags(flags)
    }
    fn flag(&mut self, flag: Flag) {
        self.e.flag(flag)
    }
    fn get_vector(&mut self, v: u8) {
        match self.slots[usize::from(v)] {
            Place::Vector(x) => self.e.get_vector(x),
            Place::Local(l) => {
                self.e.sink().local_get(l);
            }
        }
    }
    fn set_vector(&mut self, v: u8) {
        match self.slots[usize::from(v)] {
            Place::Vector(x) => self.e.set_vector(x),
            Place::Local(l) => {
                self.e.sink().local_set(l);
            }
        }
    }
    fn get_control(&mut self, control: Control) {
        self.e.get_control(control)
    }
    fn set_control(&mut self, control: Control) {
        self.e.set_control(control)
    }
    fn hook(&mut self, hook: Hook) {
        self.e.hook(hook)
    }
    fn host(&self) -> Host {
        self.e.host()
    }
    fn offer(&self) -> Offer {
        self.e.offer()
    }
    fn float_mode(&mut self) -> bool {
        self.e.float_mode()
    }
    fn stack_aligned(&mut self) -> bool {
        self.e.stack_aligned()
    }
    fn check(&mut self, address: u32, bytes: u32, access: Access, which: u8) {
        if self.checked {
            X86_64::data_address(&mut self.e.sink(), address);
        } else {
            self.e.check(address, bytes, access, which)
        }
    }
    fn check_range(&mut self, address: u32, length: u32, access: Access) -> u32 {
        self.e.check_range(address, length, access)
    }
    fn warn(&mut self, reason: Reason) {
        self.e.warn(reason)
    }
    fn raise_stop(&mut self) {
        self.e.raise_stop()
    }
    fn jump(&mut self, target: Target) {
        self.e.jump(target)
    }
    fn jump_if(&mut self, target: Target) {
        self.e.jump_if(target)
    }
    fn call(&mut self, target: Target) {
        self.e.call(target)
    }
    fn ret(&mut self, target: u32) {
        self.e.ret(target)
    }
    fn syscall(&mut self) {
        self.e.syscall()
    }
    fn trap(&mut self, trap: Trap) {
        self.e.trap(trap)
    }
    fn next(&mut self) {}
    fn condition(&mut self, cond: Condition) {
        self.e.condition(cond)
    }
}

/// Whether the VEX instruction is 256 bits wide.
fn wide(i: &Instruction) -> bool {
    let ins = &i.iced;
    (0..ins.op_count())
        .any(|n| ins.op_kind(n) == OpKind::Register && ymm(ins.op_register(n)).is_some())
        || memory_bytes(i) == 2 * XMM_BYTES
}

/// The instructions whose first operand, a vector register, is a source only: they write the flags, a
/// general register or xmm0.
fn reads_first(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Vptest
            | Vtestps
            | Vtestpd
            | Vcomiss
            | Vcomisd
            | Vucomiss
            | Vucomisd
            | Vpcmpestri
            | Vpcmpestri64
            | Vpcmpistri
            | Vpcmpestrm
            | Vpcmpestrm64
            | Vpcmpistrm
    )
}

/// The string compares that write their mask to xmm0.
fn writes_xmm0(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Vpcmpestrm | Vpcmpestrm64 | Vpcmpistrm)
}

/// The moves whose memory operand must be aligned to its size.
fn aligned_move(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Vmovaps | Vmovapd | Vmovdqa | Vmovntps | Vmovntpd | Vmovntdq | Vmovntdqa
    )
}

/// The immediate bits of a legacy form whose immediate names elements, which the upper lane of a 256-bit
/// form takes from above the lower lane's: `blendps` one a single, `blendpd` and `shufpd` one a double,
/// `mpsadbw` the two offsets of section 4.1 of its description (two bits and one).
fn lane_immediate_bits(legacy: Mnemonic) -> u32 {
    use Mnemonic::*;
    match legacy {
        Blendps => XMM_BYTES / crate::simd::DWORD,
        Blendpd | Shufpd => XMM_BYTES / size_of::<u64>() as u32,
        Mpsadbw => crate::simd::MPSADBW_DESTINATION_SHIFT as u32 + 1,
        _ => 0,
    }
}

/// The VEX instruction's legacy form for lane `lane`, the places its slots stand for, and the place its
/// destination starts as when that is a vector register. `None` when no legacy form runs it.
fn lane_instruction(
    i: &Instruction,
    lane: u32,
) -> Option<(Instruction, [Place; LANE_SLOTS], Option<Place>)> {
    let ins = &i.iced;
    let (legacy, kind) = lane_form(ins.code());
    if kind == LANE_NONE || (wide(i) && ins.mnemonic() == Mnemonic::Vptest) {
        return None;
    }
    let count = ins.op_count();
    let left_out = |n: u32| match kind {
        LANE_FROM_FIRST_SOURCE => n == FIRST_SOURCE,
        LANE_FROM_FIRST_SOURCE_MASK_LAST => n == FIRST_SOURCE || n == count - 1,
        _ => false,
    };
    let mut taken = [0; LANE_SLOTS];
    let mut len = 0;
    for n in (0..count).filter(|&n| !left_out(n)) {
        taken[len] = n;
        len += 1;
    }
    let order = &taken[..len];
    let place = |n: u32| {
        let r = ins.op_register(n);
        match ymm(r) {
            Some(x) if lane > 0 => Place::Vector(ymm_high(x)),
            Some(x) => Place::Vector(x),
            None => Place::Vector(xmm(r).expect("a vector register")),
        }
    };
    let vector = |n: u32| {
        ins.op_kind(n) == OpKind::Register
            && (xmm(ins.op_register(n)).is_some() || ymm(ins.op_register(n)).is_some())
    };
    let mut form = *ins;
    form.set_code(legacy);
    let mut slots = [Place::Vector(0); LANE_SLOTS];
    let mut start = None;
    for (k, &n) in order.iter().enumerate() {
        let k = k as u32;
        match ins.op_kind(n) {
            OpKind::Register if vector(n) => {
                let slot = DESTINATION_SLOT + k as usize;
                slots[slot] = place(n);
                form.set_op_kind(k, OpKind::Register);
                form.set_op_register(k, SLOT_REGISTERS[slot]);
            }
            OpKind::Register => {
                form.set_op_kind(k, OpKind::Register);
                form.set_op_register(k, ins.op_register(n));
            }
            other => form.set_op_kind(k, other),
        }
    }
    if kind == LANE_FROM_FIRST_SOURCE_MASK_LAST {
        slots[MASK_SLOT] = place(count - 1);
        form.set_op_kind(order.len() as u32, OpKind::Register);
        form.set_op_register(order.len() as u32, SLOT_REGISTERS[MASK_SLOT]);
    }
    if lane > 0 {
        if memory_bytes(i) == 2 * XMM_BYTES {
            form.set_memory_displacement64(
                form.memory_displacement64().wrapping_add(XMM_BYTES.into()),
            );
        }
        let bits = lane_immediate_bits(form.mnemonic());
        if bits > 0 {
            form.set_immediate8(form.immediate8() >> (bits * lane));
        }
    }
    if vector(0) && !reads_first(ins.mnemonic()) {
        start = Some(place(if kind == LANE_IN_ORDER { 0 } else { 1 }));
    }
    let part = match (wide(i), lane) {
        (false, _) => Part::Whole,
        (true, 0) => Part::LowLane,
        (true, _) => Part::HighLane,
    };
    let form = Instruction {
        iced: form,
        vex_lane: Some(part),
        ..*i
    };
    Some((form, slots, start))
}

/// Run the legacy form `code` (`legacy xmm1, xmm2`) on the vectors in locals `a`, its destination's value,
/// which becomes its result, and `b`, as `part` of the VEX instruction `i`.
pub(crate) fn run_legacy<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    code: iced_x86::Code,
    part: Part,
    a: u32,
    b: u32,
) {
    let mut form = i.iced;
    form.set_code(code);
    for (k, slot) in [DESTINATION_SLOT, DESTINATION_SLOT + 1]
        .into_iter()
        .enumerate()
    {
        form.set_op_kind(k as u32, OpKind::Register);
        form.set_op_register(k as u32, SLOT_REGISTERS[slot]);
    }
    let form = Instruction {
        iced: form,
        vex_lane: Some(part),
        ..*i
    };
    let mut slots = [Place::Vector(0); LANE_SLOTS];
    slots[DESTINATION_SLOT] = Place::Local(a);
    slots[DESTINATION_SLOT + 1] = Place::Local(b);
    let mut lane = Lane {
        e,
        slots,
        checked: true,
    };
    assert!(
        crate::meaning::legacy_meaning(&mut lane, &form),
        "a written legacy form"
    );
}

/// Whether the instruction is one of these with its meaning written.
pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    matches!(i.iced.mnemonic(), Vzeroupper | Vzeroall)
        || lane_instruction(i, 0).is_some_and(|(form, ..)| crate::meaning::written(&form))
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let ins = &i.iced;
    match m {
        Vzeroupper | Vzeroall => {
            for x in 0..crate::guest::XMM_COUNT as u8 {
                e.sink().v128_const(0);
                e.set_vector(ymm_high(x));
                if m == Vzeroall {
                    e.sink().v128_const(0);
                    e.set_vector(x);
                }
            }
            return;
        }
        _ => {}
    }
    let wide = wide(i);
    // The alignment of an aligned move, and one check of a 256-bit operand's memory, before anything changes.
    if let Some(kind) = memory_use(i)
        && (wide || aligned_move(m))
    {
        let at = address(e, i);
        let bytes = memory_bytes(i);
        if aligned_move(m) {
            e.sink()
                .local_get(at)
                .i64_const(i64::from(bytes) - 1)
                .i64_and()
                .i64_const(0)
                .i64_ne()
                .if_(BlockType::Empty);
            e.trap(Trap::GeneralProtection);
            e.sink().end();
        }
        if wide {
            e.check(at, bytes, check_kind(kind), 0);
        }
    }
    let lanes = if wide { 2 } else { 1 };
    let mut results = [None; 2];
    for (lane, result) in results.iter_mut().enumerate().take(lanes) {
        let (form, mut slots, start) = lane_instruction(i, lane as u32).expect("a lane form");
        if let Some(Place::Vector(x)) = start {
            let t = e.local(ValType::V128);
            e.get_vector(x);
            e.sink().local_set(t);
            slots[DESTINATION_SLOT] = Place::Local(t);
            *result = Some(t);
        }
        let mut lane = Lane {
            e: &mut *e,
            slots,
            checked: wide,
        };
        assert!(
            crate::meaning::legacy_meaning(&mut lane, &form),
            "a written lane form"
        );
    }
    // A 128-bit form clears the upper half of the register it writes; one that writes memory, a general
    // register or the flags changes no vector register.
    if writes_xmm0(m) {
        e.sink().v128_const(0);
        e.set_vector(ymm_high(0));
    }
    if let Some(x) = (ins.op_kind(0) == OpKind::Register && !reads_first(m))
        .then(|| ymm(ins.op_register(0)).or_else(|| xmm(ins.op_register(0))))
        .flatten()
    {
        e.sink().local_get(results[0].expect("a lane result"));
        e.set_vector(x);
        match results[1] {
            Some(high) => e.sink().local_get(high),
            None => e.sink().v128_const(0),
        };
        e.set_vector(ymm_high(x));
    }
}
