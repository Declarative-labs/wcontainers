//! The thread module: one per guest thread. It owns the lookup table and the bottom loop.

use crate::contract::*;
use crate::state::Slot;
use wasm_encoder::{
    BlockType, Catch, CodeSection, EntityType, ExportKind, ExportSection, FunctionSection,
    GlobalType, HeapType, ImportSection, MemArg, MemArgTarget, Module, RefType, TableSection,
    TableType, TagKind, TagSection, TagType, TypeSection, ValType,
};

/// The piece signature: the passed registers and the expected return address in; the passed registers out.
pub(crate) fn piece_type(types: &mut TypeSection) {
    types.ty().function(
        [ValType::I64; crate::code::PARAMETERS as usize],
        [ValType::I64; crate::PASSED as usize],
    );
}

pub(crate) fn piece_ref() -> RefType {
    RefType {
        nullable: true,
        heap_type: HeapType::Concrete(0),
    }
}

pub(crate) fn state_memory() -> wasm_encoder::MemoryType {
    wasm_encoder::MemoryType {
        minimum: 1,
        maximum: Some(65536),
        memory64: false,
        shared: true,
        page_size_log2: None,
    }
}

pub(crate) fn memarg(memory: u32, offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        target: MemArgTarget::Memory(memory),
        ordering: None,
    }
}

/// Build the thread module, with a lookup table of `slots` entries for addresses and the last slot.
pub fn thread_module(slots: u32) -> Vec<u8> {
    let mut types = TypeSection::new();
    piece_type(&mut types); // 0
    types
        .ty()
        .function([ValType::I64, ValType::I32], [ValType::I32]); // 1: load_code
    types.ty().function([ValType::I32, ValType::I64], []); // 2: warn
    types.ty().function([], []); // 3: run
    types.ty().function([ValType::I32], []); // 4: unwind tag

    const LOAD_CODE: u32 = 0;
    const WARN: u32 = 1;
    const TRAP: u32 = 2;
    const TABLE_BASE_G: u32 = 0;
    const BLOCK: u32 = 1;
    // The state memory is this module's only memory.
    const STATE_M: u32 = 0;

    let mut imports = ImportSection::new();
    imports.import(HOOKS, HOOK_LOAD_CODE, EntityType::Function(1));
    imports.import(HOOKS, HOOK_WARN, EntityType::Function(2));
    imports.import(HOOKS, HOOK_TRAP, EntityType::Function(2));
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

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: piece_ref(),
        table64: false,
        minimum: u64::from(slots) + 1,
        maximum: Some(u64::from(slots) + 1),
        shared: false,
    });
    let mut tags = TagSection::new();
    tags.tag(TagType {
        kind: TagKind::Exception,
        func_type_idx: 4,
    });

    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    const RUN_F: u32 = 3;

    functions.function(3);
    let mut f = wasm_encoder::Function::new([
        (2, ValType::I64),
        (1, ValType::Ref(piece_ref())),
        (2, ValType::I32),
    ]);
    // `exact`: the piece to call next is the one in the last slot.
    let (pc, offset, piece, payload, exact) = (0, 1, 2, 3, 4);
    let mut s = f.instructions();
    s.loop_(BlockType::Empty);
    s.block(BlockType::Result(ValType::I32));
    s.try_table(BlockType::Empty, [Catch::One { tag: 0, label: 0 }]);
    s.local_get(exact).if_(BlockType::Empty);
    s.i32_const(0).local_set(exact);
    last_slot(&mut s);
    s.table_get(0).local_set(piece);
    s.else_();
    s.global_get(BLOCK);
    Slot::Pc.load_from(&mut s, STATE_M);
    s.local_set(pc);
    lookup(&mut s, pc, TABLE_BASE_G, offset, piece);
    s.local_get(piece).ref_is_null().if_(BlockType::Empty);
    misaligned_code(&mut s, pc, TRAP);
    s.return_().end();
    s.local_get(pc)
        .i32_const(0)
        .call(LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .return_()
        .end();
    // Look the address up again.
    s.br(4);
    s.end();
    s.end();
    for slot in crate::code::passed_slots() {
        s.global_get(BLOCK);
        slot.load_from(&mut s, STATE_M);
    }
    s.i64_const(BOTTOM_RETURN | FLAGS_WANTED)
        .i64_const(0)
        .local_get(piece)
        .ref_as_non_null()
        .call_ref(0);
    // The results, last first, go back to the block. `pc` is where the guest returned to.
    for slot in crate::code::passed_slots().rev() {
        s.local_set(offset).global_get(BLOCK).local_get(offset);
        slot.store_to(&mut s, STATE_M);
    }
    s.i32_const(Reason::ContinuedAfterStackLoss as i32)
        .global_get(BLOCK);
    Slot::Pc.load_from(&mut s, STATE_M);
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
    Slot::Pc.load_from(&mut s, STATE_M);
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

/// Push the index of the lookup table's last slot, which holds a piece for no address.
pub(crate) fn last_slot(s: &mut wasm_encoder::InstructionSink<'_>) {
    s.table_size(0).i32_const(1).i32_sub();
}

/// Open an `if` that is entered, after the trap hook has been called with an alignment fault, when the
/// i64 local `address` (where control is going, already in `pc`) is not a multiple of 4. The caller ends
/// it by leaving.
pub(crate) fn misaligned_code(s: &mut wasm_encoder::InstructionSink<'_>, address: u32, trap: u32) {
    s.local_get(address)
        .i64_const(3)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty)
        .i32_const(TrapKind::Alignment as i32)
        .local_get(address)
        .call(trap);
}

/// Leave the piece for the guest address in i64 local `pc` in local `piece`, or null if there is none.
/// `table_base` is the global holding [`TABLE_BASE`]; `offset` is an i64 scratch local.
pub(crate) fn lookup(
    s: &mut wasm_encoder::InstructionSink<'_>,
    pc: u32,
    table_base: u32,
    offset: u32,
    piece: u32,
) {
    s.ref_null(HeapType::Concrete(0)).local_set(piece);
    s.local_get(pc)
        .global_get(table_base)
        .i64_sub()
        .local_tee(offset)
        .i64_const(3)
        .i64_and()
        .i64_eqz()
        .local_get(offset)
        .i64_const(2)
        .i64_shr_u();
    last_slot(s);
    s.i64_extend_i32_u()
        .i64_lt_u()
        .i32_and()
        .if_(BlockType::Empty)
        .local_get(offset)
        .i64_const(2)
        .i64_shr_u()
        .i32_wrap_i64()
        .table_get(0)
        .local_set(piece)
        .end();
}
