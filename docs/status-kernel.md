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

Not cleared: `cargo reviewable check` finds 31,840 findings in the kernel's two crates, 20,898 of
them in the generated bindings (linux/linux/). By rule and cause, in scratch lint-after/summary.txt.

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
