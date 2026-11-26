// The handler at EL1 of the single step machine's guest, assembled by build.rs into the bytes the machine copies
// into guest memory. The build script defines every name in capitals from the slot's layout and the guest's.
//
// Every exception but a synchronous one from EL0 is unexpected. The handler keeps the current slot in TPIDR_EL1,
// and leaves the guest by the doorbell's hypervisor call once a slot's `next` is zero.

    .arch_extension sve
    .text
    .balign VECTOR_TABLE_BYTES
// The vectors: from EL1 (with SP_EL0, then SP_EL1), then from EL0 in AArch64, then in AArch32; each synchronous,
// IRQ, FIQ and SError.
handler_vectors:
    .rept 8
    b 8f
    .balign VECTOR_BYTES
    .endr
    b 5f
    .balign VECTOR_BYTES
    .rept 7
    b 8f
    .balign VECTOR_BYTES
    .endr
// The entry: unlock the OS lock, which stops software step, forget the translations and instructions of the frames
// the last batch used, then run the slot. Within a batch each slot has its own ASID and frames.
handler_entry:
    msr oslar_el1, xzr
// Where a CPU left at the doorbell goes on to, its OS lock already open.
10:
    tlbi vmalle1is
    ic ialluis
    dsb ish
    isb
3:
    mrs x0, tpidr_el1
    ldr x1, [x0, #SLOT_TRANSLATION_TABLE]
    msr ttbr0_el1, x1
    isb
    add x1, x0, #SLOT_BEFORE
    ldp x2, x3, [x1, #REGISTERS_STACK_POINTER]
    msr sp_el0, x2
    msr elr_el1, x3
    ldp x2, x3, [x1, #REGISTERS_SAVED_PROGRAM_STATUS]
    msr spsr_el1, x2
    msr tpidr_el0, x3
    ldp x2, x3, [x1, #REGISTERS_FLOATING_POINT_CONTROL]
    msr fpcr, x2
    msr fpsr, x3
    add x2, x1, #REGISTERS_VECTOR
    ldp q0, q1, [x2, #0]
    ldp q2, q3, [x2, #32]
    ldp q4, q5, [x2, #64]
    ldp q6, q7, [x2, #96]
    ldp q8, q9, [x2, #128]
    ldp q10, q11, [x2, #160]
    ldp q12, q13, [x2, #192]
    ldp q14, q15, [x2, #224]
    ldp q16, q17, [x2, #256]
    ldp q18, q19, [x2, #288]
    ldp q20, q21, [x2, #320]
    ldp q22, q23, [x2, #352]
    ldp q24, q25, [x2, #384]
    ldp q26, q27, [x2, #416]
    ldp q28, q29, [x2, #448]
    ldp q30, q31, [x2, #480]
// With SVE, FFR through P0 and then every predicate, each in units of a predicate's bytes.
    ldr x3, [x0, #SLOT_SCALABLE_VECTORS]
    cbz x3, 11f
    add x2, x1, #REGISTERS_PREDICATE
    ldr p0, [x2, #FIRST_FAULT_PLACE, mul vl]
    wrffr p0.b
    ldr p0, [x2, #0, mul vl]
    ldr p1, [x2, #1, mul vl]
    ldr p2, [x2, #2, mul vl]
    ldr p3, [x2, #3, mul vl]
    ldr p4, [x2, #4, mul vl]
    ldr p5, [x2, #5, mul vl]
    ldr p6, [x2, #6, mul vl]
    ldr p7, [x2, #7, mul vl]
    ldr p8, [x2, #8, mul vl]
    ldr p9, [x2, #9, mul vl]
    ldr p10, [x2, #10, mul vl]
    ldr p11, [x2, #11, mul vl]
    ldr p12, [x2, #12, mul vl]
    ldr p13, [x2, #13, mul vl]
    ldr p14, [x2, #14, mul vl]
    ldr p15, [x2, #15, mul vl]
11:
    ldp x2, x3, [x1, #16]
    ldp x4, x5, [x1, #32]
    ldp x6, x7, [x1, #48]
    ldp x8, x9, [x1, #64]
    ldp x10, x11, [x1, #80]
    ldp x12, x13, [x1, #96]
    ldp x14, x15, [x1, #112]
    ldp x16, x17, [x1, #128]
    ldp x18, x19, [x1, #144]
    ldp x20, x21, [x1, #160]
    ldp x22, x23, [x1, #176]
    ldp x24, x25, [x1, #192]
    ldp x26, x27, [x1, #208]
    ldp x28, x29, [x1, #224]
    ldr x30, [x1, #240]
    ldp x0, x1, [x1, #0]
    eret
// A synchronous exception from EL0: save everything into the slot, then run the next one.
5:
    str x0, [sp, #-16]!
    mrs x0, tpidr_el1
    add x0, x0, #SLOT_AFTER
    stp x2, x3, [x0, #16]
    stp x4, x5, [x0, #32]
    stp x6, x7, [x0, #48]
    stp x8, x9, [x0, #64]
    stp x10, x11, [x0, #80]
    stp x12, x13, [x0, #96]
    stp x14, x15, [x0, #112]
    stp x16, x17, [x0, #128]
    stp x18, x19, [x0, #144]
    stp x20, x21, [x0, #160]
    stp x22, x23, [x0, #176]
    stp x24, x25, [x0, #192]
    stp x26, x27, [x0, #208]
    stp x28, x29, [x0, #224]
    str x30, [x0, #240]
    ldr x2, [sp], #16
    stp x2, x1, [x0, #0]
    mrs x2, sp_el0
    mrs x3, elr_el1
    stp x2, x3, [x0, #REGISTERS_STACK_POINTER]
    mrs x2, spsr_el1
    mrs x3, tpidr_el0
    stp x2, x3, [x0, #REGISTERS_SAVED_PROGRAM_STATUS]
    mrs x2, fpcr
    mrs x3, fpsr
    stp x2, x3, [x0, #REGISTERS_FLOATING_POINT_CONTROL]
    mrs x2, esr_el1
    mrs x3, far_el1
    stp x2, x3, [x0, #REGISTERS_SYNDROME]
    add x2, x0, #REGISTERS_VECTOR
    stp q0, q1, [x2, #0]
    stp q2, q3, [x2, #32]
    stp q4, q5, [x2, #64]
    stp q6, q7, [x2, #96]
    stp q8, q9, [x2, #128]
    stp q10, q11, [x2, #160]
    stp q12, q13, [x2, #192]
    stp q14, q15, [x2, #224]
    stp q16, q17, [x2, #256]
    stp q18, q19, [x2, #288]
    stp q20, q21, [x2, #320]
    stp q22, q23, [x2, #352]
    stp q24, q25, [x2, #384]
    stp q26, q27, [x2, #416]
    stp q28, q29, [x2, #448]
    stp q30, q31, [x2, #480]
// With SVE, every predicate, then FFR through P0.
    mrs x3, tpidr_el1
    ldr x3, [x3, #SLOT_SCALABLE_VECTORS]
    cbz x3, 12f
    add x2, x0, #REGISTERS_PREDICATE
    str p0, [x2, #0, mul vl]
    str p1, [x2, #1, mul vl]
    str p2, [x2, #2, mul vl]
    str p3, [x2, #3, mul vl]
    str p4, [x2, #4, mul vl]
    str p5, [x2, #5, mul vl]
    str p6, [x2, #6, mul vl]
    str p7, [x2, #7, mul vl]
    str p8, [x2, #8, mul vl]
    str p9, [x2, #9, mul vl]
    str p10, [x2, #10, mul vl]
    str p11, [x2, #11, mul vl]
    str p12, [x2, #12, mul vl]
    str p13, [x2, #13, mul vl]
    str p14, [x2, #14, mul vl]
    str p15, [x2, #15, mul vl]
    rdffr p0.b
    str p0, [x2, #FIRST_FAULT_PLACE, mul vl]
12:
    mrs x0, tpidr_el1
    mov x1, #ENDING_EXCEPTION_FROM_EL0
    str x1, [x0, #SLOT_ENDING]
    ldr x1, [x0, #SLOT_NEXT]
    cbz x1, 6f
    msr tpidr_el1, x1
    b 3b
// Any other exception: record what the syndrome registers say, and stop the batch.
8:
    mrs x0, tpidr_el1
    mov x1, #ENDING_UNEXPECTED
    str x1, [x0, #SLOT_ENDING]
    add x0, x0, #SLOT_AFTER
    mrs x2, esr_el1
    mrs x3, far_el1
    stp x2, x3, [x0, #REGISTERS_SYNDROME]
    mrs x2, spsr_el1
    str x2, [x0, #REGISTERS_SAVED_PROGRAM_STATUS]
    mrs x2, elr_el1
    str x2, [x0, #REGISTERS_PROGRAM_COUNTER]
6:
    mov x0, #DOORBELL_FUNCTION
    hvc #0
// The next run goes on to the entry, so the CPU waits here parked at it.
    b 10b
handler_end:
