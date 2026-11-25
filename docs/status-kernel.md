# Kernel status

Plan: docs/phase-1-kernel.md (section 14). Branch `kernel-port`, worktree
wcontainers-worktrees/kernel-port, cut from `origin/instruction-port` at b69a2d39f. The old status
(8,831 lines) stays on origin/agents. Scratch: pyro-parked/wcontainers-scratch/kernel-port.

## KP1, the kernel on the committed interfaces (2026-11-25, first run)

Done:
- The kernel and its suite came back from origin/agents (c86ddeb60's code) as they were, in a commit
  of their own. The port changed nothing in those folders (`git diff main b69a2d39f` on them is
  empty), so no port hunk needed re-applying.
- Fit, in the next commit: each guest space's `PageTable` is held beside its memory by the guest
  machine alone and taken `&mut`; the turn reads the port's `Processor<()>` and `Exception` enum
  (an exception kept whole as a syndrome gives the exit the old syndrome match gave); the unit-test
  executor is a `BehaviorSource` and `Executor` with an absent `PageTable`; the base's abi still
  has `Event::BackendNotice`, which is applied as main's kernel applied it.
- Three private traits went: siginfo.rs `Code` (one number function per code type), ipc.rs
  `Object` (each kind's `Ids` keeps where its objects hold their `Perm`), touch.rs `Memory` (the
  walk takes `CallMemory` itself).

Measured (rented machines, `-j 12` except the x86 library check, which ran at 96):
- `cargo check` and `cargo clippy --no-deps -- -D warnings` of `-p wcontainers-kernel-linux --lib`:
  exit 0 on ARM64 and on x86-64.
- The unit tests, in a copy without the stand-in dev-dependency: 26 passed on ARM64 and x86-64;
  the same 26 pass at c86ddeb60 on ARM64.
- No line of code with `unsafe` (one comment line in the manifest, as at origin/agents).

Linter, repair after the failed check (2026-11-25): 31,840 findings became 31,285 (scratch
kp1-repair/lint/summary.txt). 404 items no other crate can reach went from `pub` to `pub(crate)`
(rustc's `unreachable_pub` suggestions, 538 findings), and 8 constants that restated a binding now
name it (`SIGNALS`, `ROBUST_LIST_LIMIT`, `MAX_LINKS`, the four `SS_*`). The rest cannot be
cleared in KP1. Each group, with its smallest example:

| Findings | Cause | Smallest example |
|---|---|---|
| 20,898 | generated bindings: a rule question | `linux/general.rs:1832 pub const ROBUST_LIST_LIMIT: u32 = 2048;` is pub, has no link, and is also in another binding file |
| 1,083 `public_outside_interface` | the crate's surface, which other tracks' crates use | `implementation.rs:60 pub use cgroup::MOUNT as CGROUP_MOUNT;` |
| 666 | the model's answer is needed, and its key may not go to a rented machine | `conformance/suite.rs:64` `taken_tasks_are_resumed`, name_unanswered |
| 8,619 | rewriting the comments and literals of 74,000 lines | `build.rs:11 "cargo::rerun-if-changed={source}"`, magic_literal |
| 3 | the `System` trait's `Send` bounds | `system.rs:63 wait_until_ready(..): Send` |
| 14 | later milestones: the `Cpu` adapter (KP2), the suite (KS) | `guest_machine.rs:164` implements abi `Cpu` |
| 1 | a rule question: the executor's interface asks its caller to implement `TrapHandler` | `running.rs:31` implements `TrapHandler` |
| 1 | the same fact is in a crate the kernel does not depend on | `running.rs:229 ALIGNMENT_FAULT = 33`, also hardware-virtualization-inside syndrome.rs:7 |

The 666 are grammar_unclear, verb_not_first, name_unanswered, summary_unclear,
summary_unanswered, summary_restates_declaration, type_summary_not_noun_phrase and hook_name.
The 8,619 are magic_literal, number_constant_without_link, the summary shape rules,
comment_too_long and comment_budget_exceeded. Every rewritten summary of a function or type
also needs the model's answer.

## For the owner

1. The kernel's own `System` trait (system.rs:46) can go only when the hooks are the interface's
   `ProcessTracer` and that trait is generic over the kernel, so a hook's process can be suspended
   and kept:

```rust
pub trait ProcessTracer<K: Kernel> {
    type ProcessState: Clone + Send;

    fn intercept_system_call_request<P: ProcessOf<Self::ProcessState, K>>(
        &self,
        process: P,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        ControlFlow::Continue(request)
    }
    // ... every hook the same, with `K` for `Self::Kernel`
}

pub trait System<K: Kernel>: ProcessTracer<K> + FallbackObserver<K::Fallback> + /* ... */ Sync {}
```

2. The kernel's `System` bounds its waits' futures as `Send` with return type notation
   (system.rs:63, `#![feature(return_type_notation)]` at implementation.rs:13), which the linter
   cannot parse and calls an exclusion. Without it, the interfaces would say `Send` themselves:

```rust
pub trait Readiness<Handle> {
    fn wait_until_ready(
        &self,
        handle: &Handle,
        interest: ReadyFor,
    ) -> impl Future<Output = Result<ReadinessReport, HostError>> + Send;
}

pub trait FuseServer {
    fn send_request(&self, request: &[u8]) -> impl Future<Output = Result<Self::Reply, HostError>> + Send;
    fn receive_notice(
        &self,
        applied: Option<Result<(), Errno>>,
    ) -> impl Future<Output = Result<Self::Notice, HostError>> + Send;
    // ...
}

pub trait DomainNameResolver {
    fn resolve_domain(
        &self,
        question: Question<&Name<[u8]>>,
    ) -> impl Future<Output = Result<Message<Self::AnswerBytes>, HostError>> + Send;
    // ...
}
```
