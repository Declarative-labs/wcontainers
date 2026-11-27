//! What every executor of the process shares: the engine, the thread module, the probe's answer and the compiled
//! translations, kept by the code they were made from.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use wasmtime::{Config, Engine, Instance, Module, Store};
use wcontainers_arm64::{Addresses, Arm64, KeptRegisters, Offer};
use wcontainers_guest_cpu::Guest;
use wcontainers_translator::contract;
use wcontainers_translator::{LookAhead, Optimise, ProgressCount, translate};
use wcontainers_wasmtime::Wasmtime;
use wcontainers_wasmtime_direct::DirectWasmtime;

use crate::space::{GUEST_BYTES, GUEST_GUARD_BYTES};

/// The engine and everything made once for it.
pub(crate) struct Shared {
    pub(crate) engine: Engine,
    pub(crate) wasmtime: DirectWasmtime,
    pub(crate) thread_module: Module,
    /// What the engine does where WebAssembly leaves results open, which translations are made for.
    host: <Arm64 as Guest>::Host,
    translations: Mutex<HashMap<TranslationKey, Arc<Translated>>>,
}

/// What every program on this executor is told: every access to a register EL1 keeps from EL0 traps, and the kernel
/// answers it as Linux does, the counter from its own clock, which replay needs; and its data reaches the 2^48 bytes of
/// a 48-bit virtual address through the space's windows.
pub(crate) const OFFER: Offer = Offer {
    kept_registers: KeptRegisters::Trapped,
    addresses: Addresses::Windows,
};

/// Tuning number: the translations kept for their code at most; all are let go when one more is made, as each compiled
/// module holds mappings of its own and a process may map only so many (a single instruction's translation is one
/// module, and the agreement test makes millions).
const KEPT_TRANSLATIONS: usize = 1 << 16;

/// The translator's progress count, which bounds a run by the slice of its configuration.
pub(crate) const PROGRESS: ProgressCount = ProgressCount::Kept;

/// What a translation is kept under: the hash of its code bytes, their length, the start and the look-ahead.
#[derive(Clone, PartialEq, Eq, Hash)]
struct TranslationKey {
    hash: [u8; blake3::OUT_LEN],
    length: usize,
    start: u64,
    look_ahead: LookAhead,
}

/// A translation as wasmtime compiled it: its modules with each one's pieces, as code offsets in slot order, and the
/// code offsets it read.
pub(crate) struct Translated {
    pub(crate) modules: Vec<(Module, Vec<u64>)>,
    pub(crate) read: Vec<Range<u64>>,
}

impl Translated {
    /// Counts the pieces the translation puts in the lookup table.
    pub(crate) fn pieces(&self) -> usize {
        self.modules.iter().map(|(_, pieces)| pieces.len()).sum()
    }
}

impl Shared {
    /// Returns the process's one engine and what was made for it, made the first time.
    pub(crate) fn get() -> Arc<Shared> {
        static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(Shared::new())).clone()
    }

    fn new() -> Shared {
        let wasmtime = DirectWasmtime::default();
        let mut config = Config::new();
        config
            .wasm_tail_call(true)
            .wasm_function_references(true)
            .wasm_gc(true)
            .wasm_exceptions(true)
            .wasm_threads(true)
            .wasm_memory64(true)
            .wasm_multi_memory(true)
            .shared_memory(true)
            .memory_may_move(false)
            .memory_reservation(GUEST_BYTES)
            .memory_guard_size(GUEST_GUARD_BYTES);
        wasmtime.with_host_memory(&mut config);
        let engine = Engine::new(&config).expect("a wasmtime engine for the translator's output");
        let thread_module = Module::new(
            &engine,
            wcontainers_translator::thread_module::<Arm64>(PROGRESS),
        )
        .expect("wasmtime compiles the thread module");
        Shared {
            engine,
            wasmtime,
            thread_module,
            host: probe(),
            translations: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the translation of the code from the start offset, compiled, from those kept or made now; none when the
    /// translator refuses the code.
    pub(crate) fn translate(
        &self,
        code: &[u8],
        start: u64,
        look_ahead: LookAhead,
    ) -> Option<Arc<Translated>> {
        let key = TranslationKey {
            hash: *blake3::hash(code).as_bytes(),
            length: code.len(),
            start,
            look_ahead,
        };
        if let Some(kept) = self.kept().get(&key) {
            return Some(kept.clone());
        }
        let length = u64::try_from(code.len()).expect("code fits in an address");
        let translation = translate::<Arm64>(
            code,
            0..length,
            None,
            &[start],
            look_ahead,
            Optimise::Speed,
            PROGRESS,
            self.host,
            OFFER,
        )
        .ok()?;
        let modules = translation
            .modules
            .iter()
            .map(|module| {
                let compiled = Module::new(&self.engine, &module.wasm)
                    .expect("wasmtime compiles what the translator makes");
                (compiled, module.pieces.clone())
            })
            .collect();
        let translated = Arc::new(Translated {
            modules,
            read: translation.read,
        });
        let mut kept = self.kept();
        if kept.len() >= KEPT_TRANSLATIONS {
            kept.clear();
        }
        kept.insert(key, translated.clone());
        Some(translated)
    }

    fn kept(&self) -> std::sync::MutexGuard<'_, HashMap<TranslationKey, Arc<Translated>>> {
        self.translations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs the guest's probe module on an engine of its own, with no host memory, and returns its answer.
fn probe() -> <Arm64 as Guest>::Host {
    let engine = Engine::default();
    let module =
        Module::new(&engine, Arm64::probe_module()).expect("wasmtime compiles the probe module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("an instance of the probe");
    let probe = instance
        .get_typed_func::<(), i32>(&mut store, contract::PROBE)
        .expect("the probe's export");
    Arm64::host_from_probe(probe.call(&mut store, ()).expect("the probe runs"))
}
