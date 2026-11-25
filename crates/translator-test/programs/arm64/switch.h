// ARM64's context switch, as coroutine libraries write it: the callee-saved registers, sp and the return
// address swapped, then ret. Also a call on a stack of the program's own.
#include <stdint.h>

// x19 to x30, sp, d8 to d15.
struct context {
    uint64_t saved[21];
};

// Keeps the running context in `from` and resumes `to`.
void swap_context(struct context *from, struct context *to);
__asm__(".pushsection .text\n"
        ".globl swap_context\n"
        "swap_context:\n"
        "  stp x19, x20, [x0, #0]\n"
        "  stp x21, x22, [x0, #16]\n"
        "  stp x23, x24, [x0, #32]\n"
        "  stp x25, x26, [x0, #48]\n"
        "  stp x27, x28, [x0, #64]\n"
        "  stp x29, x30, [x0, #80]\n"
        "  mov x9, sp\n"
        "  str x9, [x0, #96]\n"
        "  stp d8, d9, [x0, #104]\n"
        "  stp d10, d11, [x0, #120]\n"
        "  stp d12, d13, [x0, #136]\n"
        "  stp d14, d15, [x0, #152]\n"
        "  ldp x19, x20, [x1, #0]\n"
        "  ldp x21, x22, [x1, #16]\n"
        "  ldp x23, x24, [x1, #32]\n"
        "  ldp x25, x26, [x1, #48]\n"
        "  ldp x27, x28, [x1, #64]\n"
        "  ldp x29, x30, [x1, #80]\n"
        "  ldr x9, [x1, #96]\n"
        "  mov sp, x9\n"
        "  ldp d8, d9, [x1, #104]\n"
        "  ldp d10, d11, [x1, #120]\n"
        "  ldp d12, d13, [x1, #136]\n"
        "  ldp d14, d15, [x1, #152]\n"
        "  ret\n"
        ".popsection\n");

// Makes `context` start `body` on the stack ending at `top` when it is first resumed.
static inline void start_context(struct context *context, void (*body)(void), void *top) {
    context->saved[11] = (uint64_t)body;  // x30
    context->saved[12] = (uint64_t)top;   // sp
}

// Moves to the stack ending at `top` and calls `function`, which never returns.
#define CALL_ON_STACK(top, function) \
    __asm__ volatile("mov sp, %0\n\tbl " #function "\n\tbrk #0" ::"r"(top) : "memory", "x30")
