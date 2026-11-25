# Native track status (restart of 2026-11-25)

Brief: pyro-parked/wcontainers-scratch/orchestrator/restart-2026-11-25/brief-native.md. Branch `native-restart`, cut
from `main` at 9fe98a6a6. Commit 6eab7518e brought from `origin/agents` (198bb11d6), by path: the four crates
(`crates/executor/implementation/{hardware-virtualization, hardware-virtualization-inside, host-process,
arm64-stop}`) and the six docs. `single_step.rs` came with its folder (agents' version; nothing edited). The
`instruction-port` branch has no edit in these folders yet (READ: `git diff --stat main instruction-port`, empty);
when it merges, every port hunk is kept.

## R0: inventory (no product code)

Measured on the rented Graviton4 (aarch64 Linux, rustc 1.99.0) with `arm-run --name native-restart`, jobs 12,
tree at 6eab7518e. Logs: pyro-parked/wcontainers-scratch/native-restart/r0/ (run1.out and run1/r0/: checks, the
linter's JSON; run2.out: the same checks with `--keep-going`). Job commands: native-restart/tools/r0-job.txt and
r0-job-2.txt. Load 1.1 at the start, 17.3 at the end of run 1, 11.4 at the end of run 2 (96 cores).

### Compiles or not (MEASURED, run 2, `cargo check --keep-going -p <crate>`)

No crate's own code reaches the compiler: each check stops in dependencies other tracks own. Run 1, without
`--keep-going`, named whichever of these failed first, which changes from run to run, so the table is run 2's.

| Crate | `--lib` | `--all-targets` | Dependencies that fail |
|---|---|---|---|
| arm64-stop | fails, 2 errors | fails, 2 | arm64 |
| host-process | fails, 352 | fails, 357 | arm64, kernel-linux; all-targets also processor-architecture-arm64, -conformance, distinguishing-state-generator-input-domain |
| hardware-virtualization-inside | fails, 353 | fails, 353 | arm64, kernel-linux, processor-architecture-arm64 (also its real target, `--bins --features kernel-test --target aarch64-unknown-none-softfloat`: 353) |
| hardware-virtualization | fails, 355 | fails, 360 | as host-process |

Error classes, by dependency:
- arm64 (1): "usage of an `unsafe` block" at crates/arm64/src/thread_state.rs:46 under the workspace `forbid`.
- kernel-linux (346): unsafe-ban errors (unsafe blocks, an `unsafe` trait implementation, E0453 `allow(unsafe_code)`
  overruled by `forbid`), E0277 `Vec<u8>` / `[u8]` not `VolatileBytes`, E0046/E0050/E0407 trait mismatches.
  Kernel track, KP1.
- processor-architecture-arm64 (2): E0053 `simulate_in_place` takes `&mut impl FnMut`, E0046 missing `Instructions`,
  `KnownBehaviors`, `DEFAULT_PERMISSION`, `list_known_behaviors`. The port's.
- processor-architecture-conformance (2): E0282, E0308. distinguishing-state-generator-input-domain (1).

A check under `-F unsafe_code` (run 1, `--lib`) stopped on the same dependencies, so it says nothing about the four
crates yet. Hypervisor.framework code (`cfg(target_os = "macos")`) is not compiled on Linux; the Mac was not used.

### `cargo reviewable check` (MEASURED, run 1)

Linter built on the machine from reviewable-rust 59ad72ce, the installed binary's commit (its sha1 equals the
install folder's build; `crates`, `Cargo.toml`, `Cargo.lock` unchanged up to `main` da55b9d7). Whole workspace:
exit 1, 81,688 findings, 259 s. No key on the rented machine, so every grammar question went unanswered
(`name_unanswered`, `summary_unanswered`: no key, nothing sent). The compiler view is missing for all four crates
(arm64 does not compile), so the rules read from it (`public_outside_interface`, `trait_outside_interface`,
`interface_item`; READ linter DECISIONS.md 7 and 57) did not run: these counts are a floor.

| Rule | arm64-stop | host-process | inside | monitor |
|---|---|---|---|---|
| magic_literal | 16 | 2,079 | 154 | 210 |
| function_summary_not_third_person | 23 | 315 | 103 | 144 |
| summary_not_one_line | 22 | 270 | 103 | 128 |
| summary_too_long | 17 | 193 | 64 | 83 |
| number_constant_without_link | 2 | 139 | 49 | 44 |
| summary_not_one_sentence | 8 | 55 | 56 | 32 |
| constant_defined_twice | | 112 | | 6 |
| summary_unanswered / name_unanswered (no key) | 3 / 10 | 205 / 154 | 4 / 10 | 82 / 35 |
| grammar_unclear / summary_unclear | 2 / 0 | 10 / 1 | 20 / 21 | 10 / 25 |
| summary_starts_with_name | 4 | 8 | 21 | 9 |
| verb_not_first | 1 | 5 | 18 | 2 |
| summary_restates_declaration | 1 | 2 | 15 | 15 |
| comment_budget_exceeded / comment_too_long | 0 / 3 | 3 / 4 | 12 / 6 | 1 / 0 |
| type_summary_not_noun_phrase | 1 | | 3 | 5 |
| conformance_not_run | 1 | 4 | 1 | 5 |
| unparsed (`#[path]` outside the folder) | | 1 | | 13 |
| compiler_view_missing | 1 | 1 | 1 | 1 |
| others (literal_restates_constant, summary_without_full_stop, hook_name) | | 3 | 2 | 3 |
| **Total** | **115** | **3,564** | **663** | **853** |

`conformance_run.rs` of the monitor and of host-process is not the one `tests!` line and names other
implementations; the inside and arm64-stop have none. `hook_name`: kvm.rs:119 `on_stop_signal` is not a trait method.

### Unsafe lines (MEASURED, the root manifest's command on agents, `git grep -nw unsafe <rev> -- '<folder>/*.rs' | grep -vE ':[0-9]+:\s*(//|#\[|#!\[|unsafe_code,|reason = )' | wc -l`)

| Crate | main | branch | Lint table on branch | `expect(unsafe_code)` marks: plain / UNSOUND / UNEXPLAINED |
|---|---|---|---|---|
| hardware-virtualization | 71 | 58 (kvm.rs 12, implementation.rs 10, hvf.rs 5, routines.rs 1, tests 30) | own `deny` (old way) | 36 / 12 / 10 |
| hardware-virtualization-inside | 51 | 49 (floor/tables.rs 25, bins 9, native.rs 6, floor 9) | own `deny` | 18 / 33 / 6 (57 marks: 49 lines plus 6 `no_mangle`, 2 `global_asm!`) |
| host-process | absent | 9 (probe.rs 4, code.rs 2, pages.rs 2, stub.rs 1) | own `deny` | 10 / 0 / 0 |
| arm64-stop | absent | 0 | workspace | 0 |

Outside these: crates/arm64/src/thread_state.rs:46, one line, on both. Marks counted by
native-restart/tools/marks.py (attributes `expect(...)` naming `unsafe_code`, by reason prefix).

### Traits (READ; verdicts by brief rule 5, INFERRED until built)

| Trait | Where | Verdict |
|---|---|---|
| `Hypervisor`, `VirtualMachine`, `VirtualCpu` (ours) | monitor implementation.rs:119, 126, 142; implemented in kvm.rs:235-559, hvf.rs:60-248 | two environments, never in one build: `cfg`-selected modules over R1's API crates (R3), or a question |
| `Stopper` (ours) | implementation.rs:158; kvm.rs:639, hvf.rs:313 | duplicates `ExecutionInterrupter`: replace (R3) |
| `Ring`, `Message` (ours) | inside link.rs:183, 75; floor/mod.rs:83 | internal: plain types (R3/R4) |
| `Scenario` (ours, test) | host-process tests/step.rs:306 | internal to one test: plain type |
| `Cpu` (legacy abi) | inside native.rs:1669, host-process executor.rs:1078 | remove (R2, R3) |
| `Execution` (runtime single-kernel) | inside native.rs:2079, host-process execution.rs:301 | goes with `Cpu` |
| `Executor` (owner's) | single_step.rs:1075, host-process execution.rs:546 | keep; written for the older shape (no `BehaviorSource`, READ agents' executor interface.rs:17) |
| `ExecutionInterrupter` (owner's) | single_step.rs:1011, host-process execution.rs:812 | keep |
| `MemorySpace` (owner's) | memory_space.rs:568, host-process execution.rs:453 | keep; `PageTable` and `BehaviorSource` are implemented nowhere |
| `VolatileBytes` (owner's) | memory_space.rs:810, host-process execution.rs:407 | own implementations beside the owner's `pointer` and `shared-mapping` ones: check whether those serve (R2, R3) |
| kernel's `Host`, `Clock`, `RandomSource`, `StreamTransport` | inside host.rs:95-135 | keep |

### Re-measure (for the check)

Upload the linter source (`git -C reviewable-rust archive 59ad72ce | arm-run --ssh 'tar -x -C <your target>/linter-src'`),
then run tools/r0-job.txt and r0-job-2.txt with your own `--name`, and summarize the JSON with tools/findings.py.

## R1: the mapping rule for `crates/kvm` and `crates/hypervisor-framework` (proposal of the first run; superseded by "R1: built" below)

Each crate: interface.rs holds one generated trait, mirroring the API one to one; `implementation/direct` is its one
implementation and holds every unsafe line, each marked; a conformance crate per the linter's A7.

Source the generator reads:
- KVM: `include/uapi/linux/kvm.h` and `arch/arm64/include/uapi/asm/kvm.h` at the kernel version kvm-bindings 0.14.2
  was generated from, plus `Documentation/virt/kvm/api.rst` for each ioctl's "Type" (system, vm, vcpu, device) and
  parameters. kvm-ioctls 0.25.1 defines 74 ioctls (MEASURED, grep of `ioctl_*_nr!`); the header has more.
- Hypervisor.framework: the arm64 headers of the pinned macOS SDK (hv_vm.h, hv_vcpu.h, hv_gic*.h, hv_vm_config.h,
  hv_vcpu_config.h, hv_vm_allocate.h, hv_sme_config.h): about 90 functions (MEASURED, grep of the local SDK).
  The x86 headers (hv_vmx.h, hv_arch_*) are left out.

| API item | Trait item |
|---|---|
| ioctl / `hv_*` function | one method, named as the API names it, prefix `KVM_` / `hv_` dropped, lower case: `create_vm`, `set_user_memory_region`, `arm_vcpu_init`, `vcpu_get_reg`, `vm_map`. Every name goes in reviewable-external-names.toml with its documentation link (G4), so no API name is reworded |
| object the API creates (KVM file descriptor of the system, a VM, a vCPU, a device; HVF vCPU, GIC, configs) | an opaque associated type of the one trait, released on drop (`close`, `hv_vcpu_destroy`); a method takes it by `&` when the call only reads it, `&mut` when it changes it |
| structure, constant, enum, flag | plain data: kvm-bindings' and applevisor-sys's generated types, through type aliases; flag sets as `bitflags` |
| pointer used only during the call | `&T` in, `&mut T` in and out, the return value out; pointer and length, or a flexible array (`kvm_reg_list`, `kvm_irq_routing`), a slice |
| memory the other side keeps using after the call (`KVM_SET_USER_MEMORY_REGION`, `hv_vm_map`) | taken by value and kept by the VM's handle until it is unmapped, so no safe caller can free memory the guest still reaches |
| memory the other side writes while we hold it (KVM's `kvm_run` page; `hv_vcpu_exit_t` from `hv_vcpu_create`) | the owner's `VolatileBytes`; the pointer's promise made once, in `PointerAndLength::new` inside `direct` |
| failure (`-1` and errno; `hv_return_t`) | `Result`; one error type per API (errno; the `HV_*` codes) shared by every method, under one reasoned `explicit_shared_error` exclusion |
| thread rule (an HVF vCPU only on the thread that made it; `hv_vcpus_exit` from any thread) | the vCPU type is not `Send`; `vcpus_exit` takes plain `Copy + Send` vCPU ids |
| call missing on the running system (newer macOS function; ioctl behind a capability) | still a method; it returns the API's own refusal (`HV_UNSUPPORTED`, the kernel's errno); no `cfg` in the trait |
| what kvm-ioctls / applevisor wrap | `direct` calls them; the rest through vmm-sys-util / applevisor-sys, one marked line each. `KVM_RUN` is the exception: kvm-ioctls holds `&mut kvm_run` over the run page (kvm.rs:91-97, 372, 410, the UNSOUND marks), which the `VolatileBytes` row forbids, so `direct` makes that call itself |
| our hvf_vector.c (a vector register passed by value, which Rust cannot) | stays in `direct` |

Open points in the rule, for the owner:
1. One trait per API with a handle type per object (above), or one trait per object kind.
2. The whole API (A7 then needs a suite case for every method, generated refusals for the unused) or only the calls
   used today (kvm.rs and hvf.rs).
3. KVM: arm64 and generic ioctls only, x86 added when an x86 host is served.
4. Each trait has one implementation (`direct`) today; brief rule 5 accepts a trait for several environments only.

## R1: built (run of 2026-11-25 evening)

Ruling it builds on (READ, the orchestrator's task text): one crate named for the API holds a safe trait; its
`implementation/direct` makes the real calls and holds every marked `unsafe`; our own code is the other
implementation. Branch state: the owner's main of today merged (d11d8a87d, a normal merge, no conflict);
the port's own commit that gives `PointerBytes` and `SharedMapping` an `address()` cherry-picked unchanged
(8117188b3, from instruction-port a14efecce), so the port's later merge meets the same lines.

### The mapping rule as built (the open points of the first run, decided)

1. One trait per API, one associated type per object the API makes (KVM: `System`, `Vm`, `Vcpu`, `Device`;
   HVF: `VmConfig`, `VcpuConfig`, `GicConfig`, `GicState`, `Vm`, `Vcpu`). Drop is the API's close, release
   or destroy.
2. Scope: KVM, every ioctl kvm-ioctls 0.25.1 offers on aarch64 plus the two the monitor uses that it lacks
   (`KVM_ARM_SET_COUNTER_OFFSET`, `KVM_GET_DEVICE_ATTR` on a VM and a vCPU): 42 ioctl methods. HVF, every
   function applevisor-sys 1.0.0 declares for ARM64 (generated from Apple's headers) except `hv_vm_allocate`
   and `hv_vm_deallocate`: 85 function methods. kvm.h's ioctls kvm-ioctls does not offer (for example
   `KVM_ARM_MTE_COPY_TAGS`, `KVM_ARM_GET_REG_WRITABLE_MASKS`, `KVM_SET_USER_MEMORY_REGION2`) are not in yet.
3. ARM64 only (`#![cfg(target_arch = "aarch64")]` on both interfaces).
4. Each trait has one implementation (`direct`) today.
Non-API methods, one rule each: `create_user_memory` (both: the memory a slot or `vm_map` keeps, a
`SharedMapping`, so no new `VolatileBytes` kind), KVM's `vcpu_run_page` (a borrow of the vCPU's
`PointerBytes`, so it cannot outlive the mapping), HVF's `vcpu_exit` (the exit record decoded from its bytes)
and `vcpu_id` (the `Copy` id `vcpus_exit` takes from any thread). Names: the API's name without its prefix,
each in reviewable-external-names.toml with its documentation link (42 KVM, 86 HVF). Errors: one per API
(`rustix::io::Errno`; HVF's `hv_return_t`), each under one reasoned `explicit_shared_error` exclusion.
Pointers into our memory that KVM keeps reading (`kvm_device_attr.addr`, `kvm_dirty_log`) never come from a
caller: the attribute value goes through a buffer at least as long as any ARM64 attribute
(`size_of::<kvm_smccc_filter>()`), and the dirty log is sized from the slot's own length.

### Files

| Crate | Files | Lines with `unsafe` (root manifest's command) | Marks |
|---|---|---|---|
| wcontainers-kvm | crates/kvm/interface.rs | 0 (workspace `forbid`) | 0 |
| wcontainers-kvm-conformance | crates/kvm/conformance/suite.rs (11 tests) | 0 (workspace `forbid`) | 0 |
| wcontainers-kvm-direct | implementation.rs, conformance_run.rs | 9 | 9 (all marked, none UNSOUND) |
| wcontainers-hypervisor-framework | crates/hypervisor-framework/interface.rs | 0 (workspace `forbid`) | 0 |
| wcontainers-hypervisor-framework-conformance | suite.rs (5 tests) | 0 (workspace `forbid`) | 0 |
| wcontainers-hypervisor-framework-direct | implementation.rs, build.rs, vector.c, conformance_run.rs | 93 | 93 (one per framework call, plus the extern block and 6 drops) |

### Where each unsafe line of kvm.rs (12) and hvf.rs (5) goes

| Line | What | Destination |
|---|---|---|
| kvm.rs:159 | `KVM_ARM_SET_COUNTER_OFFSET` | kvm direct `arm_set_counter_offset` |
| kvm.rs:183 | vCPU `KVM_GET_DEVICE_ATTR` | kvm direct `get_attribute` (VM, vCPU and device) |
| kvm.rs:325 | `set_user_memory_region` | kvm direct `set_user_memory_region` |
| kvm.rs:374 | second mapping of the run page | kvm direct `create_vcpu` (memmap2, safe) and `run_page_bytes` (the one promise) |
| kvm.rs:93, 99 | `unsafe impl Send`/`Sync for Remote` (UNSOUND) | gone: the run page is `PointerBytes`, `Send` and `Sync` with no `unsafe impl` |
| kvm.rs:415 | atomic view of `immediate_exit` (UNSOUND) | gone: R3 writes the byte through `vcpu_run_page` |
| kvm.rs:426 | `munmap` of the second mapping | gone: memmap2 unmaps on drop |
| kvm.rs:132, 140 | `sigaction` for the stop signal | not KVM: R3, by a safe crate (INFERRED: signal-hook's `flag::register`) |
| kvm.rs:344 | `madvise(MADV_DONTNEED)` in `give_back` | not KVM: R3, a hole punched in the memory file (INFERRED: rustix `fallocate`, safe; needs the file kept) |
| kvm.rs:654 | `tgkill` of the vCPU's thread | not KVM: R3, by a safe call (INFERRED: rustix or nix; to check) |
| hvf.rs:25 | the C function's declaration | hvf direct (the same C file, vector.c) |
| hvf.rs:86, 105 | `hv_vm_map`, `hv_vm_unmap` (map UNSOUND) | hvf direct `vm_map` (keeps the memory), `vm_unmap` |
| hvf.rs:299 | the C function's call | hvf direct `vcpu_set_simd_fp_reg` |
| hvf.rs:322 | `hv_vcpus_exit` | hvf direct `vcpus_exit` |

The monitor still holds its own 58 lines until R3 switches it onto these crates.

### Measured (rented Graviton4, Linux 7.0.0-1018-aws, rustc 1.99.0; job tools/r1-full-job.txt)

- `cargo fmt`, then `cargo clippy --all-targets -- -D warnings` on the three KVM crates: exit 0.
- `cargo test -p wcontainers-kvm-direct` (KVM, /dev/kvm): 11 passed, 0 failed.
- Hostile cases in the suite: register bytes shorter than the register refused both ways (`EINVAL`);
  attribute values longer (128 bytes) and shorter (1 byte) than the attribute; a vCPU run after its VM, its
  memory's handle and the system are dropped; a vCPU made on one thread and run on another; overlapping
  slots refused (`EEXIST`) until one is taken away; a dirty log asked of an empty slot (`ENOENT`).
- Mutation check: with the slot's keep-alive removed (`Some(_) => None`), `keeps_memory_mapped_while_a_vcpu_lives`
  and `refuses_overlapping_slots_until_one_is_taken_away` fail (9 passed, 2 failed), so the test sees it.
- HVF: `cargo clippy --all-targets -- -D warnings` on the three crates, with the suite and its run type-checked
  as if on macOS (the job edits their `cfg` in the machine's copy only): exit 0. NOT RUN on a Mac: this run's
  rules ban building on the Mac, so the 5 HVF tests have never run.
- `cargo reviewable check` (linter 59ad72ce, no model key on the machine): findings in these crates by class:

| Class | kvm | hvf | Why it stays |
|---|---|---|---|
| summary_unanswered, name_unanswered | 89, 26 | 34, 12 | need the model's answers; no key on the rented machine and nothing is sent out |
| public_outside_interface (a handle's private fields) | 8 | 14 | see linter conflict 1 |
| conformance_not_run (the run's platform `cfg`) | 1 | 1 | see linter conflict 2 |
| constant_defined_twice (build.rs: `TARGET_OS`, `MACOS`, `RERUN_IF_CHANGED`) | 0 | 3 | the inside's build_environment.rs holds the same; they meet when R3 moves the monitor's HVF build lines here |

### Linter conflicts, for the orchestrator (the rule is not loosened; smallest examples)

1. `public_outside_interface` wants a type that fills an associated type to be plain data with every field
   public. An API handle cannot be: `pub struct KvmVm(Arc<Machine>);` fills `Kvm::Vm`; a public field would let
   safe code build a VM handle around any file descriptor, or drop the memory a slot still reaches.
2. `conformance_not_run` wants conformance_run.rs to be exactly the `tests!` line. The KVM suite builds only on
   Linux ARM64 and the HVF suite only on macOS ARM64, so the run starts with `#![cfg(...)]`, as
   crates/raw-sockets/implementation/host/conformance.rs already does on main; without it, every
   `cargo test --workspace` on the x86 machine or on Linux would fail to build that target.

### What R1 still lacks

- The HVF suite run on the Mac (waits on a Mac slot; this run's rules ban the Mac).
- The two linter conflicts, and the model's answers for summaries and names.
- An independent check by a fresh agent.
- The instruction port's `crates/kvm` (instruction-port 5d54c52d0, one method, `&VmFd` in its interface)
  conflicts with this one when the port merges; this crate covers it (`create_user_memory` and
  `set_user_memory_region`), and kvm-single-step moves onto it.

### Lint tables (task of this run)

The monitor, the inside and host-process keep their own `deny` table only because each still holds its marked
lines (58, 49, 9); each goes back to `[lints] workspace = true` when R3, R4 and R2 empty it. arm64-stop and every
new crate except the two `direct` ones inherit the workspace (`forbid`).

### Milestones after R1

R2 (form P, the host-process form; brief order) and R3 wait on the port, KP1, TR1 and the runtime on this branch
(none merged yet), their tests also on X2 and X3. Form I (the kernel in process with our own memory checks,
docs/native.md and the owner's ruling of 2026-11-23) is R5, after R4, and waits on the owner's answers to brief
question 4. R4 (the inside) waits on question 3 for its unsafe lines with no outside API.
