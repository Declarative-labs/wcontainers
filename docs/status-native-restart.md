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
- `autotests = false`; the five test targets that need other tracks build only with the new feature
  `tests-on-other-tracks` (`required-features`), whose optional dependencies are the old dev-dependencies:
  kernel, busybox, speed, differential (RT1; kernel also X2's wasmtime executor, busybox X3's unicorn executor,
  speed both) and conformance (the instruction track's precise-instruction-set-translator, which does not compile
  on this base). tests/run.rs drives `run_translated` a turn at a time as the runtime's loop did (its own `Turn`,
  the count as `until` and a nanosecond a unit as the slice, as the removed `turn_end` did), so it builds now.

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
- Form P's own test binaries (27 targets): 20 build and pass here (zygote, stub, files, host, threads, stop, touch,
  memory, slice, placement, embedder, floor, step, translated, run, interrupt, copy, forged, in_process_attacks,
  in_process_way; in_process_cost and in_process_speculation are measurements, run below); kernel, busybox,
  speed, differential and conformance wait as listed above. tests/run.rs after its move (Graviton4):
  14 passed, 0 failed; clippy with warnings denied clean.

What R2's bar still lacks, and what each waits on:
- The executor conformance suite: precise-instruction-set-translator (instruction track: `Arm64` now takes its
  privilege level, and the variant names changed).
- kernel, busybox, speed, differential: RT1; kernel and speed also X2 (wasmtime executor), busybox and speed X3
  (unicorn executor). tests/differential.rs still implements the runtime's `Answer`; it moves once the runtime's
  loop on the owner's interfaces is known.
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

## Merges, R2's bar test by test, the Linux process API crate, RI's decisions (run of 2026-11-26, second)

### Merges (MEASURED: `git merge-base --is-ancestor`, and `git diff --quiet` per file)

- His main (his checkout, fetched: ac696ea19) and origin/kernel-port (e177511f5) were already inside.
- origin/translator-port 82e47e5bb: merged, no conflict. origin/arm64-instructions 44e944a2f: merged, no conflict.
  origin/executor-restart 62d137472: merged, no conflict. origin/runtime-restart 322758f61: merged, one conflict in
  `Cargo.lock` (both sides' packages kept; cargo's update on the machine settled it).
- His 29 interface folders: 91 files beside an `interface.rs`, 0 differ from his main.

### What builds now (MEASURED, Graviton4, `cargo check --keep-going -p <crate>`, tree 221da738e)

| Crate | `--lib` | `--all-targets` | What fails |
|---|---|---|---|
| kernel-linux | 1 error | 1 | `running.rs:203` builds a `Processor` without the two SVE fields the arm64-instructions merge added (E0063): kernel track, one line |
| runtime-single-kernel | 0 (with the kernel line patched in the machine copy only) | 0 | RT1 builds |
| executor-wasmtime, executor-unicorn | 76 | 76 to 87 | wasmtime's own: `syndrome`, `INSTRUCTION_BYTES` gone from processor-architecture-arm64, `set_permissions`/`map_from` moved to `PageTable` (X2 not done) |
| precise-instruction-set-translator | 11 | 12 | instruction track (as before) |
| hardware-virtualization-inside, monitor | 4 to 84 | | `syndrome` import, and the kernel's container API changed under `container.rs` (R3) |
| arm64-stop, host-process | 0 | 0 | |

### R2's bar, test by test (MEASURED unless marked)

| Test of the bar | Builds | Runs | Waits on |
|---|---|---|---|
| executor conformance suite (`conformance_run.rs`) | yes, now against a Neoverse V2's recorded behaviors (behavior-source recording `neoverse-v2`, measured with the ARM64 generator), no longer the translator's precise set | 21 interface cases ok and `next_state_after_an_error...` ok; `agrees_with_precise_instruction_set` fails in 0.09 s at `measure_recording` and `measured_behavior_agrees_with_the_source` fails at suite.rs:90. CORRECTED in the third run below: the second failure is form P's, not the instruction track's (form P refuses every generated state: the generator's code page is at 4 KiB, form P's guest area starts at 64 KiB), and the two "ok" and the two long cases pass only because two refusals count as agreeing | the instruction track for the first (the recording holds 10 instructions, and none of them measures on today's generator); the owner's question below for the second |
| differential | yes: moved off the runtime's `Answer`/`Execution` loop onto `run_translated`, as run.rs | 1 passed, 0 failed, 928 s (before the unsafe move; rerun below) | |
| kernel | no | | X2 (the wasmtime executor), and the kernel taking an executor without the abi's `Cpu` (the runtime's `Executor` and the kernel's `GuestMachine::new` ask `Cpu` for `address_space` alone; form P no longer implements `Cpu`, as the brief asks) |
| busybox | no | | X3 (the unicorn executor) and the same `Cpu` bound |
| speed | no | | X2, X3 and the same bound |
| form P's other 22 binaries | yes | see below | |

`tests-on-other-tracks` now holds only kernel, busybox and speed; conformance and differential are plain test
targets.

### The Linux process API crate (the orchestrator's ruling 1)

`crates/linux-process`: `interface.rs` holds one trait `LinuxProcess` (associated `FixedMapping`, `SharedWords`),
`implementation/direct` (`DirectLinuxProcess`) holds every marked line, `conformance/suite.rs` 10 tests (7 on
x86_64, where the four ARM64 instructions do not exist). Built for Linux on both architectures (form P runs on the
Xeon too). New names in reviewable-external-names.toml.

| host-process line | What | Now |
|---|---|---|
| stub.rs:878 | `tgkill` | `tgkill`; a stub thread id that is no thread now gives `ESRCH` before any call |
| probe.rs:254, :261 | fixed `mmap` (`FIXED_NOREPLACE`) and `munmap` | `mmap_fixed_noreplace`, unmapped on the handle's drop (`ENOSYS` where an old Linux maps elsewhere) |
| pages.rs:368, :383 | atomic views of the thread pages | `create_shared_words`, `shared_words32/64`, `shared_words_file`: the memory file is now sealed against shrinking (`F_SEAL_SHRINK`), so no other holder can make a word fault |
| probe.rs:167 | `mrs ctr_el0` | `read_ctr_el0` |
| probe.rs:199 | the SVE probe's assembly (and its `compile_error!` guard) | `predicates_after_system_call` |
| code.rs:235, :244 | `dc cvau`, `ic ivau` | `dc_cvau`, `ic_ivau` |

Lines with `unsafe` (root manifest's command): host-process 9 -> 0; linux-process 0 -> 9 (direct 9, each marked,
none UNSOUND; interface and suite 0 under the workspace `forbid`).

NOT MOVED, for the orchestrator (one line): the zygote's two `global_asm!` modules (aarch64.rs 1,474 lines,
x86_64.rs 964) are form P's own program run in another process, with 223 const operands from form P's layout, page
protocol and stub descriptors; moving them into the API crate's `direct` would move that whole layout into an
outside-API crate, so host-process keeps its `deny` table for these two marks only. Proposal: either that, or a
crate `host-process-zygote` (layout plus assembly, its own `deny` table).

Measured (rented Graviton4 and Xeon, Linux 7.0.0-1018-aws, rustc 1.99.0):
- `cargo clippy --all-targets -- -D warnings` on the three linux-process crates and host-process: exit 0 on both.
- linux-process suite: 10 passed, 0 failed, three runs in a row (Graviton4); 7 passed (Xeon). Hostile cases:
  a thread id of another process refused both ways (`ESRCH`); a held page refused (`EEXIST`) and free again after
  drop (read from /proc/self/maps); a heap page refused, its bytes unchanged; a misaligned address and length 0
  refused (`EINVAL`), a page at the top of the address space refused; the shared file refuses `ftruncate` to 0
  (`EPERM`) and keeps its size; an empty memory refused; two memories apart and zeroed.
- host-process on the Xeon: every binary, 67 passed, 0 failed, 2 ignored.
- host-process on Graviton4 after the move (job native-r2d): the library and 23 binaries (every one but kernel,
  busybox, speed and conformance): exit 0, 111 passed, 0 failed, 5 ignored (measurements); differential 1 passed.
  The executor interface's own suite cases in conformance (21 of them: borrow, zero, copy, map, unmap, permissions,
  system call, fault, interrupt answers): all ok.

### RI: form I, against its bar (the orchestrator's rulings 2 and 3)

Ruling 2 (where form I's primitives go): they go in the Linux process API crate's `direct` (brief question 4's
proposal), which is ours, not an owner interface file, so no owner line is needed. Already there and reusable by
form I: fixed windows (`mmap_fixed_noreplace`), cache maintenance (`dc_cvau`, `ic_ivau`, `read_ctr_el0`). Still
to add, each one marked method: entering translated code with every register loaded and coming back (the runner's
jump), the signal stack and its handlers, the raw signal mask (glibc refuses signals 32 and 33).

Ruling 3 (own process or lock), decided by measurement: OWN PROCESS. The lock is not as safe (MEASURED today,
challenge-3-probe/lock.c, built and run in a fresh `mktemp` folder on each machine, then removed; Linux
7.0.0-1018-aws, aarch64 and x86_64, the same lines on both):
```
lock in place
(b) forked child after the lock: sigaction(SIGSEGV) = -1 errno EPERM
(c) exec'd program after the lock: sigaction(SIGSEGV) = -1 errno EPERM
(a) fault outside guest code after the lock: child ended by Alarm clock
```
With the lock, a fault outside guest code spins until a timer kills it instead of crashing, and every child the
process forks or execs after it (the zygote's stubs, any program) can no longer install a fault handler: form P
and the wasmtime executor (READ: it panics without its handlers) stop working in that process.

What "own process" means for the build (INFERRED, the design to build next): form I's runners and the kernel run in
one process of form I's own, started by exec from the embedder like the zygote, holding none of the embedder's
memory; the embedder talks to it over a socket, one crossing per container start and per arrival, not per guest
system call (the kernel is in that process too). It owns its fault handlers, so no lock.

Bar items, state:
1. Exactness: in_process_way 4 passed (MEASURED this run, on translated code in a P stub, its window at 0); the
   window-edge test on the I executor waits on the executor.
2. Isolation: in_process_attacks 2 passed (MEASURED this run, as item 1); the two-I-plus-one-P test waits on the
   executor.
3. Speculation: holds (MEASURED the run before, numbers above).
4. Cost: per-shape baseline measured the run before; the I executor's figures wait on it.
5. Conformance, kernel, busybox, differential on I: wait on the executor, and kernel and busybox also on X2, X3.
6. Crossover: waits on the executor, and on X2, X3 for busybox.
7. Unsafe only in the API crate's `direct`: holds for what is built (0 in host-process; 9 marked in linux-process).
The executor itself (the entry, signal stack and mask primitives, and the own process) is the next piece of RI.

## The conformance claim corrected, the owner's question, the zygote's crate (run of 2026-11-26, third)

### Merges (MEASURED)

His main (his checkout, fetched: ac696ea19) is already inside (`git merge-base --is-ancestor FETCH_HEAD HEAD`: 0;
`git log HEAD..FETCH_HEAD`: 0 commits), so no merge this run; it already holds the named iterators, `MemoryReader`,
`Execution<'execution>` and the processor-cores and observation-collector crates. No owner interface file changed.

### Correction: R2's conformance failures are not both the instruction track's

The second run said both failures belong to the instruction track. The independent check did not hold that, and
this run measured why (job native-r2m, Graviton4, release, `tests/moved.rs`):
- Form P refuses every state the ARM64 generator makes: on the 10 instructions the recording holds, 78,836 of
  78,836 states (READ: the generator's `CODE_ADDRESS` is one page, 0x1000, its data pages follow; form P's guest
  area starts at `LOWEST_ADDRESS`, 64 KiB, the lowest a Linux process may map, so `checked` gives `BadAddress`).
  So `measured_behavior_agrees_with_the_source` fails on form P itself, and `execute_agrees_with_execute_in_place`,
  `execution_is_deterministic` and `next_state_after_an_error...` passed only because two refusals count as the
  same outcome.
- The recording measures none of its own 10 instructions on today's generator: each ends `ExecutionFailed
  (NotRecorded)`: its states are no longer the generator's (the instruction track changed the generator on
  2026-11-25 after the recording was made). That failure, `agrees_with_precise_instruction_set` at
  `measure_recording`, is the instruction track's.

### What the owner's interfaces allow (READ, his main ac696ea19)

Nothing lets a source say which addresses its memory has, or tells a generator where to put a state:
- `BehaviorSource`: `execute_single_instruction(_in_place)`, `measure_instruction(s)`; no address item.
- `DistinguishingStateGenerator::generate_states(&self, instruction)`: no placement.
- `InstructionInputDomain::build_state(&self, instruction, operands)`: no placement; the ARM64 domain
  (`Arm64InputDomain`, a unit struct, the instruction track's) fixes its pages in constants.
- `Executor`, `MemorySpace`, `PageTable`: a range outside the space gives `InvalidAddressError`, but nothing says
  which range is inside. `ProcessorArchitecture::ADDRESSABLE` is the architecture's, the same for every source.

So no file of his changes, and the one question for the owner is below.

### For the owner (one question)

May a behavior source say which addresses a state's memory may use, so that measuring puts every generated state
there? Form P cannot map below 64 KiB (Linux's lowest address for a process) and the ARM64 generator puts its code
at 4 KiB, so today form P runs none of the generated states.

```rust
// Before (crates/behavior-source/interface.rs)
pub trait BehaviorSource {
    type Architecture: ProcessorArchitecture;
    type Error;
    // ...
        for before in generator.generate_states(instruction) {

// After
pub trait BehaviorSource {
    type Architecture: ProcessorArchitecture;
    type Error;

    /// The addresses a state's memory may use on this source.
    fn addresses(&self) -> Range<u64>;
    // ...
        for before in generator.generate_states(instruction, self.addresses()) {
```

with `DistinguishingStateGenerator::generate_states(&self, instruction, addresses: Range<u64>)` placing every state
inside `addresses`. The pending one-liner `MemorySpace::addresses()` is not the right shape for this: measuring holds
a `BehaviorSource`, not a memory space (a recording and the KVM chip source have none), and the generator is called
from `BehaviorSource::measure_instruction`. On `BehaviorSource` the same item also serves the kernel's need behind
that one-liner (the size of a space, read from the executor in `GuestMachine::new`), since `Executor: BehaviorSource`;
its doc must not say "from 0": form P's addresses start at 64 KiB.

### Form P measured with the generator moved into its guest area (MEASURED, test code of this crate)

`tests/moved.rs`: `MovedUp` wraps the ARM64 generator and moves each state up by 64 KiB: the program counter, every
memory change, the base register of an access (the generator sets it to the address less the access's offset), and
every register or exclusive-monitor address inside the generator's pages; tags and bits above 48 stay. INFERRED:
a register that holds a value in that range only as data moves too, which changes the state but not the check,
since form P and every behavior run the same moved state.

Recorded instructions (job native-r2m, 2026-11-26 11:26 UTC, 54 s):

| Instruction | States | Refused unmoved | Refused moved | Meet a known behavior | Meet none | Of those, with the tag kept | Form P measured |
|---|---|---|---|---|---|---|---|
| Rdvl | 27 | 27 | 0 | 27 | 0 | | one known behavior |
| Brk | 18 | 18 | 0 | 18 | 0 | | one known behavior |
| StpSignedOffset64 | 31,280 | 31,280 | 0 | 19,994 | 11,286 | 11,286 | no behavior agrees |
| LdrImmediatePostIndex64 | 9,024 | 9,024 | 0 | 7,200 | 1,824 | 1,824 | no behavior agrees |
| StrImmediateUnsignedOffset64 | 15,036 | 15,036 | 0 | 10,512 | 4,524 | 4,524 | no behavior agrees |
| LdrImmediateUnsignedOffset64 | 10,528 | 10,528 | 0 | 8,408 | 2,120 | 2,120 | no behavior agrees |
| AddShiftedRegister64 | 3,680 | 3,680 | 0 | 3,680 | 0 | | one known behavior |
| AddsShiftedRegister64 | 4,880 | 4,880 | 0 | 4,880 | 0 | | one known behavior |
| Csel64 | 4,360 | 4,360 | 0 | 4,360 | 0 | | one known behavior |
| MrsCtrEl0 | 3 | 3 | 0 | 3 | 0 | | one known behavior |
| total | 78,836 | 78,836 | 0 | 59,082 | 19,754 | 19,754 | 6 of 10 |

Every state that meets no behavior is a data abort through a tagged base (top byte 0xA5): form P reports the address
without its tag (0x12000), every known behavior expects it with the tag (0xA500000000012000, or 0x0500... for the
other behavior). Cause (READ): the stub's fault handler takes the address from `si_addr`, and Linux clears bits 63:56
there unless the handler is installed with `SA_EXPOSE_TAGBITS`; the aarch64 zygote's `action_flags` lack it. The fix
is that flag plus untagging the address wherever form P looks up its own pages (`resolve_fault`, `abort`'s
mapped-or-not); NOT MADE, for the orchestrator (one line): it changes the address `TrapHandler::handle_memory_fault`
receives, which the kernel turns into a guest's `si_addr`, so a guest can observe it, unless the kernel strips the
tag as Linux does for a handler without the flag.

Every instruction: see "Every instruction, moved" below. The behavior-source suite on the moved generator (the
`behavior_source` module of tests/moved.rs): `measured_behavior_agrees_with_the_source` FAILED (suite.rs:90, as the
outcomes below explain); the other three were stopped unfinished after 1 h 15 min.

### Every instruction, moved (MEASURED, job native-r2all 20261126-123210, Graviton4, release, 10 threads)

`every_instruction_measured_on_form_p_when_moved` measures each listed instruction on form P from the moved states,
a line per instruction (log kept at wcontainers-scratch/native-restart-r2c/all.txt):

| Outcome | Instructions |
|---|---|
| one known behavior | 2,881 |
| no behavior agrees | 1,530 |
| no known behavior (Hint, MsrImmediate, Sys, Sysl) | 4 |
| refused | 0 |
| never ended | 9 |
| listed | 4,424 |

The 9 that never ended: B, BCond, Bl, Cbnz32, Cbnz64, Cbz32, Cbz64, Tbnz, Tbz (the listed instructions with no
line; Br, Blr and Ret ended). After 30 minutes their 9 stubs each ran at 99.9% CPU and the 9 measuring threads
slept waiting on them; the job was stopped. INFERRED: a state whose branch targets its own instruction (offset 0) is
a loop inside one piece, which no landing ends, so the single instruction's count never stops it: a form P fault in
`run_alone`, to fix and test next. Of the 1,530 that agree with no behavior, the tagged base explains every state of
the four recorded memory instructions above; the rest are not yet sorted by cause, and some are this test's move
(INFERRED: a base in a vector register, as SVE gathers have, is not moved, and an index register that holds a
value inside the generator's pages is).

### The zygote's own crate (the orchestrator's ruling 1)

`crates/executor/implementation/host-process-zygote` (`wcontainers-executor-host-process-zygote`, commit b8fd1c392):
`layout.rs` (moved whole: where a stub's process holds its pages), `pages.rs` (the control and register pages'
offsets, `Resume`, `CommandKind`, `ExitKind`), `stub.rs` (a stub's fixed descriptors, thread flags, stop signal),
`zygote.rs` (its stack frame, the shared block's receive frame, `StubDescriptor`, `Request`, `MESSAGE`,
`ZygoteEnd`, the restartable-sequences numbers, the section name, `TOP_AT`), and the two assembly modules
`zygote/aarch64.rs` (1,474 lines) and `zygote/x86_64.rs` (964), unchanged, each under its own
`#[expect(unsafe_code, reason = "..")]`. Its manifest holds the `[lints.rust] unsafe_code = "deny"` table;
dependencies: wcontainers-abi, libc, linux-raw-sys only. host-process re-exports what its tests name
(`layout`, `pages::{control, register, Resume, ExitKind}`, `stub::{SOCKET_FD, MEMORY_FD, BLOCK_FD, STOP_SIGNAL}`,
`zygote::ZygoteEnd`), keeps the `Zygote` that starts and talks to it, and now has `[lints] workspace = true`: its own
`deny` table is gone. The two layout checks against the ARM64 output crate (the slot table, the floor entries) stay
in host-process (code.rs, pages.rs), so the zygote's crate depends on nothing of ours but the abi.

Measured at b8fd1c392 (Linux 7.0.0-1018-aws, rustc 1.99.0):
- Graviton4 (load 51 to 56): `cargo fmt --check` of both crates exit 0; `cargo clippy --all-targets -- -D warnings`
  exit 0, 0 findings; the library and 20 binaries (zygote, stub, files, host, threads, stop, touch, memory, slice,
  placement, embedder, floor, step, translated, run, interrupt, copy, forged, in_process_way, in_process_attacks):
  exit 0, 110 passed, 0 failed, 3 ignored. Before the format pass, the same with differential and the two in-process
  measurements (job of 11:36): 110 passed, 1 failed (run.rs:787 "it ran", a counted turn that made no progress at
  load 47), 5 ignored; run, zygote and stub then five times in a row: 14, 8 and 8 passed each time.
- Xeon, `--jobs 12` (load 7 to 11): clippy as above exit 0; every target of both crates: exit 0, 67 passed, 0
  failed, 2 ignored (the count of the second run's Xeon line). At 11:53 the placement test
  `the_runners_of_two_containers_take_different_cpus_and_their_threads_follow` failed 10 times in 10 on this tree
  and 3 in 3 on the unchanged 48ef08c11 ("the guest thread follows its runner", left Some(1), right Some(4)): not
  this change; it passed at 12:04.
- Unsafe (the root manifest's command): host-process 0 -> 0, host-process-zygote 0 (its two marks are
  `#[expect(unsafe_code)]` attributes, which the command leaves out, as before in host-process).
- Linter: not run this run.

### The abi `Cpu` bound (the orchestrator's ruling 2; READ at origin's tips, 2026-11-26 12:06 UTC)

- Kernel, origin/kernel-port 36bf10eed: `git grep -nw Cpu origin/kernel-port -- crates/kernel`: no line. The last
  call (`executor.address_space()` in `GuestMachine::new`) is gone in cacbe5488 ("the kernel bounds its spaces by the
  start's ceiling"); docs/status-kernel.md there still lists it as owner line 3 (not yet updated).
- Runtime, origin/runtime-restart 93637d42e (2026-11-25 23:06, older than the kernel's change): 13 lines left, all
  in single-kernel: container.rs:10, 37 (the bound) and 168 (`Cpu::address_space(&executor).end`, the one call);
  described.rs:9, 42, 53, 72, 282, 294 (bounds); implementation.rs:13, 166 (`exit(&mut self, cpu: &mut dyn Cpu,
  ..)`), 173, 174 (its own `Execution: Cpu` trait). docs/status-runtime-restart.md line 30 says the bound is kept
  only for the kernel's `address_space`, which the kernel no longer reads.
- So form P's kernel, busybox and speed tests wait on the runtime dropping those lines (and on X2, X3), not on the
  kernel. This branch does not hold kernel-port 36bf10eed (no merge was asked this run).

### Form I (RI): state

Not built this run: the correction, the measurement on the moved states and the zygote's crate took it. Nothing
of RI waits on another track. The first piece, in order:
1. The memory: one pool reserved in a process of form I's own, each memory a window of `WINDOW_BITS` (4 GiB) at
   `Way::InProcess { slot }`'s base, `MemorySpace` + `PageTable` over it with a memfd per memory as form P does
   (the embedder's view through memmap2), the window's mapping fixed with linux-process's `mmap_fixed_noreplace`.
2. The entry: a marked method in linux-process `direct` that loads a register block, x28 the register page, and
   branches to translated code at `IN_PROCESS_ENTRIES`' floor; the floor's entries come back through it.
3. Faults and stops: the signal stack and handler, and the raw mask, each one marked method there.
Then bar items 1 and 2 on it (in_process_way's window-edge test, two I memories and a P memory in one embedder).

## The last run's work pushed, R2's bar at it, form I's first piece (run of 2026-11-27)

### The last run's work, committed and pushed (MEASURED: `git status`, `git log`, `git push`)

It sat uncommitted in this worktree: 10 files, 579 lines added, 92 removed. Formatting: `cargo fmt` of the four crates
on the rented machine changed one file only (tests/interrupt.rs, a 3-line layout; md5 of every `.rs` in the six
folders before and after), fetched back with `arm-run --fetch`. The inside's native.rs and the monitor's
tests/runner/mod.rs are outside cargo fmt's packages; `rustfmt --check` on them reports only lines above this run's
one-line changes (old marked `unsafe` blocks), left as they were. Commits, pushed (origin tip ef04a4a78):
- cf7f2a7a3 a counter read's stop carries the guest's state at the read (arm64-stop; the inside and the monitor's
  test runner ignore the new field);
- afea587d8 the zygote's fault handler keeps the top byte of a fault's address (`SA_EXPOSE_TAGBITS`);
- 0f5b2ce44 form p ends a guest at its time slice (`HostProcessConfig { slice }`, default one 1000 Hz tick),
  stops at counter reads, reports a fault's tag and ends a lone branch to itself. The tag (READ, execution.rs:660,
  863, 884): the state the execution stops at keeps the chip's address with its tag; the handler is given
  `untagged(address)`, as Linux gives a handler installed without `SA_EXPOSE_TAGBITS`, so what the kernel turns
  into a guest's `si_addr` does not change;
- ef04a4a78 the executor conformance suite tests the time slice, counter reads, branches to themselves and tagged
  fault addresses (four new cases: `branch_to_itself_runs_once_alone`, `slice_ends_a_guest_that_never_traps`,
  `counter_read_ends_execute_at_the_read`, `fault_through_a_tagged_address_keeps_the_tag_in_the_state`).

### Merges (MEASURED)

His main (his checkout, fetched: ac696ea19) is already inside (`git merge-base --is-ancestor FETCH_HEAD HEAD`: 0;
`git log HEAD..FETCH_HEAD`: 0 commits): no merge. The 91 files of his 29 interface folders: 0 differ from his main.

### R2's bar at ef04a4a78 (MEASURED, Linux 7.0.0-1018-aws, rustc 1.99.0, 12 jobs)

Graviton4 (job native-restart 20261127-040352, load 27 to 40), every target of host-process by name, then the lib:
- `cargo fmt --check` of host-process, conformance, arm64-stop, the zygote and the three linux-process crates: 0;
  `cargo clippy --all-targets -- -D warnings` on the same: 0.
- 21 binaries and the lib: 134 passed, 0 failed, 3 ignored (stub 1, memory 1, floor 1). Conformance with
  `--skip behavior_source --skip agrees`: 25 passed (the four new cases among them), 5 left out: the three
  behavior-source cases and `agrees_with_precise_instruction_set`, which wait on the owner's question below and the
  instruction track's recording. Differential 1 passed (53 s). That run's `--skip agrees` also left out two of
  in_process_way's four; they ran at 72c6e0209 below: 4 passed.
- linux-process suite: 10 passed.

Xeon (job 20261127-040626, load 15 to 23; a first start at 96 jobs, the machine's default, was stopped after clippy
and run again at 12): clippy exit 0; host-process, its zygote and linux-process, every target: 74 passed, 0 failed,
2 ignored.

### Form I (RI): its first piece, the memory (commits b60f54ef8, 72c6e0209)

Built (form I's own process holds the pool, the orchestrator's ruling 3; this piece is what runs inside it):
- `crates/linux-process`: two more methods, each one marked line in `direct`: `map_file_within` (a shared mapping
  of a file at whole pages inside a fixed mapping the handle holds, `MAP_FIXED` over its own reservation only) and
  `protect_within` (`mprotect` there). Both refuse pages not wholly inside (`EINVAL`) and any executable protection
  (`EPERM`): bytes another party writes never run as this process's code. Two new suite cases.
- `crates/executor/implementation/in-process` (`wcontainers-executor-in-process`, ARM64 Linux, `[lints] workspace
  = true`, 0 `unsafe`): `InProcessPool` hands out slots 1 to `IN_PROCESS_SLOTS` (2,047), each reserved whole
  (`1 << SLOT_BITS`, 8 GiB, by `mmap_fixed_noreplace`; a slot something else of the process maps is passed over);
  `InProcessMemory` + `InProcessPageTable` implement his `MemorySpace` + `PageTable` over a 4 GiB sealed memory file
  shown in the window at the slot's base (`Way::window_base`), the 4 GiB after it never mapped (the mark lies
  there). Guest permissions give the host pages read and write only; execute gives nothing (guest code never runs
  from its own pages). Borrowed bytes go by position in the files (pread, pwrite), no view of guest memory in Rust.
  `map_from` moves the other memory's pages into a shared block (a memory file of their own) that both windows then
  show, so a memory's own file is never shown by another; `unmap_from` shows the own file again (zero there);
  `copy_from` copies data extents (holes stay holes) and shows the same blocks for mapped pages; `zero` punches
  holes. The slot is given back only after its reservation is unmapped.
- Tests (crate's own; checks read the window through `/proc/self/mem` and its protections from `/proc/self/maps`):
  tests/memory.rs 9 (zeros and nothing allowed when new, the mark's bytes never mapped; borrowed bytes are the
  window's, across pages and at the window's last page; permissions per page; two memories apart; addresses past
  the window, reversed or unaligned refused; zero; mapping both ways, a mapping of a mapping, permissions kept;
  unmapping; copy of bytes, holes, permissions and mappings), tests/slots.rs 1 (every slot taken once, `NoSlot`
  after, given back unmapped and taken again).

Measured at the pushed 72c6e0209 (job native-restart 20261127-041727, Graviton4, load 50 to 113): fmt and clippy
of the eight crates exit 0; in-process 10 passed (`slots taken 2047, left out 0`, run before); linux-process 12
passed; host-process 136 passed, 0 failed, 3 ignored (21 binaries, conformance as above, the lib); in_process_way 4,
in_process_attacks 2. Ten runs in a row of the in-process and linux-process tests: every one passed, after a race
of this run's own was fixed: the first version of the two new suite cases searched for pages where an older case
does and, run in parallel, mapped the page that case had just checked was unmapped (1 failure in 3); each new case
now searches a region of its own. Xeon (job 20261127-041733): clippy exit 0, every target 76 passed, 0 failed (the
in-process crate is empty there until W3).

Not run: `cargo reviewable check` (no linter build on the machine this run); the linter's findings on the new crate
are unknown.

RI's bar, state: items 1 and 2 (window edges on the I executor; two I memories and a P memory in one embedder)
need the executor; the memory's part of item 2 holds for two I memories (two_memories_have_windows_apart,
the mapping and copy tests). Item 3 holds (run of 2026-11-26). Items 4 to 6 wait on the executor, 5 and 6 also on
X2, X3. Item 7: holds for what is built (0 in in-process; linux-process 11 marked, none UNSOUND).

Next piece, waiting on no other track: the entry (a marked method in linux-process `direct` that loads a register
block, x28 the register page, and branches to translated code at `IN_PROCESS_ENTRIES`' floor, the floor's entries
coming back through it), then faults and stops (signal stack, handler, raw mask), then `Executor` and
`ExecutionInterrupter` written as a loop of "run until the next trap, then ask the handler".

For the owner, still open (written above, "For the owner (one question)"): may a behavior source say which addresses
a state's memory may use (`BehaviorSource::addresses()`)?

## Form I's entry: code runs in this process until it calls the floor (run of 2026-11-27, second)

### The start of this run (MEASURED: `git status`, `git log`, `git fetch`)

The work the brief called uncommitted was already committed and pushed by the run before (origin tip 03b15216f, the
section above); the worktree was clean. His main (fetched: ac696ea19) was already inside; the owner's interface files
(every `crates/*/interface.rs` his main holds) are byte-identical to his main (`git diff --quiet`: 0). The four
interface files this branch adds (kvm, hypervisor-framework, linux-process, linux-socket-options) are the tracks'
API crates, not on his main.

### Built (commits c019fd1cd, 041ebbda8, b7c8eae95; pushed, origin tip b7c8eae95)

- `crates/linux-process`, three more items. `map_code(address, code)`: a memory file of the bytes, sealed against
  shrinking, growing and every write, then mapped shared, readable and executable, `MAP_FIXED_NOREPLACE` (one marked
  line): no one, this process included, can change code that runs. `exit_code()`: the 16 bytes of an exit (`stp x23,
  x30, [x28, #256]`, `ldr x23, [x28, #272]`, `blr x23`, `udf`). `run_code(&mut RunFrame)`: one naked function of
  assembly, which keeps x19 to x30, d8 to d15, FPCR and TPIDR_EL0 on the thread's stack, loads every register from
  the frame (x28 the frame, x23 the code's first address) and branches; an exit saves every register the code left
  into the frame, puts the thread's back and returns the exit's address. New plain types in the interface:
  `Arm64Registers`, `RunFrame` (32 words the code owns, at x28 + 0, then 4 words of the run, then the registers).
- `crates/executor/implementation/in-process`, module `run`: `floor()` maps one page of exits at the in-process way's
  entries (`IN_PROCESS_ENTRIES`' page), once per process; `InProcessThread` holds a frame, `enter(way, state, page,
  float, at)` takes the stop crate's `in_process::rejoin`, `run()` gives `Exit::Call(code)` from the exit's address,
  and `hardware()`, `page()`, `after()` give what `in_process::resolve` reads. Compile-time checks tie the exits to
  the way: x28 is `PAGE`, x23 is `SCRATCH`, an exit is one entry, the page and the 4 words after it fit the frame.
- No new trait. No owner file changed. The `Executor` itself is not written yet: the run is the step it loops on.

### Tests (MEASURED)

- linux-process suite, 2 new: `maps_code_no_one_can_write` (refuses an unaligned address, part of a page, nothing:
  `EINVAL`; a mapped place: `EEXIST`; maps `r-xs`; a write through `/proc/self/mem` is refused and the bytes stay;
  the drop unmaps); `runs_code_until_an_exit_and_keeps_the_thread_s_own` (code that changes TPIDR_EL0, FPCR's rounding
  mode, sp, d8 and x19, then exits: every register in the frame is what it left, x23 the exit, x28 the frame, pc and
  x30 after the branch, the frame's own words untouched; after it the test thread's thread-local reads its value and
  1/3 rounds as before; then two threads run it at once).
- in-process tests/run.rs, 3 new, on translated code of the in-process way run natively in this process
  (`ldr x1, [x2]`, `add x3, x1, #5`, `str x3, [x2, #8]`, `add x15, x24, #1`, `movz x8, #172`, `svc #0`):
  each resolves through `in_process::resolve` to `Stop::Syscall` with every one of the 31 registers the guest's
  (x15 and x24, which the way withholds, among them), pc after the `svc`, sp unchanged, the store in the memory's
  window. `two_memories_run_apart_each_in_its_own_window` (slots of one pool, runs A, B, A: each reads and writes only
  its own); `a_thousand_runs_leave_the_thread_its_own` (1,000 runs on one thread, the guest's thread pointer kept
  apart from the thread's, its thread-local and rounding as before).
- A race in the old suite fixed: `refuses_a_page_at_a_bad_address_or_length` searched the same page as
  `maps_a_fixed_page_and_unmaps_it_on_drop` and, run at once, mapped the page the other had just checked was unmapped
  (1 failure, job 20261127-061904); it now searches its own region.

Before the push (Graviton4, job 20261127-062112, load 43 to 46): `cargo fmt --check` of the four crates 0; clippy
`--all-targets -D warnings` of them and host-process and its zygote 0; the in-process and linux-process tests ten
times in a row, each time memory 9, run 3, slots 1, linux-process 14 passed, 0 failed. Xeon (job 20261127-062129):
clippy 0; linux-process 10 passed, three times.

At the pushed b7c8eae95 (Linux 7.0.0-1018-aws, rustc 1.99.0, 12 jobs), linux-process's three crates, in-process,
host-process, its zygote and arm64-stop; tests with `--skip behavior_source --skip
agrees_with_precise_instruction_set` (the cases that wait on the owner's question and the instruction track):
- Graviton4 (job 20261127-062450, load 43 at the start, up to 198 from other jobs): `cargo fmt --check` 0; clippy
  `--all-targets -D warnings` 0; tests exit 0, 43 test binaries, 164 passed, 0 failed, 6 ignored (1,377 s, most of it
  differential in this build).
- Xeon (job 20261127-062456): clippy 0; tests exit 0, 77 passed, 0 failed, 2 ignored (in-process is empty there).

Not run: `cargo reviewable check` (its naming rules ask a decision model over the network; no build of it on the
machines this run); the hostile-guest suite as a whole (in_process_attacks ran in host-process above: passed).

### Security (MEASURED, the root manifest's command, and `#[expect(unsafe_code` marks)

- linux-process: lines 11 -> 12 (`map_code`'s mmap), all in `direct`; marks 11 -> 13. The 13th is the attribute
  `#[unsafe(naked)]` of the run, which the command leaves out (it starts `#[`); its mark sits on the module `entering`,
  since rustc refuses `expect` on a naked function (E0736). That mark is UNSOUND: a safe fn runs whatever code the
  frame's pc names on its own stack; only the translated code's promise (it reaches its window and its frame alone)
  keeps this process's memory as Rust holds it. FOR THE ORCHESTRATOR (one line): `run_code` is safe by signature and
  UNSOUND by Rust's rules, as every entry into other code is; ruling 2 put it here; say whether an UNSOUND mark is
  acceptable for it, or which form you want.
- in-process 0 -> 0; host-process 0; zygote 0 by the command (its two marks unchanged); arm64-stop 0.
- New dependencies: none from outside; in-process now depends on arm64-stop (ours), and in its tests on the
  translator, guest-cpu and arm64 (ours).
- A signal that reaches a thread while its guest code runs is taken on the guest's stack inside the window: the next
  piece (the signal stack) closes that. Until then nothing of form I runs outside these tests.

### RI's bar, state

Item 1: the four seeded in_process_way tests run on unicorn and in a P stub; on form I natively, the six instructions
above pass; the window-edge test needs faults (next piece). Item 2: two I memories each see only their own bytes,
natively (above); a P memory beside them in one embedder waits on the executor. Item 3 holds. Items 4 to 6 wait on
the executor, 5 and 6 also on X2, X3. Item 7: holds (0 in in-process; linux-process 13 marks, 1 UNSOUND, above).

Next piece, waiting on no other track: faults and stops (a signal stack per thread and the fault handler, both marked
methods in linux-process `direct`, the raw mask), so that the mark, a guest fault and an unmapped slot end a run as a
`Taken::Access`; then `Executor` and `ExecutionInterrupter` written as a loop of "run until the next trap, then ask
the handler"; the slot table's pages (map_code within a reservation) for pieces that jump.

For the owner, still open (written above): may a behavior source say which addresses a state's memory may use
(`BehaviorSource::addresses()`)?

## Form I's faults, stops and executor (run of 2026-11-27, third)

### Start (MEASURED: `git fetch` of his checkout, `git merge-base --is-ancestor`, `git diff --quiet`)

His main is still ac696ea19 and already inside (ancestor: 0); every owner `crates/*/interface.rs` is byte-identical to
it. The four interface files his main lacks (kvm, hypervisor-framework, linux-process, linux-socket-options) are the
tracks' API crates.

### The check's weak tests, planted again (MEASURED, mutants in job native-ri3-mut on Graviton4)

- Rounding: both run tests now compute 1/10, which rounds to 0x3fb999999999999a to nearest and one unit lower toward
  zero, and read FPCR itself back (`LinuxProcess::read_fpcr`, one marked line) before and after; the guest's mode in
  the runs is toward zero. Mutant: FPCR's restore (`msr fpcr, x9`) deleted: `runs_code_until_an_exit_and_keeps_the_thread_s_own`
  FAILED (exit 101).
- Write seal: `maps_code_no_one_can_write` now also takes the code's file (`LinuxProcess::code_file`) and expects the
  seal listed, a write refused (`EPERM`) and a second, writable shared mapping of the file refused (`EPERM`, memmap2's
  safe `map_raw`). Mutant: `SealFlags::WRITE` removed: the test FAILED (exit 101). An `mprotect` of the mapping itself
  has no test: no safe call reaches it from a test under `forbid`.

### The UNSOUND mark (the orchestrator's ruling)

- `InProcessThread::enter` and `run` are private to the executor crate: module `run` is no longer public, and the
  executor (below) is its only caller. No safe public method of the executor runs code a caller names.
- `LinuxProcess::run_code` keeps its one UNSOUND mark. Its obligation is written in its doc and in the mark: the code
  reads and writes no memory but the mappings made for it and the frame's own words; its stack is its own, whatever its
  stack pointer holds; it makes no system call; it changes the general and vector registers, NZCV, FPSR, FPCR and
  TPIDR_EL0 and no other register user space can write (TPIDR2_EL0, SVCR, DIT, SSBS, TCO, the SVE bits above 128);
  x28 holds the frame at each of its instructions.
- For the owner (policy, not a design change here): may a safe method of an outside API crate run machine code its
  caller names, or must it take a value only the translator can make? Today: a safe method, marked UNSOUND.

### RI's third step: signal stack, handlers, raw mask (commits 00a7791bd, 45bdc04f0)

In `crates/linux-process` (`direct`, each use one marked block):
- `prepare_run_thread()` gives a `RunThread` (not `Send`): a signal stack of its own (a guard page, then a record
  page at the stack's bottom, then at least 256 KiB, or 8 times AT_MINSIGSTKSZ), the run's handlers installed once per
  process (SIGSEGV, SIGBUS, SIGILL, SIGTRAP with `SA_ONSTACK | SA_NODEFER`, the stop `RUN_STOP_SIGNAL` = SIGUSR1 with
  `SA_ONSTACK`, each blocking the stop), and every signal blocked but those and SIGFPE by the raw `rt_sigprocmask`,
  which reaches 32 and 33. Its drop puts the earlier stack and mask back. `EBUSY` for a second one on a thread.
- `run_code(&mut RunThread, &mut RunFrame) -> RunEnd`: `Exit(address)`, `NotEntered`, `Stopped`, `Fault { address,
  write, alignment }`, `Raised(signal)`. The run writes into the record where its parts lie, then the frame; the
  handler finds the record by the signal stack's address the kernel hands it (never through the thread pointer, which
  may be the guest's; it touches no thread-local and sets no errno). A stop before the code starts ends the run
  `NotEntered` (also one that came before the run); in the code, `Stopped` with every register; in an exit or the
  run's own saving, it waits for the next run; a fault in the code ends the run with ESR's WnR from the frame's record
  and BUS_ADRALN for alignment; a fault anywhere else goes back to the handler the process had before.
- In-process `run`: a stop inside a floor entry before it changed a register is the call it was making; an alignment
  fault at sp or pc is `Taken::StackAlignment` / `JumpAlignment`; every other fault `Taken::Access`; the mark, a guest
  fault and an unmapped slot each end the run as `Taken::Access`.

Tests (linux-process suite, now 18): faults of a load and a store at an unmapped page, a load through a misaligned sp,
a branch to a misaligned address, `udf`, `brk` (registers at the instruction, the thread's own kept; an exclusive load
of a misaligned address did not fault on Graviton4, so it is not used); a stop from another thread while code runs
`b .` with its sp pointing into this process's memory (a pattern), at an unmapped page and at 0; a stop before the run;
the mask (`SigBlk` of `/proc/thread-self/status`: all but the eight, 32 and 33 among the blocked; another thread's
unchanged; back after the drop; `EBUSY`).
Mutants (MEASURED): stop handler without `SA_ONSTACK`: the stop test FAILED (the pattern was written); no signal stack
at all: the test binary died of SIGSEGV; the mask not set: the mask test FAILED.

### The executor and its interrupter (commits c11e93117, 1c22d1349)

`execution.rs`: `InProcess` implements `Executor` and `BehaviorSource`; `InProcessExecution::run` is a loop of
"run until the next trap, then ask the handler" (`Run::until_trap`, `ask_handler`), so a synchronous `run_until_trap`
is a small change. The reference path: every guest instruction is a piece of its own (`LookAhead::None`), translated
for the in-process way, cached by slot, displacement, address and instruction word, and mapped sealed in a code area;
no slot table page is mapped, so every exit faults in the slot table and lands. Each slot's slot table (2^35 bytes)
and displaced code's (slot 0's) are reserved inaccessible, so nothing else of the process can be mapped where code
jumps. `InProcessInterrupter` asks before the next entry and sends the stop to the thread; a slicer ends a slice of
1 ms with no trap or translation as an interrupt. A fault's tag comes from the access's operand (the in-process way
drops it before the host access, so no host fault reports it).

Tests (in-process, run 9, memory 10, slots 1), all through the executor: system call and its return; two memories;
1,000 executions keep the thread's own (thread-local, FPCR, a tenth); faults (read of a page that allows nothing, write
of a read-only page, a jump to a page without execute; retry after the handler allows the page); **every access form
outside its window is stopped**: 18 forms (ldr base, immediate, unscaled negative, register, shifted register,
sign-extended word, pre- and post-index, ldp, ldrb, str, stp, ld1, st1, ldxr, stxr, ldar, a load through sp) at 4
targets each (the other memory's data by its host offset, this window's guard, 2^40, below 0): 72 stopped, each a
fault at the guest's address, the other memory's data unchanged; signals while guest code runs with a hostile guest
sp (into its own writable page holding a pattern, into a page it may not touch, outside its window): the pattern is
untouched; sp moved out and back in; an interrupt before the run; the slot tables held inaccessible.
The outside test found a real fault: a vector structure store outside the window was given to the handler as a read
(fixed: `SimdMemory { load: false }` is a store).
Mutants (MEASURED): fault handlers without `SA_ONSTACK`: the run binary died of SIGSEGV; the vector store read: FAILED;
displaced code not run displaced: hangs (timeout 300 s, exit 124); the request not taken before entry: hangs (124);
the slot table not reserved: FAILED.

The executor suite (`conformance_run.rs`, against the Neoverse V2 recording, MEASURED): 25 cases pass, among them
the slice, a branch to itself alone, a fault through a tagged address, a counter read, interrupts answered continue.
Fail: `agrees_with_precise_instruction_set` and `behavior_source::measured_behavior_agrees_with_the_source` fail in
the recording's own measurement ("an instruction shows no one known behavior"), the two form P's bar skips too;
`interrupt_after_the_end_does_nothing`: the reference path's one fault per instruction makes even one count down
outlast the 1 ms slice. `behavior_source::execute_agrees_with_execute_in_place` and `execution_is_deterministic` did
not finish in 30 minutes at that speed. Linking pieces through slot table pages is what removes both.

### Off the legacy `abi` crate

`PAGE_SIZE` is gone from in-process (its own `GUEST_PAGE`, with its link). `Permissions`, `Access` and
`SystemCallRequest` stay: his executor interface names them (`PageTable::set_permissions`, `TrapHandler`).

### The bar at the pushed 1c22d1349 (MEASURED, Linux 7.0.0-1018-aws, rustc 1.99.0, 12 jobs)

Crates: linux-process (interface, direct, conformance), in-process, host-process, its zygote, arm64-stop. Tests skip
`behavior_source`, `agrees_with_precise_instruction_set`, `interrupt_after_the_end` and `interrupts_answered_continue`
(the names above; the last two also skip form P's copies, which passed at b796f5c04: 43 binaries, 175 passed, 0
failed, 6 ignored, job 20261127-133632).
- Graviton4 (job native-ri3 20261127-150123, load 82 to 103): `cargo fmt --check` 0; clippy `--all-targets -D warnings`
  0; form I and linux-process five times in a row, each time suite 23, run 9, memory 10, slots 1, linux-process 18
  passed, 0 failed; all tests exit 0, 44 binaries, 195 passed, 0 failed, 6 ignored.
- Xeon (job 20261127-150128): clippy 0; tests exit 0, 77 passed, 0 failed, 2 ignored (form I is ARM64 only).

### Linter (MEASURED: `cargo reviewable check` from reviewable-rust da55b9d7 built on the machine, no model key, so the
naming and summary questions are unanswered; findings in linux-process and in-process)

At d19523a8c (before this run): 566. At 1c22d1349: 875. By rule at 1c22d1349: magic_literal 288, number_constant_without_link
86, summary_not_one_line 84, name_unanswered 80, summary_unanswered 75, function_summary_not_third_person 68,
public_outside_interface 52, summary_too_long 42, conformance_not_run 30, constant_defined_twice 25,
summary_not_one_sentence 16, conformance_untested_method 8, comment_too_long 5, interface_item 4,
summary_starts_with_name 4, hook_name 2, implementation_folder 2, verb_not_first 2, comment_budget_exceeded 1,
trait_not_implemented 1. Most magic literals are the run's assembly offsets and the tests' instruction encodings. Not
cleaned this run.

### Security (MEASURED: the root manifest's command, `git grep -nw unsafe <rev> -- '<folder>/*.rs' | grep -vE ...`)

| Crate | unsafe lines d19523a8c | 1c22d1349 |
|---|---|---|
| linux-process (all in `direct`) | 12 | 25 |
| in-process | 0 | 0 |
| host-process | 0 | 0 |
| host-process-zygote | 0 | 0 (its 2 marks unchanged) |
| arm64-stop | 0 | 0 |

The 13 new blocks, each marked with its reason: the signal stack's map, its guard, its record, the stack and raw
mask set, their restore on drop, the handlers' install, the earlier handler put back, the handler's five (its
arguments as references, the fault address, the frame's records, the stack's record, the running frame), and
`read_fpcr`. Plus the naked run's attribute, under the module's one UNSOUND mark (unchanged in number). No lint table
lowered, no exclusion; every crate keeps `[lints] workspace = true` but `direct`'s existing deny table. New
dependencies: none from outside (in-process now uses aarch64-cpu, already in the lock, and our crates; the
linux-process conformance crate uses memmap2, already in the lock). Hostile-guest tests: in_process_attacks passed in
the bar; on form I itself the outside-window test (72 accesses, 18 forms) and the hostile-stack signal test pass.

### RI's bar, state

Item 1 (exactness at window edges, seeded): not yet on form I. Item 2: two I memories apart and every access form
stopped at the other's data (above); a P memory beside them waits. Item 3 holds. Item 4 (cost) waits on linked
pieces. Item 5: the executor suite runs on I (25 pass; three fail as above); kernel, busybox and differential tests
wait on X2, X3. Item 6 waits on the same. Item 7: unsafe only in linux-process `direct`, each marked, one UNSOUND.

Next, waiting on no other track: slot table pages (code mapped within the reserved table, one sealed page per 256
slots, each slot unlinked when its line is announced changed or its page loses execute), so pieces link and the
suite's count-down cases finish; then look-ahead beyond one instruction, and the window-edge exactness test.

## The owner's `run_code` ruling, the check's plants, one operation per mark (run of 2026-11-27, fourth)

### Start (MEASURED: `git fetch` of his checkout, `git merge-base --is-ancestor`, `git diff --quiet`)

His main is still ac696ea19, already merged (`merge-base --is-ancestor` 0). Branch `translator-confinement` 5db90a9ff
merged (a fast-forward: it was cut from this branch's tip) for `TranslatedInstructions`. Every owner `interface.rs` that
main has is byte-identical to main's.

### The ruling: `instructions`, the mapping a run takes, a `pc` outside it refused (commits 7f43c42d7, 7d8640f4e)

- linux-process: `CodeMapping` is `InstructionMapping`, `map_code` is `map_instructions(address, instructions,
  window, displaced)`, `code_file` is `instructions_file`, `exit_code` is `exit_instructions`, `run_code` is
  `run_instructions(thread, &InstructionMapping, frame)`; every doc line says instructions. New `RunEnd::Refused` and
  `WINDOW_BASE_REGISTER` (x24).
- `run_instructions` refuses, running nothing and changing nothing, a `pc` that is not a 4-byte-aligned address inside
  the mapping, and for instructions not displaced a stack pointer (top byte ignored) outside the mapping's window; it
  sets x24 to the window's base itself (FOR-THE-NATIVE-TRACK.md (b), (c)).
- The executor translates with `translate_instructions` and maps `TranslatedInstructions::padded`, each piece with its
  window and displacement; `Loaded` holds the `TranslatedInstructions`.
- The pool reserves 64 KiB below every window (FOR-THE-NATIVE-TRACK.md (d)): a slot is `[base - 64 KiB, base + 2^33
  - 64 KiB)`, so a window's lower guard is its own whether the slot below is held or not. The test checks it for slot 1
  and another, held and given back.
- Not done, and why: `Entry::Start | Resume(Stopped)` (refusal by construction, entry only at a piece's start or a stop
  point). The executor's resumes (`Resolved::Again` and the others) re-enter at a `pc` the stop crate computes with
  answered registers; carrying a `Stopped` token through them is the next run's change. Until then a caller can still
  enter mid-piece inside the mapping. The one UNSOUND mark stays, its obligation narrowed to that and to the bytes
  being the translator's: linux-process cannot name `TranslatedInstructions` (it is below the translator), so
  `map_instructions` takes bytes. For him, one line: may `TranslatedInstructions` live where linux-process can name it
  (or linux-process take a sealed token the translator crate makes), so that `map_instructions` takes only it?

### The check's surviving plants, each with a test (MEASURED, job nr-hand on Graviton4, each plant 3 rounds)

| Plant | Test that catches it | Rounds caught |
|---|---|---|
| L12 `stp q0, q1` of the exit deleted | `takes_every_register_in_and_gives_every_register_back` (every x +1, x29 += x30, every v inverted) | 3 of 3 |
| I7 interrupt asked before the run not checked | `an_interrupt_asked_before_the_execution_is_taken_first_by_a_guest_that_keeps_making_system_calls` | 3 of 3 |
| I11 the write of an atomic to a read-only page | `a_fault_is_a_write_or_a_read_as_linux_reports_it` (ldadd, dc zva on READ_ONLY and outside, ic ivau) | 3 of 3 |
| I6 code change announced on a forbidden page goes on | same test, `ic ivau` on NOTHING, and goes on on READ_ONLY and DATA | 3 of 3 |
| L3 a stop in the entry not aborted (timing) | `keeps_every_register_through_stops_that_land_anywhere` (2,000 stops at least) | 3 of 3 |
| I9 a stop in an exit on the floor page (timing) | `interrupts_that_land_anywhere_change_nothing_the_guest_sees` | 2 of 3 at 2,000 calls; lengthened |
| dc zva not a write (check item 3) | `a_fault_is_a_write_or_a_read_as_linux_reports_it` | 3 of 3 |
| no guard below the window | the slots and memory tests | 3 of 3 |

The check's other items: I10 (`JumpAlignment`) is unreachable and removed (a misaligned pc is found before entry; a
piece branches only to aligned addresses), and with it the alignment-at-pc arm of the stop; the host's write bit is no
longer used (the instruction decides, `access_of`, atomics and `dc zva` now writes, as Linux's `is_write_abort`); a
foreign fault or stop now goes to the handler the process had before (a SA_SIGINFO handler is called, a default or
ignore is put back for a fault), and a run's handler stays: `earlier_handlers` test, own process, with signal-hook's
safe handlers. Not this track's and left: `dc cvau` on a forbidden page (the translator's `CleanCache` copy), the
whole-workspace `cargo fmt` diffs in other crates, form P's load-dependent flake in `moved.rs`.

### Mutants (MEASURED: cargo-mutants 27.1.0 on Graviton4 only, each copy building in its own folder; not in any lock)

A first run (nr-mut0) shared one build folder between copies, so its verdicts are void; it was stopped. The valid runs:

- in-process `execution.rs`, `run.rs`, `memory.rs`, against tests run, memory, slots and behavior (the executor
  conformance suite is too long per mutant): at 2fd27f085 74 missed of 366; at 3eeb1ec26 15; at 0132e8f2c (job
  nr-mut3) 310 mutants, 210 caught, 13 timeouts, 73 unviable, **14 missed**.
- linux-process `direct/implementation.rs`, against its conformance suite and the earlier-handler test: at 0132e8f2c
  99 of 262 done when the machine reached a load of 424 and the job was stopped: 52 caught, 36 timeouts, 18 unviable,
  **11 missed**.

Each survivor at 0132e8f2c, and what was done:

| Survivor | Verdict |
|---|---|
| execution.rs:52 `+`→`-`, `<<`→`>>` (the instruction area's start) | equivalent: any start outside the windows, tables and floor serves; the test that pieces lie outside every slot and table catches the one that is not |
| execution.rs:160, 162 (the next free place's bound and step) | equivalent: allocation order and the area's 2^40 bytes, never filled by a test |
| execution.rs:505 the landing guard made false | not equivalent: a two-piece loop then takes seconds, not a slice each; the test now bounds it (5 s), after this run's mutants |
| execution.rs:643, every linux-process `|`→`^` (11 sites) | equivalent: each `|` joins disjoint bits (flags, instruction fields, a tag on an untagged address) |
| memory.rs:200 `Block::eq` → false | equivalent: only stops adjacent equal ranges from merging; the `true` mutant is caught by the side-by-side blocks test |
| memory.rs:284, 298 `<`→`>` (stop after a short read or write) | equivalent: a memory file's reads and writes below its end are whole |
| memory.rs:746 `<`→`<=`, 760 `-`→`/` (copy loop) | equivalent: one more empty turn; a slower copy of the same bytes |
| run.rs:36 `/`→`%` | equivalent: weakens a compile-time assertion only |
| run.rs:143 `&`→`^` (the floor page of a stopped pc) | equivalent for every pc the test reaches; the floor-stop arm itself is caught by the interrupt storm |
| run.rs:181 `resume` → () | not equivalent, and a real defect behind it: checking every general register after `msr`/`mrs fpsr` FOUND that the guest's x30 came back as a host address (0x2004_0000_..., this process's instruction area). Form I gave the stop crate a call's pc after its branch into the floor; form P and the stop crate's `after_call` take it at the branch, so the resume skipped the piece's reload of x30 from the register page. Fixed in run.rs (the pc at the branch, as form P's `h.pc - INSTRUCTION`); a guest-visible register and a leak of a host address to the guest (SECURITY) |
| direct:115 `KEPT_BYTES` `+`→`*`, 135 `1 << 18`→`>>` | equivalent: a larger save area; the signal stack's floor from AT_MINSIGSTKSZ |

### Assembly plants (MEASURED, docs/plants-linux-process-run.md, job nr-asm at 0132e8f2c)

156 plants, one per instruction line of the naked run: 95 caught by a failing test, 55 past the 150 s limit (each of
those copies' test binaries was still spinning more than three hours later, found and killed at 00:15: a hang, so
caught; the driver now kills a timed-out copy's whole process group), 6 missed: the saves and restores of the thread's d8 to d15 (5 lines; a
caller's values in those registers are seen only by code built with optimization, and the tests are built without),
and the exit's save of FPSR (the every-register test now sets every FPSR flag, after the plant run).

### Two stop-storm defects in this run's own test (MEASURED, fixed in a4185d5b8 and fe315a8f8)

The first form of `keeps_every_register_through_stops_that_land_anywhere` ran a stopped run on from its frame. It hung
in the bar for 36 minutes: a stop between `add x23` and `blr x23` resumed with x23 the first address again (the run
sets x23 at every start), so `blr x23` branched to itself. The executor never does that (a stopped piece starts again
at its first instruction); the test now does the same and checks the registers at each stop. A stop sender with no
pause also starved the runner on a loaded host; it now pauses between stops. The L3 plant is caught 3 of 3.

### The bar at the pushed fe315a8f8 (MEASURED, Graviton4, job nr-bar 20261128-033952, load 105 to 160, 12 jobs)

Crates: linux-process (interface, direct, conformance), in-process, host-process, its zygote, arm64-stop. Skips:
`behavior_source`, `agrees_with_precise_instruction_set`, `interrupt_after_the_end` (the check's run showed
`interrupts_answered_continue_change_nothing` passes; its skip is gone).
- `cargo fmt --check` 0; clippy `--all-targets -D warnings` 0.
- All tests: 46 binaries, 224 passed, 1 failed, 6 ignored. The failure is form P's `recorded_instructions_on_form_p_when_moved`
  (StpSignedOffset64: 1 of 31,280 moved states meets no known behavior), host-process code this run did not change: the
  load-dependent flake the check found.
- in-process and linux-process five rounds in a row: 89 passed, 0 failed each round.
- Xeon at 0132e8f2c (job nr-bar-x86 20261127-211010): fmt 0, clippy 0, 46 binaries, 77 passed, 0 failed, 2 ignored.
  The commits after it change only ARM64-only code and docs (INFERRED); the machine could not be reached afterwards.

### Linter

Not measured this run: the Xeon, where it was building, went away mid-build, and the ARM64 machine stayed above a load of
150 for hours. 875 findings at 1c22d1349 (last run); not cleaned.

### Security (MEASURED: the root manifest's command; `#[expect(unsafe_code` marks)

| Crate | unsafe lines 21d82e37e | fe315a8f8 |
|---|---|---|
| linux-process (all in `direct`) | 25 | 30 |
| in-process | 0 | 0 |
| host-process | 0 | 0 |
| host-process-zygote | 0 | 0 |
| arm64-stop | 0 | 0 |

Each of the 30 is one operation under its own `#[expect(unsafe_code, reason = ...)]` whose reason is its obligation
(31 marks: those and the `entering` module's, over the naked run's attribute, the one UNSOUND mark). The 5 more: the
blocks that did two or more things are split (stack and mask set, their put-back, the handler's two arguments), the
handler install is now one raw `rt_sigaction` (no `zeroed`, `sigemptyset`, `sigaddset`), and the chain to an earlier
handler takes two `transmute`s (one per handler form). New dependencies: none from outside (signal-hook 0.3.18, already
in the lock for host-process, now a dev-dependency of `direct`). No lint table lowered, no exclusion; the two
`cfg_attr(not(aarch64), expect(dead_code))` are on fields only ARM64 reads. Hostile-guest tests: in_process_attacks
passed in the bar; on form I the outside-window test (18 forms x 4 targets) and the hostile-stack signal test pass.
Security fixed this run: a guest's x30 no longer comes back as this process's instruction address after a float status
access; 64 KiB below every window are held (the confinement track's measured reach below a base); a foreign fault no
longer turns form I's handler off for good; atomics and `dc zva` outside the window are writes.

### Next

Waiting on no other track: `Entry::Start | Resume(Stopped)` in linux-process (refusal by construction; a resume keeps
x15, x23, x30 and the piece's kept registers, FOR-THE-NATIVE-TRACK.md section 1), carried through the executor's
resumes; then RI's slot table pages (linked pieces), look-ahead, the window-edge exactness test; the linter clean-up.
