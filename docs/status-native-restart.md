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
2. Scope: KVM, every ioctl of Linux 7.0's kvm.h that applies to ARM64 (CORRECTED by the repair run below: the
   first build held only what kvm-ioctls 0.25.1 offers plus two, 42 ioctl methods, and missed 12; now 45 ioctls
   as 54 methods, the table in "R1 repair"). HVF, every function applevisor-sys 1.0.0 declares for ARM64
   (generated from Apple's headers) except `hv_vm_allocate` and `hv_vm_deallocate`: 85 function methods.
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
| wcontainers-kvm-conformance | crates/kvm/conformance/suite.rs (21 tests since the repair; 11 before) | 0 (workspace `forbid`) | 0 |
| wcontainers-kvm-direct | implementation.rs, conformance_run.rs | 20 since the repair (9 before) | 20 (all marked, none UNSOUND) |
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

### What R1 still lacks (superseded by "R1 repair" below)

- The HVF suite run on the Mac (waits on a Mac slot; this run's rules ban the Mac).
- The two linter conflicts (ruled: see "R1 repair"), and the model's answers for summaries and names.
- An independent check by a fresh agent (run: DOES NOT HOLD on two points, repaired below).
- The instruction port's `crates/kvm` (done in the repair: one crate).

## R1 repair (run of 2026-11-25 night)

The independent check of R1 said DOES NOT HOLD on two points: the KVM trait was not the whole API, and two
mutants survived the suite. Both are repaired here.

### Branch state

- The owner's main of today (1b6b5d6dc, the processor core interface) merged: f3218432c, a normal merge, no conflict.
- origin/arm64-instructions (d708c4a55) merged: 87edf3b6a, a normal merge. Conflicts: the seven files of
  `crates/kvm` (both branches added the crate; this branch's whole-API crate kept, the port's one-method crate
  of 5d54c52d0 replaced by it), `Cargo.lock` (the port's taken, then cargo's update on the machine), and
  `single_step.rs` (the port deleted it; deleted, every port hunk kept). The port retired the monitor's
  single-step executor, so three files brought from agents that stood only on it went too: monitor
  `memory_space.rs` (agents' `GuestMemorySpace`), `conformance_run.rs` (the executor suite on
  `SingleStepMachine`) and `tests/guest_memory.rs`, with `pub mod memory_space`, the `conformance` test entry
  and its five dev-dependencies. They are in 6eab7518e for R3 to take from.

### The whole KVM API (MEASURED: `grep` of every `_IO*(KVMIO, ...)` define in Linux 7.0's
`include/uapi/linux/kvm.h`, from the rented machine's linux-aws-7.0 headers; arm64 placement READ in
arch/arm64/include/uapi/asm/kvm.h, which defines `__KVM_HAVE_IRQ_LINE` and `__KVM_HAVE_VCPU_EVENTS`, and in
docs.kernel.org's api.html "Architectures" lines; the arm64 handler behavior for the excluded generic ones is
INFERRED from the kernel's arm.c, not read in this run)

The trait now holds 54 ioctl methods over 45 ioctls (one ioctl on several objects is one method each), plus
three that are not ioctls: `create_user_memory`, `vcpu_run_page`, and new `vcpu_dirty_ring` (the vCPU's
dirty ring pages, mapped at `KVM_DIRTY_LOG_PAGE_OFFSET` when the VM turned dirty rings on before making it).
New: `set_user_memory_region2`, `create_irq_chip`, `irq_line_status`, `arm_set_device_addr`,
`arm_mte_copy_tags`, `arm_get_reg_writable_masks`, `vm_get_stats_fd`, `vcpu_get_stats_fd`, `set_signal_mask`,
`clear_dirty_log`, `reset_dirty_rings`, `pre_fault_memory`. `vcpu_run_page` now says what it maps: the
`kvm_run` page and the coalesced MMIO ring after it (the vCPU mapping is `KVM_GET_VCPU_MMAP_SIZE` long, which
on ARM64 covers both). Not an ioctl and not in the trait: mapping a guest_memfd (a plain file mapping) and
reading a statistics file (a plain read). docs.kernel.org already lists `KVM_SET_MEMORY_ATTRIBUTES2`, which
the 7.0 header does not have.

| ioctl | In the trait | Object, method | How `direct` calls it, or why it is not in the trait |
|---|---|---|---|
| `KVM_GET_API_VERSION` | yes | system: `get_api_version` | kvm-ioctls |
| `KVM_CREATE_VM` | yes | system: `create_vm` | kvm-ioctls |
| `KVM_CHECK_EXTENSION` | yes | system, VM: `check_extension, vm_check_extension` | kvm-ioctls |
| `KVM_GET_VCPU_MMAP_SIZE` | yes | system: `get_vcpu_mmap_size` | kvm-ioctls |
| `KVM_CREATE_VCPU` | yes | VM: `create_vcpu` | kvm-ioctls |
| `KVM_GET_DIRTY_LOG` | yes | VM: `get_dirty_log` | kvm-ioctls |
| `KVM_SET_USER_MEMORY_REGION` | yes | VM: `set_user_memory_region` | kvm-ioctls, marked (unsafe there) |
| `KVM_SET_USER_MEMORY_REGION2` | yes | VM: `set_user_memory_region2` | kvm-ioctls, marked (unsafe there) NEW |
| `KVM_CREATE_IRQCHIP` | yes | VM: `create_irq_chip` | kvm-ioctls NEW |
| `KVM_IRQ_LINE` | yes | VM: `irq_line` | kvm-ioctls |
| `KVM_IRQ_LINE_STATUS` | yes | VM: `irq_line_status` | raw, marked NEW |
| `KVM_SET_GSI_ROUTING` | yes | VM: `set_gsi_routing` | kvm-ioctls |
| `KVM_IRQFD` | yes | VM: `irqfd` | raw, marked |
| `KVM_IOEVENTFD` | yes | VM: `ioeventfd` | raw, marked |
| `KVM_SIGNAL_MSI` | yes | VM: `signal_msi` | kvm-ioctls |
| `KVM_ARM_SET_DEVICE_ADDR` | yes | VM: `arm_set_device_addr` | raw, marked NEW |
| `KVM_ARM_MTE_COPY_TAGS` | yes | VM: `arm_mte_copy_tags` | raw, marked NEW |
| `KVM_ARM_SET_COUNTER_OFFSET` | yes | VM: `arm_set_counter_offset` | raw, marked |
| `KVM_ARM_GET_REG_WRITABLE_MASKS` | yes | VM: `arm_get_reg_writable_masks` | raw, marked NEW |
| `KVM_CREATE_DEVICE` | yes | VM: `create_device` | kvm-ioctls |
| `KVM_SET_DEVICE_ATTR` | yes | VM, vCPU, device: `vm_set_device_attr, vcpu_set_device_attr, set_device_attr` | raw, marked (one shared line) |
| `KVM_GET_DEVICE_ATTR` | yes | VM, vCPU, device: `vm_get_device_attr, vcpu_get_device_attr, get_device_attr` | raw, marked (one shared line) |
| `KVM_HAS_DEVICE_ATTR` | yes | VM, vCPU, device: `vm_has_device_attr, vcpu_has_device_attr, has_device_attr` | kvm-ioctls |
| `KVM_RUN` | yes | vCPU: `run` | raw, marked |
| `KVM_SET_SIGNAL_MASK` | yes | vCPU: `set_signal_mask` | raw, marked NEW |
| `KVM_GET_MP_STATE` | yes | vCPU: `get_mp_state` | kvm-ioctls |
| `KVM_SET_MP_STATE` | yes | vCPU: `set_mp_state` | kvm-ioctls |
| `KVM_SET_GUEST_DEBUG` | yes | vCPU: `set_guest_debug` | kvm-ioctls |
| `KVM_GET_VCPU_EVENTS` | yes | vCPU: `get_vcpu_events` | kvm-ioctls |
| `KVM_SET_VCPU_EVENTS` | yes | vCPU: `set_vcpu_events` | kvm-ioctls |
| `KVM_ENABLE_CAP` | yes | VM: `enable_cap` | kvm-ioctls |
| `KVM_GET_ONE_REG` | yes | vCPU: `get_one_reg` | kvm-ioctls |
| `KVM_SET_ONE_REG` | yes | vCPU: `set_one_reg` | kvm-ioctls |
| `KVM_ARM_VCPU_INIT` | yes | vCPU: `arm_vcpu_init` | kvm-ioctls |
| `KVM_ARM_PREFERRED_TARGET` | yes | VM: `arm_preferred_target` | kvm-ioctls |
| `KVM_GET_REG_LIST` | yes | vCPU: `get_reg_list` | kvm-ioctls |
| `KVM_CLEAR_DIRTY_LOG` | yes | VM: `clear_dirty_log` | raw, marked NEW |
| `KVM_ARM_VCPU_FINALIZE` | yes | vCPU: `arm_vcpu_finalize` | kvm-ioctls |
| `KVM_RESET_DIRTY_RINGS` | yes | VM: `reset_dirty_rings` | raw, marked NEW |
| `KVM_GET_STATS_FD` | yes | VM, vCPU: `vm_get_stats_fd, vcpu_get_stats_fd` | raw, marked (one shared helper, 2 lines) NEW |
| `KVM_SET_MEMORY_ATTRIBUTES` | yes | VM: `set_memory_attributes` | kvm-ioctls |
| `KVM_CREATE_GUEST_MEMFD` | yes | VM: `create_guest_memfd` | kvm-ioctls, marked descriptor adoption |
| `KVM_PRE_FAULT_MEMORY` | yes | vCPU: `pre_fault_memory` | raw, marked NEW |
| `KVM_REGISTER_COALESCED_MMIO` | yes | VM: `register_coalesced_mmio` | kvm-ioctls |
| `KVM_UNREGISTER_COALESCED_MMIO` | yes | VM: `unregister_coalesced_mmio` | kvm-ioctls |
| `KVM_GET_MSR_INDEX_LIST` | no | | x86 |
| `KVM_S390_ENABLE_SIE` | no | | s390 |
| `KVM_GET_SUPPORTED_CPUID` | no | | x86 |
| `KVM_GET_EMULATED_CPUID` | no | | x86 |
| `KVM_GET_MSR_FEATURE_INDEX_LIST` | no | | x86 |
| `KVM_SET_NR_MMU_PAGES` | no | | x86 |
| `KVM_GET_NR_MMU_PAGES` | no | | x86 |
| `KVM_SET_TSS_ADDR` | no | | x86 |
| `KVM_SET_IDENTITY_MAP_ADDR` | no | | x86 |
| `KVM_S390_UCAS_MAP` | no | | s390 |
| `KVM_S390_UCAS_UNMAP` | no | | s390 |
| `KVM_S390_VCPU_FAULT` | no | | s390 |
| `KVM_S390_KEYOP` | no | | s390 |
| `KVM_GET_IRQCHIP` | no | | x86 (arm64 uses the GIC device's attributes) |
| `KVM_SET_IRQCHIP` | no | | x86 (arm64 uses the GIC device's attributes) |
| `KVM_CREATE_PIT` | no | | x86 |
| `KVM_GET_PIT` | no | | x86 |
| `KVM_SET_PIT` | no | | x86 |
| `KVM_REINJECT_CONTROL` | no | | x86 |
| `KVM_CREATE_PIT2` | no | | x86 |
| `KVM_SET_BOOT_CPU_ID` | no | | x86 |
| `KVM_XEN_HVM_CONFIG` | no | | x86 (Xen) |
| `KVM_SET_CLOCK` | no | | x86 |
| `KVM_GET_CLOCK` | no | | x86 |
| `KVM_GET_PIT2` | no | | x86 |
| `KVM_SET_PIT2` | no | | x86 |
| `KVM_PPC_GET_PVINFO` | no | | ppc |
| `KVM_SET_TSC_KHZ` | no | | x86 |
| `KVM_GET_TSC_KHZ` | no | | x86 |
| `KVM_PPC_GET_SMMU_INFO` | no | | ppc |
| `KVM_PPC_ALLOCATE_HTAB` | no | | ppc |
| `KVM_CREATE_SPAPR_TCE` | no | | ppc |
| `KVM_CREATE_SPAPR_TCE_64` | no | | ppc |
| `KVM_ALLOCATE_RMA` | no | | ppc |
| `KVM_PPC_GET_HTAB_FD` | no | | ppc |
| `KVM_PPC_RTAS_DEFINE_TOKEN` | no | | ppc |
| `KVM_PPC_RESIZE_HPT_PREPARE` | no | | ppc |
| `KVM_PPC_RESIZE_HPT_COMMIT` | no | | ppc |
| `KVM_PPC_CONFIGURE_V3_MMU` | no | | ppc |
| `KVM_PPC_GET_RMMU_INFO` | no | | ppc |
| `KVM_PPC_GET_CPU_CHAR` | no | | ppc |
| `KVM_SET_PMU_EVENT_FILTER` | no | | x86 (arm64 uses the vCPU attribute KVM_ARM_VCPU_PMU_V3_FILTER) |
| `KVM_PPC_SVM_OFF` | no | | ppc |
| `KVM_GET_REGS` | no | | generic vCPU ioctl; api.rst: every architecture but arm64; arm64's handler returns EINVAL |
| `KVM_SET_REGS` | no | | generic vCPU ioctl; api.rst: every architecture but arm64; arm64's handler returns EINVAL |
| `KVM_GET_SREGS` | no | | x86, ppc (api.rst); arm64's handler returns EINVAL |
| `KVM_SET_SREGS` | no | | x86, ppc (api.rst); arm64's handler returns EINVAL |
| `KVM_TRANSLATE` | no | | x86 (api.rst); arm64's handler returns EINVAL |
| `KVM_INTERRUPT` | no | | x86, ppc, mips, riscv, loongarch (api.rst); not in arm64's vCPU ioctls |
| `KVM_GET_MSRS` | no | | x86 |
| `KVM_SET_MSRS` | no | | x86 |
| `KVM_SET_CPUID` | no | | x86 |
| `KVM_GET_FPU` | no | | x86, loongarch (api.rst); arm64's handler returns EINVAL |
| `KVM_SET_FPU` | no | | x86, loongarch (api.rst); arm64's handler returns EINVAL |
| `KVM_GET_LAPIC` | no | | x86 |
| `KVM_SET_LAPIC` | no | | x86 |
| `KVM_SET_CPUID2` | no | | x86 |
| `KVM_GET_CPUID2` | no | | x86 |
| `KVM_TPR_ACCESS_REPORTING` | no | | x86 |
| `KVM_SET_VAPIC_ADDR` | no | | x86 |
| `KVM_S390_INTERRUPT` | no | | s390 |
| `KVM_S390_STORE_STATUS` | no | | s390 |
| `KVM_S390_SET_INITIAL_PSW` | no | | s390 |
| `KVM_S390_INITIAL_RESET` | no | | s390 |
| `KVM_NMI` | no | | x86 |
| `KVM_X86_SETUP_MCE` | no | | x86 |
| `KVM_X86_GET_MCE_CAP_SUPPORTED` | no | | x86 |
| `KVM_X86_SET_MCE` | no | | x86 |
| `KVM_GET_DEBUGREGS` | no | | x86 |
| `KVM_SET_DEBUGREGS` | no | | x86 |
| `KVM_GET_XSAVE` | no | | x86 |
| `KVM_SET_XSAVE` | no | | x86 |
| `KVM_GET_XCRS` | no | | x86 |
| `KVM_SET_XCRS` | no | | x86 |
| `KVM_DIRTY_TLB` | no | | ppc (booke) |
| `KVM_KVMCLOCK_CTRL` | no | | x86 |
| `KVM_S390_MEM_OP` | no | | s390 |
| `KVM_S390_GET_SKEYS` | no | | s390 |
| `KVM_S390_SET_SKEYS` | no | | s390 |
| `KVM_S390_IRQ` | no | | s390 |
| `KVM_S390_SET_IRQ_STATE` | no | | s390 |
| `KVM_S390_GET_IRQ_STATE` | no | | s390 |
| `KVM_SMI` | no | | x86 |
| `KVM_S390_GET_CMMA_BITS` | no | | s390 |
| `KVM_S390_SET_CMMA_BITS` | no | | s390 |
| `KVM_MEMORY_ENCRYPT_OP` | no | | x86 (SEV, TDX) |
| `KVM_MEMORY_ENCRYPT_REG_REGION` | no | | x86 (SEV) |
| `KVM_MEMORY_ENCRYPT_UNREG_REGION` | no | | x86 (SEV) |
| `KVM_HYPERV_EVENTFD` | no | | x86 |
| `KVM_GET_NESTED_STATE` | no | | x86 |
| `KVM_SET_NESTED_STATE` | no | | x86 |
| `KVM_GET_SUPPORTED_HV_CPUID` | no | | x86 |
| `KVM_S390_NORMAL_RESET` | no | | s390 |
| `KVM_S390_CLEAR_RESET` | no | | s390 |
| `KVM_S390_PV_COMMAND` | no | | s390 |
| `KVM_X86_SET_MSR_FILTER` | no | | x86 |
| `KVM_XEN_HVM_GET_ATTR` | no | | x86 (Xen) |
| `KVM_XEN_HVM_SET_ATTR` | no | | x86 (Xen) |
| `KVM_XEN_VCPU_GET_ATTR` | no | | x86 (Xen) |
| `KVM_XEN_VCPU_SET_ATTR` | no | | x86 (Xen) |
| `KVM_XEN_HVM_EVTCHN_SEND` | no | | x86 (Xen) |
| `KVM_GET_SREGS2` | no | | x86 |
| `KVM_SET_SREGS2` | no | | x86 |
| `KVM_GET_XSAVE2` | no | | x86 |
| `KVM_S390_PV_CPU_COMMAND` | no | | s390 |
| `KVM_S390_ZPCI_OP` | no | | s390 |

151 ioctls: 45 in the trait, 106 not; unclassified: 0; in the trait but not in the header: []

Rules for the new pointers, each with a suite test that fails without it: an attribute value goes through a
fixed buffer as long as the longest ARM64 attribute (`kvm_smccc_filter`, 24 bytes), and a value longer than
that is refused with `E2BIG` both ways; `clear_dirty_log` refuses a bitmap with fewer bits than pages;
`set_user_memory_region2` keeps a view of exactly `memory_size` bytes and refuses a size larger than the
memory, or a size with no memory (KVM would take host address 0 on); `irqfd` refuses
`KVM_IRQFD_FLAG_RESAMPLE` with no resample file (KVM would read descriptor 0); `arm_mte_copy_tags` takes the
tag buffer as `&mut [u8]` and names a length of exactly 16 bytes a tag; `arm_get_reg_writable_masks` writes
into an array of `KVM_ARM_FEATURE_ID_RANGE_SIZE`; `set_signal_mask` passes the length KVM checks before it
reads; the stats files and the guest_memfd come back as `OwnedFd`.

### Tests and mutants (MEASURED, rented Graviton4, Linux 7.0.0-1018-aws, rustc 1.99.0; jobs
pyro-parked/wcontainers-scratch/native-restart-r1-repair/r1-repair-job{,-2,-3}.sh, output run1.out, run2.out, run3.out)

`cargo test -p wcontainers-kvm-direct`: 21 passed, 0 failed, eight runs in a row. The checker's two tests are
in the suite as `sizes_the_dirty_log_from_a_large_slot` and `keeps_the_slot_through_a_refused_change`.
Each mutant is one edit of `direct`'s implementation.rs, then the suite:

| Mutant | Result | Test that sees it |
|---|---|---|
| keepalive (a slot keeps no view) | killed, 5 fail | keeps_memory_mapped_while_a_vcpu_lives and four more |
| dirty_half (log sized from half the slot) | killed | sizes_the_dirty_log_from_a_large_slot |
| remove_on_failure (a refused change drops the slot) | killed | keeps_the_slot_through_a_refused_change |
| buffer shorter than the longest attribute (the design that replaces `no_buffer_growth`) | killed, 2 fail | sets_up_the_interrupt_controller_and_its_interrupts, reads_and_sets_vcpu_and_vm_attributes |
| no length check on a read attribute | killed | sets_up_the_interrupt_controller_and_its_interrupts |
| run_no_error (`KVM_RUN`'s error dropped) | killed | refuses_to_run_a_vcpu_not_yet_initialized (`ENOEXEC`) |
| irqfd_ignores_resample | killed | sets_up_the_interrupt_controller_and_its_interrupts |
| has_attr_always, user_memory_unsealed, memory_any_length | killed | as in the check |
| bitmap_unchecked, size_without_memory, no_dirty_ring | killed | clears_the_dirty_log..., gives_memory_as_a_slot_of_the_second_kind, keeps_a_dirty_ring_of_the_pages_written |
| resample_unchecked (our `EINVAL` for resample with no file removed) | SURVIVES | KVM then reads descriptor 0, which in the test process is not an event file, and refuses with the same `EINVAL`; seen only where descriptor 0 is an event file |

`unsafe {}` planted in interface.rs and in suite.rs: both refused by the compiler ("usage of an `unsafe`
block"), exit 101. HVF: `cargo clippy --all-targets -- -D warnings` on its three crates with the suite and its
run type-checked as if on macOS, exit 0. NOT RUN: the HVF tests, which need the owner's Mac, where this run's
rules allow no hypervisor work; that half is type-checked only.

### The chip source on the one KVM crate (MEASURED)

Callers changed, all in crates/behavior-source/implementation/kvm-single-step (the only users of the port's
crate: `git grep wcontainers-kvm` on origin/arm64-instructions):
- machine.rs: `create_machine` makes the system, VM, memory slot, vCPU and GIC through `DirectKvm`'s trait
  methods (`open`, `create_vm`, `vm_set_device_attr` for the SMCCC filter, `create_user_memory` and
  `set_user_memory_region`, `arm_preferred_target`, `check_extension`, `create_vcpu`, `arm_vcpu_init`,
  `arm_vcpu_finalize`, `create_device`, `set_device_attr`); `Machine::run` calls `run` and reads the exit reason
  and the hypercall's function from `vcpu_run_page` as volatile bytes instead of kvm-ioctls' `VcpuFd::run`
  (which held a reference over the run page); `get` and `set` call `get_one_reg` and `set_one_reg`. Its guest
  memory is the slot's memory file, kept by the VM handle, instead of anonymous memory kept by forgetting a view.
- Cargo.toml: the `kvm-ioctls` dependency removed.
The port's two KVM suite tests are covered by this suite: fresh memory zeroed (refuses_overlapping_slots...)
and an overlapping slot refused (`EEXIST`).
Chip source tests: `--lib` 2 passed, 6 ignored (as the port has them). Its executor conformance run fails
1 of 4 (`measured_behavior_agrees_with_the_source`: "an instruction shows no one known behavior",
suite.rs:90), twice; the port's own kvm crate and chip source in the same tree fail the same test the same
way (run2.out, "port's chip source conformance exit 101"), so it is the instruction track's, not this move's.

### Linter (MEASURED, run4.out: `cargo reviewable check` from reviewable-rust da55b9d7 built on the machine; whole
workspace exit 1, 83,840 findings; these crates' share by tools/findings.py)

| Class | kvm (three crates) | hvf (three crates) | Why it stays |
|---|---|---|---|
| summary_unanswered, name_unanswered | 131, 39 | 34, 12 | need the model's answers; no key on the rented machine, nothing sent out |
| public_outside_interface (a handle's private field) | 8 | 14 | the orchestrator's ruling: reported, counted, listed for the owner |
| conformance_not_run (the run's platform `cfg`) | 1 | 1 | the same ruling |
| constant_defined_twice (build.rs) | 0 | 3 | as in R1 |

Every other finding in crates/kvm is fixed (the interface's comments cut to its budget, 2,900 of 2,918
characters by this run's count). `cargo fmt` and `cargo reviewable format`: no change. The chip source has
308 findings, nearly all in the port's own files; in machine.rs the 12 error-message strings, `MachineError`'s
public field, `to_id`'s two-line summary and the 12 register constants without a link are the port's lines,
unchanged by the move (INFERRED: compared by reading, the linter was not run on the port's tree).

### Linter rulings applied (orchestrator, this run)

No exclusion added. An API handle keeps its private field, so safe code cannot forge one; its
`public_outside_interface` findings stay reported and counted (KVM 8, HVF 14). The suite runs keep their
platform `cfg` line, as main's raw-sockets host conformance.rs does; their `conformance_not_run` findings stay
reported (KVM 1, HVF 1).

### Lint tables (task of this run)

The monitor, the inside and host-process keep their own `deny` table only because each still holds its marked
lines (58, 49, 9); each goes back to `[lints] workspace = true` when R3, R4 and R2 empty it. arm64-stop and every
new crate except the two `direct` ones inherit the workspace (`forbid`).

### Milestones after R1

R1 is repaired and waits on its second independent check. R2 (form P, the host-process form) and R3 (the VM
monitor) build only once KP1 (kernel), TR1 (translator) and RT1 (runtime) are on this branch; the instruction
port is on it now (87edf3b6a). Their tests also need X2 and X3 (executor). None of KP1, TR1, RT1, X2, X3 is
merged here (READ: `git log` of this branch), so neither starts its build in this run. What R2 can take
without them is reading: the unstopped-stub fault sits in host-process executor.rs `end_stub` (833) and the
memory drop (1240-1272): a stub not seen to end is only marked `unstopped` or counted as left unreaped, and
(INFERRED, read, not run) the blocks it was given stay mapped in it while `drop_unmapped_blocks`
(execution.rs:85) drops them once no record maps them.
The fix and its failing-first test need the crate to compile, so they wait with the build. Form I (the kernel
in process with our own memory checks) is R5, after R4, and waits on the owner's answers to brief question 4.
R4 (the inside) waits on question 3 for its unsafe lines with no outside API.

## Merges, R2 and form I's milestone (run of 2026-11-26)

### Merges (MEASURED: `git merge-base --is-ancestor`, and `git diff --quiet` per file)

- origin/kernel-port at e177511f5: 47272ad81, a normal merge, no conflict.
- origin/translator-port at its tip 15968cb89: 978b44497, a normal merge, no conflict.
- The owner's main (his checkout, fetched: ac696ea19) was already inside after the kernel port's merge; fetched
  again at the end of the run, still ac696ea19 and inside.
- Every file beside an `interface.rs` in the 29 interface folders of his main (91 files, the folder's own files, not
  its subfolders) is byte-identical to his main: 0 differ. This branch adds two interface folders of its own,
  `crates/kvm` and `crates/hypervisor-framework` (R1).

### What builds now (MEASURED, rented Graviton4, `cargo check --keep-going -p <crate> --lib` and `--all-targets`, tree 978b44497)

| Crate | `--lib` | `--all-targets` | What fails |
|---|---|---|---|
| kernel-linux | 0 errors | 22 | runtime-single-kernel (dev-dependency) |
| runtime-single-kernel | 22 | 23 | its own: `unsafe` under `forbid` (7), trait mismatches with the system interface (E0049 and more): RT1 |
| translator, arm64, guest-cpu, processor-architecture-arm64, executor-conformance, translator-test | 0 | 0 | |
| arm64-stop | 0 | 0 | |
| host-process | 4 | 33 | runtime-single-kernel; all targets also precise-instruction-set-translator (10: E0107 `Arm64` without its level, E0599 old variant names), the instruction track's |
| hardware-virtualization (monitor) | 5 | 23 | runtime-single-kernel, through the inside |
| hardware-virtualization-inside | 4 | 4 | runtime-single-kernel (`container.rs` on every target; `native.rs`'s `Execution` on the inside's own target) |
| executor-wasmtime, executor-unicorn | 22 | 33 | runtime-single-kernel; all targets also the two precise instruction sets |

### R2: form P on the owner's interfaces (built as far as it goes without RT1, X2, X3)

Commits: 37ba12d47 (the interfaces), ac9ac56ff (the security fixes and their tests).

Built (READ in the code; each measured below):
- `HostProcess` implements his `BehaviorSource` (`Architecture = Arm64<User>`, error `SingleInstructionRefusal`:
  an exclusive monitor that holds an address, which no host call can set in a stub's thread, or a host refusal,
  where it used to panic) and `Executor` (`create_guest_memory` gives the space and its page table,
  `prepare_execution` gives `HostProcessExecution` and its interrupter). `HostProcessMemory` implements
  `MemorySpace` (`borrow`, `zero`), `HostProcessPageTable` implements `PageTable` (`set_permissions`, `map_from`,
  `unmap_from`, `copy_from`); the space and its table share one handle, and the stub ends when both are dropped.
  `HostProcessExecution::run` is the loop "run until the next trap (`run_until_trap`), then ask the handler
  (`ask_handler`)", so a synchronous `run_until_trap` with one free function calling the handler is a small change.
  `Wait` is `core::future::Pending<()>`, never made: form P runs every execution to its end.
- Exceptions are his typed `Exception`: a system call is `SupervisorCall` with the pc after it (the legacy shape had
  the pc at it), an abort is `DataAbort` or `InstructionAbort` with `TranslationFaultLevel0` above the 48 bits,
  level 3 otherwise, a stub that ended is `Undecoded` with an SError syndrome.
- The legacy `Cpu` impl is gone: its methods are the executor's own (`impl HostProcessExecutor`), and the runtime's
  `Execution` impl is gone with the library's dependency on the runtime. `run_translated` is public, so the forged
  exits test runs its guest without the runtime's loop.
- Tests moved to the interface: tests/copy.rs and tests/interrupt.rs. Removed: the copy within one space
  (`a_copy_within_one_space_reads_its_bytes_as_they_were_before_it`): `PageTable::copy_from(&mut self, .., other:
  &Self, ..)` cannot name one table twice, so the interface rules it out.
- `autotests = false`; the six test targets that need other tracks build only with the new feature
  `tests-on-other-tracks` (`required-features`), whose optional dependencies are the old dev-dependencies:
  kernel, run, busybox, speed, differential (RT1; kernel also X2's wasmtime executor, busybox X3's unicorn
  executor, speed both) and conformance (the instruction track's precise-instruction-set-translator, which does not
  compile on this base).

Security fixes, each with a test that fails without it (MEASURED: the tests run on 37ba12d47 with ac9ac56ff's
tests/memory.rs and Cargo.toml, job native-r2-before, then on ac9ac56ff):

| Fault | Fix | Test | Without the fix | With it |
|---|---|---|---|---|
| A stub not seen to end keeps every shared block it was given (DESIGN-4 change 2) | a stub is killed by its own process pidfd (`Stub::kill`, rustix's safe `pidfd_send_signal`), not by asking the zygote, which may answer nothing (DESIGN-4 change 3) | `a_stub_whose_turn_failed_while_the_zygote_answers_nothing_keeps_no_shared_block` (zygote stopped, its sockets shut; B's turn refused; B unmaps the block; B's stub must map no "wcontainers shared block") | FAILED (memory.rs:792: the stub still maps it) | ok |
| The same kill, after a refused command | as above | `a_kill_the_zygote_cannot_answer_after_a_refused_command_never_leaves_a_narrowed_page_writable` (was `a_failed_kill_...`: its "the stub lives on" becomes "the stub is killed, a zombie") | FAILED (memory.rs:744) | ok |
| pidfd: "seen to end" rested on the stub's socket hanging up, which any holder of a copy of the socket keeps off | `Stub::wait_ended` polls the stub's process pidfd, readable once every thread has ended; the pidfd is opened when the zygote reports the fork, before anyone may reap | `a_dropped_memory_s_stub_whose_socket_is_held_open_is_still_seen_to_end` (was `..._not_seen_to_end_is_not_waited_for`) | FAILED (memory.rs:817: left unreaped) | ok |
| EINTR: a signal to the embedder's thread made `wait_hung_up` and `end` fail, so a live stub counted as not seen to end and its memory was lost | every wait retries `EINTR`: `wait_ended`, `hung_up`, the zygote's `sendmsg` and `recv`, `waitpid` of the zygote | `signals_at_the_embedder_s_thread_leave_no_stub_unreaped` (SIGUSR2 with a signal-hook handler sent without pause at the test's thread through 200 memories) | FAILED (8 stubs "not seen to end, left unreaped" in the first rounds) | ok |

The pidfd path for a stub that truly survives its kill (D state for 10 s) still marks it unstopped and leaves it to
the zygote's end; no test reaches it (INFERRED: no unprivileged way to hold a process off SIGKILL).

Measured (rented Graviton4, Linux 7.0.0-1018-aws, rustc 1.99.0, load 5 to 18; rented Xeon, load 17 to 22):
- `cargo check -p wcontainers-executor-host-process --all-targets`: exit 0, 0 warnings. `cargo clippy --all-targets
  -- -D warnings`: exit 0 on both machines.
- `cargo test -p wcontainers-executor-host-process --no-fail-fast` on Graviton4: exit 0; 21 test binaries plus the
  library's: 96 passed, 0 failed, 5 ignored (measurements). tests/memory.rs five more times: 17 passed each.
  On the Xeon: exit 0 (copy, floor, forged, interrupt, in_process_* and translated hold no test there).
- Form P's own 25 test binaries: 19 build and pass here (zygote, stub, files, host, threads, stop, touch, memory,
  slice, placement, embedder, floor, step, translated, interrupt, copy, forged, in_process_attacks,
  in_process_way; in_process_cost and in_process_speculation are measurements, run below); kernel, run, busybox,
  speed, differential and conformance wait as listed above.

What R2's bar still lacks, and what each waits on:
- The executor conformance suite: precise-instruction-set-translator (instruction track: `Arm64` now takes its
  privilege level, and the variant names changed).
- kernel, run, busybox, speed, differential: RT1; kernel and speed also X2 (wasmtime executor), busybox and speed
  X3 (unicorn executor). tests/run.rs and tests/differential.rs still drive the runtime's `Execution`; they move to
  `HostProcess` or `run_translated` once the runtime's loop on the owner's interfaces is known.
- Zero unsafe in the crate: 9 lines left (unchanged), none with a safe route found this run:
  - code.rs:235, 244 `dc cvau`, `ic ivau` (no crate wraps cache maintenance by address);
  - probe.rs:167 `mrs ctr_el0` (aarch64-cpu 12 has no CTR_EL0), :199 the SVE probe's assembly, :254 and :261 the
    fixed `mmap` (`FIXED_NOREPLACE`) and `munmap` of the user-space search (rustix keeps fixed mappings unsafe;
    region 3.0.2's safe `alloc_at` maps with `MAP_FIXED`, which replaces, so it is no route);
  - pages.rs:368, 383 the atomic views of the thread pages the stub writes (VolatileBytes has no atomics; this is
    the guest-memory views design owed to the owner with the thread-test review, brief rule 9: stopped);
  - stub.rs:871 `tgkill` (a thread pidfd needs Linux 6.9 and an id the stub cannot forge: enforcement decision 7);
  - and outside the count, the zygote's two assembly modules under `expect(unsafe_code)`.
  These are the orchestrator's questions (brief: unsafe that fits neither an API crate nor VolatileBytes).
- The crate keeps its own `deny` lint table while those lines stay.

Linter (MEASURED: reviewable-rust da55b9d7 built on the machine, no model key, whole workspace exit 1, 97,078
findings, 302 s; tools/findings.py): host-process 3,899 (R0: 3,564; the compiler view now runs, adding
`public_outside_interface` 294, `implementation_folder` 10, `trait_not_implemented` 1); magic_literal 2,073 and the
comment rules are the old code's. New since R0 and for the orchestrator: `implementation_folder` on
`impl VolatileBytes for HostProcessBytes` (execution.rs:351) and `impl BehaviorSource for HostProcess` (:493),
and `trait_not_implemented` on `impl MemorySpace for HostProcessMemory` (:397): the linter wants each under its own
interface's folder, where the executor interface makes `Executor: BehaviorSource` and names `MemorySpace`.
arm64-stop 176, crates/kvm 179, crates/hypervisor-framework 64 (as R1).

### R3: the VM monitor (does not build without RT1)

MEASURED above: the monitor's library stops in runtime-single-kernel, through the inside, whose `container.rs`
runs the runtime's loop beside the kernel in the virtual machine, on every target; and every monitor test boots an
inside image built from the same crate for `aarch64-unknown-none-softfloat`. So nothing of R3 compiles until RT1,
and its tests need X2 (exits, container) and X3 (busybox, kernel, speed) too. Ready for it: R1's `crates/kvm` and
`crates/hypervisor-framework` (the monitor's switch onto them, `Stopper` to `ExecutionInterrupter`, guest memory as
the slot's `SharedMapping` as the chip source does since R1's repair).

### RI: form I, the kernel in process with our own inline memory checks (new milestone, after R2)

The owner ruled on 2026-11-23 that where no virtual machine is available both native forms are built. Form I's
milestone, from the plan's W2 (docs/native-without-virtual-machine.md section 13) on the owner's interfaces:

Builds: an in-process executor implementing `Executor`, `BehaviorSource`, `MemorySpace` + `PageTable` and
`ExecutionInterrupter` over windows of one pool in the embedder's process, running the translator's
`Way::InProcess` code. Pass bar (each MEASURED on Graviton4, then on the Xeon at W3):
1. Exactness: in_process_way's four seeded tests and the window-edge test (every access form at, inside and
   outside each edge, sp moved out by every form) with 0 disagreements against the reference, cases counted.
2. Isolation: in_process_attacks passes; two I memories and a P memory in one embedder each read and write every
   edge of its window, and none sees another's bytes.
3. Speculation: in_process_speculation, every shape: under I the secret's line is cached no more often than an
   untouched line; under the uncut way at least half the rounds, which shows the test sees a leak.
4. Cost: the check's cost per access shape no higher than today's (below); the translator's benchmark set within
   the sound form's figure plus 3 points; a `getppid` entry under 100 ns median.
5. The executor conformance suite and form P's kernel, busybox and differential tests run on the I executor.
6. The crossover: busybox `sh` starting 1,000 programs, a Python start and the compute benchmark in both forms,
   with the kernel entries per second at which they cost the same.
7. Unsafe only in the places the owner rules for form I's primitives (brief question 4: entering translated code,
   the signal stack and handlers, the raw mask, fixed windows, cache maintenance), each marked; 0 elsewhere.

Started (MEASURED, job native-r2-formi, Graviton4, release, pinned to CPU 63, load 31 to 34):
- Item 3: every shape under the in-process way cached the secret's line 0.000 of rounds (an untouched line 0.000);
  under the uncut process way 1.000 for pair, acquire, vector, structure and stack pointer, 0.670 to 0.789 with
  the address loaded first. Holds.
- Item 4's per-shape baseline (20,000,000 iterations, 15 pairs a shape, medians of the pairs' ratios): no access
  -0.0%, register offset +16.6%, at base +14.6%, immediate offset +14.6%, byte +10.2%, pair +13.7%, load and store
  +24.3%, stack -0.0%.
- Items 1 and 2 on translated code in the emulator and in a P stub: in_process_way 4 passed, in_process_attacks 2
  passed.

What RI waits on: the executor itself needs the owner's answers to brief question 4 (its primitives, all
inherently unsafe and with no outside API) and enforcement decision 2 (its own process or ours behind a lock);
items 5 and 6 also need RT1, X2 and X3.
