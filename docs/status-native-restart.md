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

## R1: the mapping rule for `crates/kvm` and `crates/hypervisor-framework` (proposal; nothing built; STOP for it)

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
