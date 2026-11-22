# Status: the instruction design slice (ARM64, ten instructions)

Scratch: pyro-parked/wcontainers-scratch/instruction-slice/design. Builds and tests run on the rented ARM64
machine (`arm-run --name instruction-slice`).

## Step 1 (2026-11-22): the workspace compiles again

The new `Executor` trait needs `type Architecture` and `execute_single_instruction_in_place`. Two types
implement it; each now names the ARM64 architecture and has a body that says single instruction execution
is not built for it yet (`todo!`):
- `StandIn` in crates/executor/implementation/unicorn/standin.rs (the Unicorn stand-in).
- `Plain` in crates/kernel/implementation/linux/executing.rs (a test executor inside `#[cfg(test)]`).

The WebAssembly executor's `impl Executor for CpuSide` is the runtime's own trait
(crates/runtime/implementation/single-kernel/container.rs), not this one, so it needed nothing. The hardware
executor implements no `Executor` yet. No x86-64 architecture was needed: no executor of this trait runs x86.

New: crates/processor-architecture/implementation/arm64 with its types declared.
Checked: `cargo check --workspace --all-targets` passes on the ARM64 machine.

## For the owner

(nothing yet)
