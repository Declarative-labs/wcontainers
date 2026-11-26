//! The thread module: one per guest thread. It owns the lookup table and the bottom loop.

use crate::ProgressCount;
use crate::contract::*;
use crate::layout::Layout;
use crate::state::Slot;
use wasm_encoder::{
    BlockType, Catch, CodeSection, EntityType, ExportKind, ExportSection, FunctionSection,
    GlobalType, HeapType, ImportSection, MemArg, MemArgTarget, Module, RefType, TableSection,
    TableType, TagKind, TagSection, TagType, TypeSection, ValType,
};
use wcontainers_guest_cpu::Guest;

/// The type index of a piece, the first type of every module.
pub(crate) const T_PIECE: u32 = 0;

/// The piece signature: the passed registers, the expected return address and the depth in, the passed
/// registers out; with the progress count kept, the count last in and last out.
pub(crate) fn piece_type<G: Guest>(types: &mut TypeSection, progress: ProgressCount) {
    types.ty().function(
        vec![ValType::I64; crate::code::parameters::<G>(progress) as usize],
        vec![ValType::I64; crate::code::results::<G>(progress) as usize],
    );
}

pub(crate) fn piece_ref() -> RefType {
    RefType {
        nullable: true,
        heap_type: HeapType::Concrete(T_PIECE),
    }
}

pub(crate) fn state_memory() -> wasm_encoder::MemoryType {
    wasm_encoder::MemoryType {
        minimum: 1,
        maximum: Some((1u64 << u32::BITS) / crate::contract::WASM_PAGE_BYTES),
        memory64: false,
        shared: true,
        page_size_log2: None,
    }
}

/// A memarg's alignment for an access of a `T`: the log2 of its size, its natural alignment.
pub(crate) const fn natural_align<T>() -> u32 {
    size_of::<T>().trailing_zeros()
}

pub(crate) fn memarg(memory: u32, offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        target: MemArgTarget::Memory(memory),
        ordering: None,
    }
}

/// Build the thread module, for code modules translated with `progress`. Its lookup table starts with
/// [`EXACT_SLOT`] and the slot before it; the runner grows it for the pieces it adds.
pub fn thread_module<G: Guest>(progress: ProgressCount) -> Vec<u8> {
    const T_LOAD_CODE: u32 = T_PIECE + 1;
    const T_WARN_TRAP: u32 = T_LOAD_CODE + 1;
    const T_RUN: u32 = T_WARN_TRAP + 1;
    const T_UNWIND: u32 = T_RUN + 1;
    let mut types = TypeSection::new();
    piece_type::<G>(&mut types, progress);
    types
        .ty()
        .function([ValType::I64, ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32, ValType::I64], []);
    types.ty().function([], []);
    types.ty().function([ValType::I32], []);
    debug_assert_eq!(types.len(), T_UNWIND + 1);

    const LOAD_CODE: u32 = 0;
    const WARN: u32 = 1;
    const TRAP: u32 = 2;
    const TABLE_BASE_G: u32 = 0;
    const BLOCK: u32 = 1;
    const SLOTS_G: u32 = 2;
    const SLOTS_COVER_G: u32 = 3;
    // The state memory is this module's only memory.
    const STATE_M: u32 = 0;

    let mut imports = ImportSection::new();
    imports.import(HOOKS, HOOK_LOAD_CODE, EntityType::Function(T_LOAD_CODE));
    imports.import(HOOKS, HOOK_WARN, EntityType::Function(T_WARN_TRAP));
    imports.import(HOOKS, HOOK_TRAP, EntityType::Function(T_WARN_TRAP));
    imports.import(ENV, STATE, EntityType::Memory(state_memory()));
    let constant = |ty| {
        EntityType::Global(GlobalType {
            val_type: ty,
            mutable: false,
            shared: false,
        })
    };
    imports.import(ENV, TABLE_BASE, constant(ValType::I64));
    imports.import(ENV, THREAD_BLOCK, constant(ValType::I32));
    imports.import(ENV, SLOTS, constant(ValType::I32));
    imports.import(ENV, SLOTS_COVER, constant(ValType::I64));

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: piece_ref(),
        table64: false,
        minimum: EXACT_SLOT as u64 + 1,
        maximum: None,
        shared: false,
    });
    let mut tags = TagSection::new();
    tags.tag(TagType {
        kind: TagKind::Exception,
        func_type_idx: T_UNWIND,
    });

    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    const RUN_F: u32 = TRAP + 1;

    functions.function(T_RUN);
    let mut f = wasm_encoder::Function::new([
        (2, ValType::I64),
        (1, ValType::Ref(piece_ref())),
        (2, ValType::I32),
    ]);
    // `exact`: the piece to call next is the one in the exact slot.
    let pc = 0;
    let offset = pc + 1;
    let piece = offset + 1;
    let payload = piece + 1;
    let exact = payload + 1;
    // How deep each label below nests, from the outermost.
    const LOOK_UP: u32 = 0;
    const RESULT: u32 = LOOK_UP + 1;
    const CATCH: u32 = RESULT + 1;
    const IF_EXACT: u32 = CATCH + 1;
    const IF_MISSING: u32 = IF_EXACT + 1;
    let mut s = f.instructions();
    s.loop_(BlockType::Empty);
    s.block(BlockType::Result(ValType::I32));
    s.try_table(BlockType::Empty, [Catch::One { tag: 0, label: 0 }]);
    s.local_get(exact).if_(BlockType::Empty);
    s.i32_const(0).local_set(exact);
    s.i32_const(EXACT_SLOT).table_get(0).local_set(piece);
    s.else_();
    s.global_get(BLOCK);
    Slot::pc::<G>().load_from(&mut s, STATE_M);
    s.local_set(pc);
    let slots = Slots {
        memory: STATE_M,
        table_base: Value::Global(TABLE_BASE_G),
        at: Value::Global(SLOTS_G),
        cover: Value::Global(SLOTS_COVER_G),
    };
    lookup::<G>(&mut s, pc, slots, offset, piece);
    s.local_get(piece).ref_is_null().if_(BlockType::Empty);
    misaligned_code::<G>(&mut s, pc, TRAP, |s| {
        s.return_();
    });
    s.local_get(pc)
        .i32_const(0)
        .call(LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .return_()
        .end();
    // Look the address up again.
    s.br(IF_MISSING - LOOK_UP);
    s.end();
    s.end();
    for slot in crate::code::passed_slots::<G>() {
        s.global_get(BLOCK);
        slot.load_from(&mut s, STATE_M);
    }
    s.i64_const(G::UNREACHABLE_RETURN | G::FLAGS_WANTED)
        .i64_const(0);
    let count = memarg(
        STATE_M,
        progress_count_offset::<G>() as u64,
        natural_align::<u64>(),
    );
    if progress == ProgressCount::Kept {
        s.global_get(BLOCK).i64_load(count);
    }
    s.local_get(piece).ref_as_non_null().call_ref(T_PIECE);
    // The results, last first, go back to the block. `pc` is where the guest returned to.
    if progress == ProgressCount::Kept {
        s.local_set(offset)
            .global_get(BLOCK)
            .local_get(offset)
            .i64_store(count);
    }
    for slot in crate::code::result_slots::<G>().rev() {
        s.local_set(offset).global_get(BLOCK).local_get(offset);
        slot.store_to(&mut s, STATE_M);
    }
    s.i32_const(Reason::ContinuedAfterStackLoss as i32)
        .global_get(BLOCK);
    Slot::pc::<G>().load_from(&mut s, STATE_M);
    s.call(WARN);
    s.br(2);
    s.end(); // try_table
    s.unreachable();
    s.end(); // block: the unwind payload is on the stack
    s.local_tee(payload)
        .i32_const(UNWIND_LEAVE)
        .i32_eq()
        .if_(BlockType::Empty)
        .return_()
        .end();
    s.global_get(BLOCK);
    Slot::pc::<G>().load_from(&mut s, STATE_M);
    s.local_set(pc);
    s.i32_const(Reason::StackNearlyFull as i32)
        .i32_const(Reason::LeftToAddress as i32)
        .local_get(payload)
        .i32_const(UNWIND_DEEP)
        .i32_eq()
        .select()
        .local_get(pc)
        .call(WARN);
    s.local_get(payload)
        .i32_const(UNWIND_GO_TO_EXACT)
        .i32_eq()
        .if_(BlockType::Empty)
        .local_get(pc)
        .i32_const(1)
        .call(LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .return_()
        .end()
        .i32_const(1)
        .local_set(exact)
        .end();
    s.br(0);
    s.end(); // loop
    s.end();
    code.function(&f);

    let mut exports = ExportSection::new();
    exports.export(LOOKUP, ExportKind::Table, 0);
    exports.export(UNWIND, ExportKind::Tag, 0);
    exports.export(RUN, ExportKind::Func, RUN_F);

    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&tables)
        .section(&tags)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// Where the i64 local `address` (where control is going, already in `pc`) is not a multiple of the
/// instruction alignment, call the trap hook with an alignment fault and then `leave`. Nothing is emitted
/// for a guest whose instructions may start at any byte.
pub(crate) fn misaligned_code<G: Guest>(
    s: &mut wasm_encoder::InstructionSink<'_>,
    address: u32,
    trap: u32,
    leave: impl FnOnce(&mut wasm_encoder::InstructionSink<'_>),
) {
    if G::ALIGN == 1 {
        return;
    }
    s.local_get(address)
        .i64_const(G::ALIGN as i64 - 1)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty)
        .i32_const(TrapKind::Alignment as i32)
        .local_get(address)
        .call(trap);
    leave(s);
    s.end();
}

/// A global or a local.
#[derive(Clone, Copy)]
pub(crate) enum Value {
    Global(u32),
    Local(u32),
}

impl Value {
    pub(crate) fn push(self, s: &mut wasm_encoder::InstructionSink<'_>) {
        match self {
            Value::Global(g) => s.global_get(g),
            Value::Local(l) => s.local_get(l),
        };
    }
}

/// Where a lookup finds the array of slots: the memory it is in, and the values of [`TABLE_BASE`],
/// [`SLOTS`] and [`SLOTS_COVER`].
#[derive(Clone, Copy)]
pub(crate) struct Slots {
    pub(crate) memory: u32,
    pub(crate) table_base: Value,
    pub(crate) at: Value,
    pub(crate) cover: Value,
}

/// Leave the piece for the guest address in i64 local `pc` in local `piece`, or null if there is none:
/// the array of slots gives its slot in the lookup table. `offset` is an i64 scratch local.
pub(crate) fn lookup<G: Guest>(
    s: &mut wasm_encoder::InstructionSink<'_>,
    pc: u32,
    slots: Slots,
    offset: u32,
    piece: u32,
) {
    s.ref_null(HeapType::Concrete(T_PIECE)).local_set(piece);
    s.local_get(pc);
    slots.table_base.push(s);
    s.i64_sub().local_tee(offset);
    // One i32 slot per `ALIGN` bytes, so the slot's byte index is the offset shifted by 2 - log2(ALIGN).
    if G::ALIGN > 1 {
        s.i64_const(G::ALIGN as i64 - 1)
            .i64_and()
            .i64_eqz()
            .local_get(offset);
    }
    slots.cover.push(s);
    s.i64_lt_u();
    if G::ALIGN > 1 {
        s.i32_and();
    }
    s.if_(BlockType::Empty);
    slots.at.push(s);
    s.local_get(offset).i32_wrap_i64();
    if Layout::<G>::ALIGN_SHIFT < SLOT_SHIFT {
        s.i32_const((SLOT_SHIFT - Layout::<G>::ALIGN_SHIFT) as i32)
            .i32_shl();
    }
    s.i32_add()
        .i32_load(memarg(slots.memory, 0, natural_align::<SlotEntry>()))
        .table_get(0)
        .local_set(piece)
        .end();
}
