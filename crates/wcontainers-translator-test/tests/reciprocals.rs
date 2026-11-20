//! x86's `rcpss` and `rsqrtss` as the meanings work them out (plan 5.8's formula, the WebAssembly the
//! translator emits, run in wasmtime) against the CPU running this test, on every input: all 2^32 by
//! default, the first `WC_RECIPROCAL_INPUTS` otherwise. Only on an x86-64 host, which is the reference.

#![cfg(target_arch = "x86_64")]

use core::arch::x86_64::{__m128i, _mm_castps_si128, _mm_castsi128_ps, _mm_rcp_ps, _mm_rsqrt_ps};
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, MemArg, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};
use wasmtime::{Engine, Instance, Memory, Store, TypedFunc};

/// Vectors of four inputs each call works through, and the bytes their results take.
const VECTORS: u32 = 1 << 16;
const LANES: u32 = u128::BITS / u32::BITS;
const RESULT_BYTES: u64 = VECTORS as u64 * size_of::<u128>() as u64;
const WASM_PAGE: u64 = 1 << 16;

/// A module with `rcp` and `rsqrt`, each `(first: i32)`: the four lanes of each of `VECTORS` vectors
/// from input `first` on, their results stored one after the other from address 0.
fn module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], []);
    module.section(&types);
    let mut functions = FunctionSection::new();
    functions.function(0);
    functions.function(0);
    module.section(&functions);
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: RESULT_BYTES / WASM_PAGE,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memories);
    let mut exports = ExportSection::new();
    exports.export("rcp", ExportKind::Func, 0);
    exports.export("rsqrt", ExportKind::Func, 1);
    exports.export("memory", ExportKind::Memory, 0);
    module.section(&exports);
    let mut code = CodeSection::new();
    for sqrt in [false, true] {
        // Locals after the parameter: the vector index, then the formula's three vectors.
        let mut f = Function::new([(1, ValType::I32), (3, ValType::V128)]);
        let (first, k, x, exponent, wide) = (0, 1, 2, 3, 4);
        let mut s = f.instructions();
        let lane_numbers = (0..LANES).fold(0u128, |v, n| v | u128::from(n) << (n * u32::BITS));
        s.loop_(wasm_encoder::BlockType::Empty)
            .local_get(k)
            .i32_const(size_of::<u128>() as i32)
            .i32_mul()
            .local_get(first)
            .local_get(k)
            .i32_const(LANES as i32)
            .i32_mul()
            .i32_add()
            .i32x4_splat()
            .v128_const(lane_numbers as i128)
            .i32x4_add();
        wcontainers_x86::reciprocal(&mut s, x, exponent, wide, sqrt);
        s.v128_store(MemArg {
            offset: 0,
            align: 0,
            target: wasm_encoder::MemArgTarget::Memory(0),
            ordering: None,
        })
        .local_get(k)
        .i32_const(1)
        .i32_add()
        .local_tee(k)
        .i32_const(VECTORS as i32)
        .i32_lt_u()
        .br_if(0)
        .end()
        .end();
        code.function(&f);
    }
    module.section(&code);
    module.finish()
}

/// The CPU's answer for four inputs.
fn native(inputs: [u32; 4], sqrt: bool) -> [u32; 4] {
    // SAFETY: SSE is part of x86-64; the conversions only reinterpret 128 bits.
    unsafe {
        let v = _mm_castsi128_ps(core::mem::transmute::<[u32; 4], __m128i>(inputs));
        let r = if sqrt { _mm_rsqrt_ps(v) } else { _mm_rcp_ps(v) };
        core::mem::transmute::<__m128i, [u32; 4]>(_mm_castps_si128(r))
    }
}

#[test]
fn reciprocals_match_the_cpu_on_every_input() {
    let inputs: u64 = std::env::var("WC_RECIPROCAL_INPUTS")
        .ok()
        .map(|v| v.parse().expect("a count"))
        .unwrap_or(1 << u32::BITS);
    let per_call = u64::from(VECTORS * LANES);
    let calls = inputs.div_ceil(per_call);
    let engine = Engine::default();
    let module = wasmtime::Module::new(&engine, module()).expect("the module");
    let next = AtomicU64::new(0);
    let differences = AtomicU64::new(0);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut store = Store::new(&engine, ());
                let instance = Instance::new(&mut store, &module, &[]).expect("an instance");
                let memory: Memory = instance.get_memory(&mut store, "memory").unwrap();
                let functions: [TypedFunc<i32, ()>; 2] =
                    ["rcp", "rsqrt"].map(|name| instance.get_typed_func(&mut store, name).unwrap());
                let mut results = vec![0u8; RESULT_BYTES as usize];
                loop {
                    let call = next.fetch_add(1, Ordering::Relaxed);
                    if call >= calls {
                        break;
                    }
                    let first = (call * per_call) as u32;
                    for (function, sqrt) in functions.iter().zip([false, true]) {
                        function.call(&mut store, first as i32).unwrap();
                        memory.read(&store, 0, &mut results).unwrap();
                        for (n, chunk) in results.chunks(size_of::<u128>()).enumerate() {
                            let start = first + n as u32 * LANES;
                            let ours: [u32; 4] = core::array::from_fn(|l| {
                                let at = l * size_of::<u32>();
                                u32::from_le_bytes(
                                    chunk[at..at + size_of::<u32>()].try_into().unwrap(),
                                )
                            });
                            let inputs = core::array::from_fn(|l| start + l as u32);
                            let theirs = native(inputs, sqrt);
                            for l in 0..LANES as usize {
                                if ours[l] != theirs[l]
                                    && differences.fetch_add(1, Ordering::Relaxed) < 16
                                {
                                    eprintln!(
                                        "{} of {:#010x}: ours {:#010x}, the CPU's {:#010x}",
                                        if sqrt { "rsqrt" } else { "rcp" },
                                        inputs[l],
                                        ours[l],
                                        theirs[l]
                                    );
                                }
                            }
                        }
                    }
                }
            });
        }
    });
    let inputs_run = (calls * per_call).min(inputs);
    eprintln!("{inputs_run} inputs of each, {threads} threads");
    assert_eq!(differences.load(Ordering::Relaxed), 0, "differences");
}
