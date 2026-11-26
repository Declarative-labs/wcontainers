//! The host thread a stand-in's unicorn instances live on, which every call of the stand-in and its spaces runs on.
//!
//! A unicorn instance may not move to another host thread, and the kernel moves its executor and spaces between its
//! runners' threads: so the stand-in, its spaces and their page tables are handles that send each call to this thread
//! and wait for its answer.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;

use crate::memory::Memory;
use crate::standin::Machine;

/// A call to run on the engine's thread.
type Job = Box<dyn FnOnce(&mut Engine) + Send>;

/// What a stand-in and its spaces hold, on the engine's thread.
#[derive(Default)]
pub(crate) struct Engine {
    /// The stand-ins' memories, threads and blocks of the abi's `Cpu`, by the key of each stand-in.
    pub(crate) machines: HashMap<u64, Machine>,
    /// The spaces of the executor interface, by their key.
    pub(crate) spaces: HashMap<u64, Rc<RefCell<Memory>>>,
    /// The memory single instructions run on, cleared before each: a new unicorn instance costs far more.
    single_instruction: Option<Memory>,
    last_key: u64,
}

impl Engine {
    /// Returns a cleared memory whose addresses end at the end given, for one instruction to run on.
    pub(crate) fn single_instruction_memory(&mut self, end: u64) -> &mut Memory {
        let memory = match self.single_instruction.take() {
            Some(mut memory) if memory.end() == end => {
                memory.clear();
                memory
            }
            _ => Memory::new(end).expect("host memory for one more space"),
        };
        self.single_instruction.insert(memory)
    }

    /// Returns a key no machine or space of this engine had.
    pub(crate) fn take_key(&mut self) -> u64 {
        self.last_key += 1;
        self.last_key
    }

    /// Returns the machine of the key, which lives as long as its stand-in.
    pub(crate) fn machine(&mut self, key: u64) -> &mut Machine {
        self.machines
            .get_mut(&key)
            .expect("a stand-in's machine lives as long as the stand-in")
    }

    /// Returns the space of the key, none once the space is dropped.
    pub(crate) fn space(&self, key: u64) -> Option<Rc<RefCell<Memory>>> {
        self.spaces.get(&key).map(Rc::clone)
    }
}

/// A handle to an engine's thread, which may move between host threads.
#[derive(Clone)]
pub(crate) struct EngineHandle {
    jobs: Sender<Job>,
    /// Shared by every handle of one engine, so handles tell whether they reach the same one.
    identity: Arc<()>,
}

impl EngineHandle {
    /// Starts a new engine on a host thread of its own, which ends once every handle to it is dropped.
    pub(crate) fn start() -> Self {
        let (jobs, received) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("unicorn".to_owned())
            .spawn(move || {
                let mut engine = Engine::default();
                for job in received {
                    job(&mut engine);
                }
            })
            .expect("a host thread for unicorn");
        Self {
            jobs,
            identity: Arc::new(()),
        }
    }

    /// Runs the call on the engine's thread and returns its answer; a panic of the call is raised again here.
    pub(crate) fn call<Answer: Send + 'static>(
        &self,
        call: impl FnOnce(&mut Engine) -> Answer + Send + 'static,
    ) -> Answer {
        let (answer, answered) = mpsc::sync_channel::<Result<Answer, Box<dyn Any + Send>>>(1);
        let job: Job = Box::new(move |engine| {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| call(engine)));
            // The caller waits for the answer until it comes.
            let _ = answer.send(outcome);
        });
        self.jobs.send(job).expect("the engine's thread runs");
        match answered.recv().expect("the engine's thread answers") {
            Ok(answer) => answer,
            Err(panicked) => panic::resume_unwind(panicked),
        }
    }

    /// Returns whether both handles reach the same engine.
    pub(crate) fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}
