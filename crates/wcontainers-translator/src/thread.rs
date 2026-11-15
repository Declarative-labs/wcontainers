//! The thread module: one per guest thread. It owns the registers, the lookup table and the bottom loop.

use crate::contract::*;
use core::mem::offset_of;
use wasm_encoder::{
    BlockType, Catch, CodeSection, EntityType, ExportKind, ExportSection, FunctionSection,
    GlobalSection, GlobalType, HeapType, ImportSection, MemArg, MemArgTarget, Module, RefType,
    TableSection, TableType, TagKind, TagSection, TagType, TypeSection, ValType,
};
use wcontainers_abi::{RegisterBlock as B, pstate};

/// Where the packed flags sit in the register block.
const PSTATE: u64 = offset_of!(B, pstate) as u64;

/// A register kept as a global: its export name, its type, and where it sits in the register block.
pub(crate) struct RegisterGlobal {
    pub name: String,
    pub ty: ValType,
    pub offset: u64,
    /// For the flags: the bit in `pstate`.
    pub flag_bit: Option<u64>,
}

/// Every register global in export order. Code modules import them all.
pub(crate) fn registers() -> Vec<RegisterGlobal> {
    let plain = |name: String, ty, offset: usize| RegisterGlobal {
        name,
        ty,
        offset: offset as u64,
        flag_bit: None,
    };
    let mut all: Vec<_> = (0..31)
        .map(|i| plain(format!("x{i}"), ValType::I64, offset_of!(B, x) + 8 * i))
        .collect();
    all.push(plain("sp".into(), ValType::I64, offset_of!(B, sp)));
    all.push(plain("pc".into(), ValType::I64, offset_of!(B, pc)));
    for (name, bit) in [
        ("n", pstate::N),
        ("z", pstate::Z),
        ("c", pstate::C),
        ("v", pstate::V),
    ] {
        all.push(RegisterGlobal {
            name: name.into(),
            ty: ValType::I32,
            offset: offset_of!(B, pstate) as u64,
            flag_bit: Some(bit.trailing_zeros().into()),
        });
    }
    all.push(plain(
        "tpidr_el0".into(),
        ValType::I64,
        offset_of!(B, tpidr_el0),
    ));
    all.push(plain("fpsr".into(), ValType::I32, offset_of!(B, fpsr)));
    all.push(plain("fpcr".into(), ValType::I32, offset_of!(B, fpcr)));
    for i in 0..32 {
        all.push(plain(
            format!("v{i}"),
            ValType::V128,
            offset_of!(B, v) + 16 * i,
        ));
    }
    all
}

/// The piece signature: x0 to x3, sp and the expected return address in; x0 to x3 and sp out.
pub(crate) fn piece_type(types: &mut TypeSection) {
    types
        .ty()
        .function([ValType::I64; 6], [ValType::I64; crate::PASSED as usize]);
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

/// Build the thread module, with a lookup table of `slots` entries.
pub fn thread_module(slots: u32) -> Vec<u8> {
    let mut types = TypeSection::new();
    piece_type(&mut types); // 0
    types.ty().function([ValType::I64], [ValType::I32]); // 1: load_code
    types.ty().function([ValType::I32, ValType::I64], []); // 2: warn
    types.ty().function([], []); // 3: run, save, load
    types.ty().function([ValType::I32], []); // 4: unwind tag

    const LOAD_CODE: u32 = 0;
    const WARN: u32 = 1;
    const TRAP: u32 = 2;
    const TABLE_BASE_G: u32 = 0;
    const THREAD_BLOCK_G: u32 = 1;
    const FIRST_REGISTER: u32 = 2;

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

    let registers = registers();
    let global =
        |name: &str| FIRST_REGISTER + registers.iter().position(|r| r.name == name).unwrap() as u32;
    let mut globals = GlobalSection::new();
    for r in &registers {
        let init = match r.ty {
            ValType::I32 => wasm_encoder::ConstExpr::i32_const(0),
            ValType::I64 => wasm_encoder::ConstExpr::i64_const(0),
            _ => wasm_encoder::ConstExpr::v128_const(0),
        };
        globals.global(
            GlobalType {
                val_type: r.ty,
                mutable: true,
                shared: false,
            },
            &init,
        );
    }

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: piece_ref(),
        table64: false,
        minimum: slots.into(),
        maximum: Some(slots.into()),
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
    const SAVE_F: u32 = 4;
    const LOAD_F: u32 = 5;

    // run
    {
        functions.function(3);
        let mut f =
            wasm_encoder::Function::new([(1, ValType::I64), (1, ValType::Ref(piece_ref()))]);
        let (offset, piece) = (0, 1);
        let pc = global("pc");
        let mut s = f.instructions();
        s.loop_(BlockType::Empty);
        s.block(BlockType::Result(ValType::I32));
        s.try_table(BlockType::Empty, [Catch::One { tag: 0, label: 0 }]);
        lookup(&mut s, pc, TABLE_BASE_G, offset, piece);
        s.local_get(piece).ref_is_null().if_(BlockType::Empty);
        s.global_get(pc).local_set(offset);
        misaligned_code(&mut s, offset, TRAP);
        s.return_().end();
        s.global_get(pc)
            .call(LOAD_CODE)
            .i32_eqz()
            .if_(BlockType::Empty)
            .return_()
            .end();
        s.br(3);
        s.end();
        for i in 0..4 {
            s.global_get(global(&format!("x{i}")));
        }
        s.global_get(global("sp"))
            .i64_const(BOTTOM_RETURN)
            .local_get(piece)
            .ref_as_non_null()
            .call_ref(0);
        s.global_set(global("sp"));
        for i in (0..4).rev() {
            s.global_set(global(&format!("x{i}")));
        }
        s.i64_const(BOTTOM_RETURN).global_set(pc);
        s.br(2);
        s.end(); // try_table
        s.unreachable();
        s.end(); // block: the unwind payload is on the stack
        s.i32_const(UNWIND_GO_TO)
            .i32_ne()
            .if_(BlockType::Empty)
            .return_()
            .end();
        s.i32_const(Reason::LeftToAddress as i32)
            .global_get(pc)
            .call(WARN)
            .br(0);
        s.end(); // loop
        s.end();
        code.function(&f);
    }

    // save_state and load_state
    for save in [true, false] {
        functions.function(3);
        let mut f = wasm_encoder::Function::new([]);
        let mut s = f.instructions();
        if save {
            // pstate is built from the four flags, then everything else is stored in place.
            s.global_get(THREAD_BLOCK_G).i64_const(0);
            for r in registers.iter().filter(|r| r.flag_bit.is_some()) {
                s.global_get(global(&r.name))
                    .i64_extend_i32_u()
                    .i64_const(r.flag_bit.unwrap() as i64)
                    .i64_shl()
                    .i64_or();
            }
            s.i64_store(memarg(0, PSTATE, 3));
        }
        for r in registers.iter().filter(|r| r.flag_bit.is_none()) {
            let m = memarg(0, r.offset, 0);
            s.global_get(THREAD_BLOCK_G);
            if save {
                s.global_get(global(&r.name));
                match r.ty {
                    ValType::I32 => s.i32_store(m),
                    ValType::I64 => s.i64_store(m),
                    _ => s.v128_store(m),
                };
            } else {
                match r.ty {
                    ValType::I32 => s.i32_load(m),
                    ValType::I64 => s.i64_load(m),
                    _ => s.v128_load(m),
                };
                s.global_set(global(&r.name));
            }
        }
        if !save {
            for r in registers.iter().filter(|r| r.flag_bit.is_some()) {
                s.global_get(THREAD_BLOCK_G)
                    .i64_load(memarg(0, PSTATE, 0))
                    .i64_const(r.flag_bit.unwrap() as i64)
                    .i64_shr_u()
                    .i32_wrap_i64()
                    .i32_const(1)
                    .i32_and()
                    .global_set(global(&r.name));
            }
        }
        s.end();
        code.function(&f);
    }

    let mut exports = ExportSection::new();
    for (i, r) in registers.iter().enumerate() {
        exports.export(&r.name, ExportKind::Global, FIRST_REGISTER + i as u32);
    }
    exports.export(LOOKUP, ExportKind::Table, 0);
    exports.export(UNWIND, ExportKind::Tag, 0);
    exports.export(RUN, ExportKind::Func, RUN_F);
    exports.export(SAVE_STATE, ExportKind::Func, SAVE_F);
    exports.export(LOAD_STATE, ExportKind::Func, LOAD_F);

    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&tables)
        .section(&tags)
        .section(&globals)
        .section(&exports)
        .section(&code);
    module.finish()
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

/// Leave the piece for the guest address in global `pc` in local `piece`, or null if there is none.
/// `offset` is an i64 scratch local.
pub(crate) fn lookup(
    s: &mut wasm_encoder::InstructionSink<'_>,
    pc: u32,
    table_base: u32,
    offset: u32,
    piece: u32,
) {
    s.ref_null(HeapType::Concrete(0)).local_set(piece);
    s.global_get(pc)
        .global_get(table_base)
        .i64_sub()
        .local_tee(offset)
        .i64_const(3)
        .i64_and()
        .i64_eqz()
        .local_get(offset)
        .i64_const(2)
        .i64_shr_u()
        .table_size(0)
        .i64_extend_i32_u()
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
