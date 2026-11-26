# x86 instruction forms

Every Intel XED instruction form (iform) that can execute in 64-bit mode, from XED v2026.08.23.

| Count | Forms |
|---|---:|
| **In the list** (`Instruction`, `instruction-forms.jsonl`) | **8,881** |
| Encodings of those forms (XED records valid in 64-bit mode) | 10,571 |
| Valid in 64-bit mode only / also in 16-bit and 32-bit modes | 1,844 / 7,037 |
| Ring 3: unprivileged / faults / depends on the privilege level | 8,715 / 93 / 73 |
| Encoding: EVEX / legacy / VEX / XOP | 5,222 / 1,870 / 1,628 / 161 |
| Categories (XED's class) / extensions / ISA sets | 112 / 109 / 284 |
| Left out: only valid in 16-bit or 32-bit mode | 120 |
| All XED iforms (the list plus the left-out ones) | 9,001 |

Largest extensions: AVX512EVEX 3,915; APXEVEX 1,136; AVX 702; BASE 693; AVX2 286; SSE2 272; FMA 192; XOP 153; X87 146; MMX 132.
Largest categories: AVX512 1,579; CONVERT 860; DATAXFER 620; VFMA 528; APX 505; AVX 471; SSE 386; LOGICAL 346; BINARY 346; FP16 296.
Recount any of these with `jq`, for example `jq -r .extension instruction-forms.jsonl | sort | uniq -c`.

## Sources

| Source | Version | License | License text | Used for |
|---|---|---|---|---|
| Intel XED | v2026.08.23 | Apache-2.0 | `LICENSE-XED` here (XED's `LICENSE`) | every record |
| Intel mbuild | v2026.08.23 | Apache-2.0 | mbuild's `LICENSE` | running XED's generator |
| serde, serde_json | 1.0.229, 1.0.154 | MIT OR Apache-2.0 | each crate's license files | generator and tests |
| Intel SDM, AMD64 manual (prose) | current | read only | | ring-3 behavior of 80 forms XED marks CPL 3; CPUID bits of the 329 AMD-only forms |

Nothing comes from GPL, LGPL or AGPL code or from Arm's machine-readable files.

## The record

One JSON line per form: `name` (the variant), `iform`, `mnemonic`, `iclass`, `category`, `extension`, `isa_set`, `cpuid` (alternatives of CPUID bits; empty when none is needed), `ring3_behavior` (`unprivileged`, `faults` or `depends_on_privilege_level`) with `ring3_behavior_reason` where it is not XED's CPL field alone, `xed_cpl`, and `encodings`.
Each encoding has the space, opcode map, opcode, mandatory prefixes, REX.W, vector length, ModRM constraints, REX2 rule, modes, XED's pattern verbatim, operands (kind, register, visibility, read and written, width), flags (read, written, undefined, cleared, set) and attributes.

## Granularity: iforms against iced-x86 `Code`

iced-x86 1.22.0 has 4,935 `Code` values besides `INVALID`, 4,695 of them valid in 64-bit mode.
Decoding 506,404 byte strings with both (`../x86-iform-code-mapping`) gave 7,811 distinct pairs.
A `Code` merges register and memory operands (`Add_rm64_imm8`) and XED splits them (`ADD_GPRv_IMMb`, `ADD_MEMv_IMMb`, `ADD_LOCK_MEMv_IMMb`).
An iform merges operand sizes (`GPRv`) and iced splits them (`Add_rm16_imm8`, `Add_rm32_imm8`, `Add_rm64_imm8`).
3,307 Codes map to two or more iforms; 568 iforms map to two or more Codes.
The iform is the finer division in most places, and the only one with APX and AVX10.2, which iced 1.22.0 does not decode.

## Left out

- 120 iforms valid only outside 64-bit mode, such as AAA, DAA, BOUND, INTO, PUSHA, LDS, ARPL, PUSH and POP of CS, DS, ES and SS, INC and DEC at 40h to 4Fh, and the 82h aliases of the arithmetic group.
- The encodings of listed forms that are valid only outside 64-bit mode.
- What XED does not model: Knights Corner (KNC), Cyrix and other forms iced decodes only behind a decoder option.

## Regenerate

On a Linux machine, in a new folder holding XED and mbuild of the same release side by side:

1. `cd xed && python3 mfile.py just-prep && python3 pysrc/xed_to_db.py --xed-dgen=obj/dgen --out=obj/xed_db.json`
2. For the source tests only, `python3 mfile.py examples` builds `obj/xed-iform-map-init.c`.
3. Here: `cargo run -p wcontainers-processor-architecture-x86-64 --example generate_forms -- <xed>/obj`, then `cargo test -p wcontainers-processor-architecture-x86-64 --test forms_list`, then `XED_BUILD=<xed>/obj cargo test -p wcontainers-processor-architecture-x86-64 --test forms_source -- --ignored`.
