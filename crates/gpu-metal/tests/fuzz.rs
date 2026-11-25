//! A seeded random driver of every method of the Metal trait on `DirectMetal`, and of `Gpu` on `MetalGpu`.
//!
//! Every field is drawn from its edge values (zero, one, each limit and one past it, powers of two around the limits,
//! the largest integers, and values whose sums and products wrap) and from valid values, in random sequences of calls on
//! random live objects, objects of a device since dropped, and objects whose other handles were dropped. Each batch
//! runs in a process of its own with Metal's API and shader validation on, so that an abort is caught and counted with
//! its seed. Every buffer keeps a shadow of what it must hold: when it is dropped and when the batch ends it is read
//! back, and a byte changed outside every range a taken call was allowed to change is a write outside its range, the
//! bytes on both sides of each call's range among them. A value past a limit reaches the call as drawn, and the call
//! must refuse it: one taken is a failure too, and ends its batch before the GPU runs it. Only a value the checks
//! pass is cut, after them, where it would make large objects or long work.
//!
//! It is an ignored test: `cargo test -p wcontainers-gpu-metal --test fuzz -- --ignored`, with `FUZZ_SEEDS` (seeds
//! joined by commas), `FUZZ_CALLS` (calls of each seed), `FUZZ_BATCH` (calls of each batch) and `FUZZ_JOBS` (batches
//! at once).

#[cfg(target_os = "macos")]
#[path = "fuzz/gpu.rs"]
mod gpu;
#[cfg(target_os = "macos")]
#[path = "fuzz/metal.rs"]
mod metal;
#[cfg(target_os = "macos")]
#[path = "fuzz/random.rs"]
mod random;
#[cfg(target_os = "macos")]
#[path = "fuzz/shadow.rs"]
mod shadow;

/// The most bytes of a buffer a batch makes: a length past a limit is sent as drawn, one within it is cut to this.
#[cfg(target_os = "macos")]
const LARGEST_MADE: u64 = 1 << 21;

/// The most of each kind of object a batch keeps; past it a new one replaces, and so drops, an old one.
#[cfg(target_os = "macos")]
const POOL: usize = 24;

/// The most bytes a texture of the batches holds, so that no batch makes one larger than a few megabytes.
#[cfg(target_os = "macos")]
const TEXTURE_BUDGET: u64 = 8 << 20;

#[cfg(target_os = "macos")]
mod driver {
    use std::collections::VecDeque;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use crate::random::Random;

    /// The variable that makes a run of this program one batch, as `seed:batch:calls`.
    pub const BATCH_VARIABLE: &str = "FUZZ_ONE_BATCH";

    /// The line a batch prints its counts on, which the driver adds up.
    pub const RESULT_LINE: &str = "FUZZ-RESULT";

    /// What a batch found: calls, refusals, canary bytes changed, GPU errors and calls taken that had to be refused.
    #[derive(Default)]
    pub struct Counts {
        pub calls: u64,
        pub refused: u64,
        pub canary_bytes: u64,
        pub gpu_errors: u64,
        pub accepted: u64,
    }

    impl Counts {
        /// Prints the counts on the result line.
        pub fn print(&self) {
            println!(
                "{RESULT_LINE} {} {} {} {} {}",
                self.calls, self.refused, self.canary_bytes, self.gpu_errors, self.accepted
            );
        }

        /// Counts one call, refused or not.
        pub fn call(&mut self, refused: bool) {
            self.calls = self.calls.saturating_add(1);
            if refused {
                self.refused = self.refused.saturating_add(1);
            }
        }

        /// Returns whether the batch found nothing wrong.
        pub fn clean(&self) -> bool {
            self.canary_bytes == 0 && self.gpu_errors == 0 && self.accepted == 0
        }
    }

    /// Runs one batch here: an even one drives the Metal trait, an odd one `Gpu`; the first two also have a window.
    pub fn run_batch(spec: &str) -> std::process::ExitCode {
        let numbers: Vec<u64> = spec.split(':').filter_map(|n| n.parse().ok()).collect();
        let [seed, batch, calls] = numbers[..] else {
            eprintln!("a batch is seed:batch:calls, not {spec}");
            return std::process::ExitCode::FAILURE;
        };
        let mut random = Random::new(seed, batch);
        let window = batch < 2;
        let counts = if batch.is_multiple_of(2) {
            crate::metal::run(&mut random, calls, window)
        } else {
            crate::gpu::run(&mut random, calls, window)
        };
        counts.print();
        if counts.clean() {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        }
    }

    /// One batch running in its own process.
    struct Running {
        seed: u64,
        batch: u64,
        child: Child,
        started: Instant,
        /// The file its messages go to: a pipe would fill and stop a batch that writes many before it ends.
        messages: std::path::PathBuf,
    }

    /// How long a batch may run before it counts as hung.
    const BATCH_TIME: Duration = Duration::from_secs(300);

    /// Runs every batch of every seed, each in its own process, and fails if any aborted, hung or changed a canary.
    pub fn fuzz() -> Result<(), libtest_mimic::Failed> {
        let variable = |name: &str, default: &str| std::env::var(name).unwrap_or(default.into());
        let seeds: Vec<u64> = variable("FUZZ_SEEDS", "1,2,3")
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        let calls: u64 = variable("FUZZ_CALLS", "700000").parse().unwrap_or(700_000);
        let per_batch: u64 = variable("FUZZ_BATCH", "4000")
            .parse()
            .unwrap_or(4000)
            .max(1);
        let jobs: usize = variable("FUZZ_JOBS", "4").parse().unwrap_or(4).max(1);
        let program = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut waiting: VecDeque<(u64, u64)> = VecDeque::new();
        for &seed in &seeds {
            for batch in 0..calls.div_ceil(per_batch) {
                waiting.push_back((seed, batch));
            }
        }
        let started = Instant::now();
        let mut running: Vec<Running> = Vec::new();
        let mut total = Counts::default();
        let mut failures: Vec<String> = Vec::new();
        let mut batches = 0u64;
        while !waiting.is_empty() || !running.is_empty() {
            while running.len() < jobs
                && let Some((seed, batch)) = waiting.pop_front()
            {
                let messages = std::env::temp_dir()
                    .join(format!("fuzz-{}-{seed}-{batch}.txt", std::process::id()));
                let file = std::fs::File::create(&messages).map_err(|e| e.to_string())?;
                let child = Command::new(&program)
                    .env(BATCH_VARIABLE, format!("{seed}:{batch}:{per_batch}"))
                    .env("MTL_DEBUG_LAYER", "1")
                    .env("MTL_DEBUG_LAYER_ERROR_MODE", "assert")
                    .env("MTL_SHADER_VALIDATION", "1")
                    .stdout(Stdio::piped())
                    .stderr(file)
                    .spawn()
                    .map_err(|e| e.to_string())?;
                running.push(Running {
                    seed,
                    batch,
                    child,
                    started: Instant::now(),
                    messages,
                });
            }
            std::thread::sleep(Duration::from_millis(20));
            let mut still = Vec::new();
            for mut job in running.drain(..) {
                let done = job.child.try_wait().map_err(|e| e.to_string())?;
                if done.is_none() && job.started.elapsed() < BATCH_TIME {
                    still.push(job);
                    continue;
                }
                if done.is_none() {
                    // A batch that never ends is a failure too: a wait the GPU never answers.
                    let _ = job.child.kill();
                }
                let output = job.child.wait_with_output().map_err(|e| e.to_string())?;
                batches = batches.saturating_add(1);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut counted = false;
                for line in stdout.lines() {
                    let words: Vec<u64> = line
                        .strip_prefix(RESULT_LINE)
                        .map(|rest| {
                            rest.split_whitespace()
                                .filter_map(|w| w.parse().ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    if let [calls, refused, canary, errors, accepted] = words[..] {
                        total.calls = total.calls.saturating_add(calls);
                        total.refused = total.refused.saturating_add(refused);
                        total.canary_bytes = total.canary_bytes.saturating_add(canary);
                        total.gpu_errors = total.gpu_errors.saturating_add(errors);
                        total.accepted = total.accepted.saturating_add(accepted);
                        counted = true;
                    }
                }
                let stderr = std::fs::read_to_string(&job.messages).unwrap_or_default();
                let _ = std::fs::remove_file(&job.messages);
                if !output.status.success() || !counted {
                    // Each line that names a failure, with the line after it, where Metal's assertions give their reason.
                    let lines: Vec<&str> = stderr.lines().collect();
                    let why: String = lines
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| {
                            l.contains("failed assertion")
                                || l.contains("panicked")
                                || l.contains("validation")
                                || l.contains("CANARY")
                                || l.contains("ACCEPTED")
                                || l.contains("error")
                        })
                        .take(4)
                        .flat_map(|(at, l)| {
                            [
                                *l,
                                lines.get(at.saturating_add(1)).copied().unwrap_or_default(),
                            ]
                        })
                        .collect::<Vec<_>>()
                        .join(" | ");
                    let line = format!(
                        "seed {} batch {}: {:?}{} {}",
                        job.seed,
                        job.batch,
                        output.status,
                        if done.is_none() { " (hung)" } else { "" },
                        why.chars().take(600).collect::<String>()
                    );
                    eprintln!("FAIL {line}");
                    failures.push(line);
                }
            }
            running = still;
        }
        println!(
            "fuzz: seeds {seeds:?}, {batches} batches, {} calls, {} refused, {} canary bytes changed, {} GPU errors, {} \
             taken that had to be refused, {} failed batches, {:.1} s",
            total.calls,
            total.refused,
            total.canary_bytes,
            total.gpu_errors,
            total.accepted,
            failures.len(),
            started.elapsed().as_secs_f64()
        );
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n").into())
        }
    }
}

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    if let Ok(spec) = std::env::var(driver::BATCH_VARIABLE) {
        return driver::run_batch(&spec);
    }
    let arguments = libtest_mimic::Arguments::from_args();
    let trials = vec![
        libtest_mimic::Trial::test("fuzzes_the_metal_trait_and_gpu", driver::fuzz)
            .with_ignored_flag(true),
    ];
    libtest_mimic::run(&arguments, trials).exit()
}

#[cfg(not(target_os = "macos"))]
fn main() {}
