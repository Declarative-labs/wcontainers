# Status: the instruction design slice (ARM64, ten instructions)

Scratch: pyro-parked/wcontainers-scratch/instruction-slice/design. Builds and tests run on the rented ARM64
machine (`arm-run --name instruction-slice`); on the Mac only editing, rustfmt on single files and
`cargo metadata --offline` for the lock.

## Step 1 (2026-11-22): the workspace compiles again

The new `Executor` trait needs `type Architecture` and `execute_single_instruction_in_place`. Two types
implement it; each now names the ARM64 architecture and has a body that says single instruction execution
is not built for it yet (`todo!`):
- `StandIn` in crates/executor/implementation/unicorn/standin.rs (the Unicorn stand-in).
- `Plain` in crates/kernel/implementation/linux/executing.rs (a test executor inside `#[cfg(test)]`).

The WebAssembly executor's `impl Executor for CpuSide` is the runtime's own trait
(crates/runtime/implementation/single-kernel/container.rs), not this one, so it needed nothing. The hardware
executor implements no `Executor` yet. No x86-64 architecture was needed: no executor of this trait runs x86.

## Step 2: the ARM64 architecture (crates/processor-architecture/implementation/arm64)

- implementation.rs: `Arm64`, `Processor` (X0-X30, SP, PC, NZCV, V0-V31, FPCR, FPSR, TPIDR_EL0, and the
  exception the last instruction raised: ESR_EL1 and FAR_EL1 where valid), `PagePermission` (unmapped, or
  mapped with a data access and an instruction fetch permission), the ten `Instruction`s, the
  `PreciseInstruction`s with every behavior below.
- encoding.rs: the operand fields and each instruction's fixed bits.
- simulation.rs: the formulas. tests.rs: 20 hand-worked cases.
- conformance.rs: runs the architecture suite on 15 behaviors over every state the generator makes.
- The suite (crates/processor-architecture/conformance): simulate agrees with simulate_in_place; simulation
  is deterministic; a failed simulation leaves the processor unchanged.

Behaviors, and what is measured:
- One behavior each (expected the same on both chips; not run on a chip yet): ADD, ADDS, CSEL, LDR and STR
  (unsigned offset), BRK. Only the syndrome of a store to a read-only page (0x9200004F, ISV 0) was measured,
  on both chips, by the earlier measure-in-vm work.
- STP across a fault: `WritesNothing`, `WritesBytesBelowFault`. Expected, not measured.
- LDR post-index with target = base: `WritebackSuppressed`, `WritebackOverwritesLoad` (the UNKNOWN value
  taken as the written-back address), `Undefined`, `NoOperation`. Expected, not measured.
- RDVL: `Undefined` (expected on the M5 outside streaming mode), `Reads(VectorLength)` (expected 128 bits
  on the Neoverse V2, if the executor gives the guest SVE). Not measured.
- MRS CTR_EL0: `MrsCtrEl0 { cache_type }`. The chips' values are not recorded yet.

Assumptions the formulas make about EL1 (an executor must configure it so): SCTLR_EL1.SA0 set, A clear,
E0E clear (little-endian), UCT set; 48-bit addresses; a missing page faults at level 3; SVE enabled for EL0
where the chip has it.

## Step 3: the generator (crates/distinguishing-state-generator/implementation/input-domain)

- implementation.rs: the draft `InstructionInputDomain` trait, in the shape asked for, and the generic
  `CriticalValueProduct` generator (every combination of each operand's critical values).
- arm64.rs: the ARM64 domain. Operands: a register field's number (0, 1, 31), a register's value (0, 1,
  sign bit, all ones), unsigned immediates (0, 1, largest), signed immediates (0, 1, -1, largest, smallest),
  every value of a field of 4 bits or fewer, the flags (all 16), where the access lies around the boundary
  of two data pages (4), and each data page's permission (unmapped, no access, read-only, read-write).
- The suite (crates/distinguishing-state-generator/conformance): every two distinct candidates of an
  instruction give different states after on at least one generated state. It passes for 21 candidates.

States per instruction: ADD 5184, ADDS 5184, CSEL 6912, LDR 1728, STR 1728, STP 8640, LDR post-index
2880, BRK 3, RDVL 15, MRS CTR_EL0 3.

## Still to fill in

- Single instruction execution for the Unicorn stand-in, the kernel's test executor and the hardware
  executor (HVF and KVM); none implements it yet.
- The probe, the recordings and one precise instruction set per chip; the chips' CTR_EL0 values and vector
  lengths; running the generated states on both chips.
- The EL0 state the processor does not hold yet (TODO in implementation.rs): DIT, SSBS, TCO, BTYPE,
  TPIDRRO_EL0, SVE and SME state.
- The fault address of an access that crosses into a faulting page in its middle is taken as the first
  faulting byte; not measured.
- The cargo reviewable check has not been run on these crates (paused for everyone).

## For the owner

- `ExecutionState` and `MemoryDelta` derive `Clone`, `PartialEq` and `Eq`, which puts those bounds on the
  architecture type itself: generic code cannot clone or compare an `ExecutionState<'_, A>` without
  `A: Clone + Eq` (the suites carry that bound now). Example: `first.simulate(before.clone())` in the
  generator suite does not compile for `A: ProcessorArchitecture` alone.
- `DistinguishingStateGenerator<Architecture>` is generic over the architecture, so a generator that serves
  any architecture cannot be called as `generator.generate_states(instruction)`: the compiler cannot infer
  the architecture from the instruction. Callers write
  `DistinguishingStateGenerator::<Arm64>::generate_states(&generator, instruction)`.
- The draft `InstructionInputDomain` has one `OperandValue` type for every operand, so `to_state` can be
  given a value of the wrong kind for an operand and must panic on it. Example: `Register(RD)` paired with
  `Integer(5)`.
