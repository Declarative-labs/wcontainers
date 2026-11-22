# Status: the instruction design slice (ARM64, ten instructions)

Scratch: pyro-parked/wcontainers-scratch/instruction-slice/design. Builds and tests run on the rented ARM64
machine (`arm-run --name instruction-slice`); on the Mac only editing, rustfmt on single files and
`cargo metadata --offline` for the lock.

## FOR THE OTHER ENGINEERS: the old input domain names are gone (2026-11-22, commit 66a69f24)

- The draft trait `InstructionInputDomain` inside the generator crate is deleted; the slice uses the
  committed interface crates/instruction-input-domain (`InstructionInputDomain<Architecture>`).
- The ARM64 domain is the new crate crates/instruction-input-domain/implementation/arm64 (package
  wcontainers-instruction-input-domain-arm64), value `Arm64InputDomain`. `Arm64Operand` is now
  `OperandType`, `Arm64OperandValue` is `OperandValue` (its register variant is `RegisterNumber`);
  `AccessPlacement` and `DataPage` moved with them.
- `CriticalValueProduct` (crate wcontainers-distinguishing-state-generator-input-domain) is now generic over
  the domain it holds. Where you wrote `&CriticalValueProduct`, write
  `&CriticalValueProduct::new(Arm64InputDomain)` with
  `use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;` and add to your Cargo.toml
  `wcontainers-instruction-input-domain-arm64 = { path = "<up to crates>/instruction-input-domain/implementation/arm64" }`.
  Its states are the same as before, in the same counts.
- Not changed by me (their owners' files): crates/processor-architecture/implementation/arm64/conformance.rs,
  the recording crate, the measured set, and the executors' conformance and tests; each needs the line above.

## The input domain (2026-11-22)

- crates/instruction-input-domain/implementation/arm64: the operand types, their critical values (the
  same as step 3 below) and `build_state`. `list_operand_types` gives the operands of the assembly syntax in
  its order, then what the instruction reads besides them (register values, flags, where the access lies,
  the data pages' permissions). tests.rs: the counts per instruction, and every critical state decoded back
  (word at the program counter, every field, register values, flags, access placement, page permissions)
  gives the instruction and the operands asked for; the operands read back build the very same state.
- The suite (crates/instruction-input-domain/conformance), three functions on `(domain, architecture)`:
  every operand type an instruction lists has a critical value; one critical state per combination; the same
  operands build the same state, and the state of the first critical values is a critical state.
- crates/distinguishing-state-generator/implementation/input-domain: `CriticalValueProduct<Domain>`, whose
  `generate_states` is the domain's `generate_critical_states`. Its suite (every two behaviors told apart)
  passes for the 21 behaviors.
- For the owner: a decoding check cannot be part of the generic suite: no interface reads the instruction
  and its operands back out of a state. It needs something like
  `fn decode(&self, state: &ExecutionState<'_, Architecture>) -> Option<(Instruction, Vec<OperandValue>)>`.
  Until then the ARM64 crate checks it in its own tests.
- For the owner: an instruction's operand values are not always independent in the state. Example: ADD with
  Rn = Rm = X0 and the values 1 and all ones: X0 can hold only one of them (the last wins), and a value given
  to the zero register (number 31) is held nowhere. Such states repeat others; ADD's 5184 states hold fewer
  distinct ones.
- For the owner: one `OperandValue` type for every operand type still lets `build_state` be handed a value of
  the wrong kind (`Register(RD)` with `Integer(5)`); it panics, documented under `# Panics`.

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

## The measured set and the recordings (2026-11-22)

What exists:
- crates/processor-architecture/implementation/recording: `Recording`, a `Simulator` whose error is
  `NotRecorded`, holding each state before and the chip's state after in full; `record` runs one
  instruction on an executor over every generated state; `leave_same_memory` compares two lists of memory
  changes by the memory they leave. arm64.rs is the text form: one file per instruction, a `processor`
  line with every field of the first state, then a `before` and an `after` line per state, each listing only
  the fields that differ and the memory changes; a run of bytes longer than 16 is written once on a `bytes`
  line and named by number. Plain text so a person can read it and a changed behavior shows as a changed line
  in a diff. record.rs is the tool: `cargo test -p wcontainers-processor-architecture-recording --test record
  -- <folder>` records this host's chip through the single step machine (HVF on the Mac, signed by itself;
  KVM on Linux).
- crates/precise-instruction-set/implementation/measured: `MeasuredInstructionSet::measure(chip, known sets,
  generator)` keeps, per instruction, the one distinct known behavior the chip agrees with on every
  generated state; an instruction all known sets agree on is not run. `MeasurementFailure` is
  `NoBehaviorAgrees` (instruction, state before, each behavior with its state after, the chip's state after),
  `IndistinguishableBehaviors` (instruction, behaviors) or `NoKnownBehavior` (no set given). chip.rs measures
  the Linux host's chip and prints the time.
- crates/precise-instruction-set/implementation/apple-m5 and neoverse-v2: each only its choices. The V2's
  recordings are in neoverse-v2/recordings (32,277 states, 5.4 MB) and its suite runs against them.
- crates/executor/implementation/simulation: `Simulation`, a stand-in chip that plays a precise instruction
  set, for tests that must run on any machine.
- crates/precise-instruction-set/conformance: `agrees_with_recordings(set, recordings)`: the set's behavior
  of every listed instruction turns each recorded before into its after.

Measured (rented Graviton4, Neoverse V2, KVM, load 18 to 29):
- Recording all ten instructions, 32,277 states one at a time: 1.385 s, 0.123 s of it in the guest.
- The V2's set agrees with every recorded state: STP across a fault writes nothing, LDR post-index into its
  base suppresses the writeback, RDVL reads 128 bits, CTR_EL0 reads 0xB444C004 under KVM as on bare metal.
- Start-up measurement against the M5 and V2 sets (RDVL and MRS CTR_EL0 run, 18 states): 0.236, 0.240
  and 0.252 ms, about 0.14 ms of it in the guest; it measured the V2 set.

Not done:
- The M5's recordings: they need Hypervisor.framework, so the tool must run on the Mac, where nothing may be
  built now. Until then the M5 set is expected, not measured (TODO in its file); the suite test for it is
  held back (it would fail without recordings). To make them: run the record tool on the Mac into
  crates/precise-instruction-set/implementation/apple-m5/recordings and add a conformance.rs like the V2's.
- Recorded twice on the V2, with the old and with the committed input domain: the same 32,277 cases and the
  same answers, in another order. The generator gives ADD and ADDS 5184 states of which only 2052 differ
  (the same state comes several times); the recording keeps the repeats as the generator gives them.

### For the owner (the measured set and the recordings)

- `PreciseInstructionSet` has no way to build one, so the measured set has a placeholder `measure`. The trait
  I would need: `trait MeasuredPreciseInstructionSet<A>: PreciseInstructionSet<A> + Sized { type Failure;
  fn measure(chip: &mut impl Executor<Architecture = A>, known: &[&dyn PreciseInstructionSet<A>],
  generator: &impl DistinguishingStateGenerator<A>) -> Result<Self, Self::Failure>; }`.
- Two states holding the same memory are not equal under the derived `Eq` of `ExecutionState` when their
  change lists differ. Example: STR of the value already there; the formula reports the store, the chip
  reports nothing, and the memory is the same. The comparisons use `leave_same_memory` instead.
- The precise instruction set suite takes `Recording`, a type of an implementation crate, because no
  interface describes a recording. If recordings are to stay, a small interface for "the recorded pairs of
  one instruction" would let the suite name no implementation.
