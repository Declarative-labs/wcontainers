# Status: the instruction design slice (ARM64, ten instructions)

Builds and tests run on the rented ARM64 machine (`arm-run --name instruction-slice`); the M5 runs on the Mac at
`nice -n 19`, one cargo command at a time, in a build folder deleted afterwards. The reading order and every open
question for the owner are in pyro-parked/wcontainers-scratch/instruction-slice/FOR-THE-OWNER.md.

## Where things are (2026-11-22, after the repair of both checks)

- crates/processor-architecture/implementation/arm64: `Arm64`, its processor, page permissions, the ten
  instructions and every behavior (implementation.rs); the operand fields (encoding.rs); the formulas
  (simulation.rs); the syndrome parts and the memory readers the executors share (syndrome.rs, memory.rs);
  `list_behaviors`, every behavior the crate defines, which the architecture and generator suites both use.
  tests.rs: 23 hand-worked cases.
- crates/instruction-input-domain/implementation/arm64: `Arm64InputDomain` on the committed interface.
- crates/distinguishing-state-generator/implementation/input-domain: `CriticalValueProduct<Domain>`.
- crates/processor-architecture/implementation/recording: `Recording` (a `Simulator` whose error is
  `NotRecorded`) and its text form (text.rs).
- crates/precise-instruction-set/implementation: `AppleM5` and `NeoverseV2`, each checked against its own
  recordings (recordings/ beside each); `Translator` and `UnicornMax`, the WebAssembly and unicorn back ends;
  `measured`, the probe (`MeasuredInstructionSet::measure`), its chip test (chip.rs) and the record tool
  (`cargo run -p wcontainers-precise-instruction-set-measured --example record -- <folder>`).
- crates/executor/implementation: hardware-virtualization/single_step.rs (`SingleStepMachine`, HVF and KVM),
  wasmtime/single_instruction.rs and `Arm64Executor`, unicorn's `StandIn`, simulation (`Simulation`, a stand-in
  chip that plays a set).
- The suites: processor-architecture (simulate agrees with simulate_in_place, determinism, a failed simulation
  leaves the processor; and `leave_same_memory`, the one memory comparison), instruction-input-domain,
  distinguishing-state-generator (every two behaviors told apart), precise-instruction-set (a set against
  recorded instruction, before and after triples), executor (an executor against a set on every generated state).

## Behaviors, all measured on both chips

- ADD, ADDS, CSEL, BRK: one behavior.
- LDR and STR (unsigned offset), STP, LDR (post-index): `MisalignedStackPointer` `Faults` (both chips) or
  `Accesses` (unicorn, which leaves SCTLR_EL1.SA0's check out).
- STR and STP across a fault: `StoreAcrossFault` `WritesNothing` (both chips, the translator) or
  `WritesBytesBelowFault` (unicorn).
- LDR post-index into its own base: `WritebackSuppressed` (V2), `Undefined` (M5), `WritebackOverwritesLoad`
  (translator, unicorn), `NoOperation` (no chip seen).
- RDVL: `Reads(Bits128)` (V2), `SmeTrapped(DisabledByCpacr)` (M5: syndrome 0x76000000, since
  Hypervisor.framework shows no SME and EL1 leaves it trapped), `SmeTrapped(NotInStreamingMode)` (M5 with SMEN
  forced open, 0x76000002, measured by the executor's engineer), `Undefined` (translator, unicorn).
- MRS CTR_EL0: 0xB444C004 (V2 under KVM), 0x9_9444_C004 (M5 inside the virtual machine), 0x8444C004 (unicorn),
  the translator's constant.
- Every instruction: a misaligned program counter or a page without instruction fetch faults before it runs.
  Hand-tested only: no generated state has such a code page, since it cannot tell two behaviors apart.

## Measured (rented Graviton4 Neoverse V2 under KVM, load 6 to 15 on 96 cores; Mac M5, load 4.7 to 12.7)

- States per instruction: ADD 5184, ADDS 5184, CSEL 6912, LDR 1728, STR 1728, STP 8640, LDR post-index 2880,
  BRK 3, RDVL 15, MRS 3: 32,277 in all.
- Start-up probe against the M5 and V2 sets (they differ on post-index, RDVL and MRS): V2 0.08 to 0.17 ms over
  15 runs; M5 0.034 to 0.070 ms over 5. Both measure as their own set.
- Batch, warm: V2 4.23 to 4.31 us a state (0.78 to 0.81 in the guest), M5 2.94 (0.71 to 0.79). Cold: V2 8.1 to
  8.4, M5 4.1 to 4.3. One at a time through the executor suite: V2 10.7 to 10.8, M5 5.5 to 5.8.
- Comparing written pages 64 bytes at a time took the V2's warm batch from 4.8 to 5.1 down to 4.2 to 4.3.
- Full recording: V2 1.372 s (0.114 s in the guest), twice identical; M5 0.879 and 0.889 s (0.086 s), twice
  identical.
- The executor suite: the translator 3.95 s and the unicorn stand-in 15.3 s over every state, each against its own
  set; both pass.

## Not done

- The processor leaves out part of EL0's state (TODO on `Processor`: TPIDRRO_EL0, TPIDR2_EL0, DIT, SSBS, TCO,
  BTYPE, SVCR, Z beyond 128 bits, P, FFR, ZA, ZT0, FPMR, GCSPR_EL0, the exclusive monitor).
- The generator repeats states where operands alias (ADD and ADDS: 5184 states, 2052 distinct).
- `cargo reviewable check` was not run (paused); the slice's implementation crates export helpers and constants
  its rules will flag.

## For the owner

Each is a question with one example in FOR-THE-OWNER.md: how executors and precise sets are built (placeholders
`SingleStepMachine::new`, `Arm64Executor::new`, `StandIn::new`, `Simulation::new`, `MeasuredInstructionSet::measure`);
when two execution states hold the same memory (`leave_same_memory` stands in); a batch form of execution;
permissions on part of a page; listing an instruction's behaviors; reading operands back out of a state; operand
independence and one value type in the input domain.
