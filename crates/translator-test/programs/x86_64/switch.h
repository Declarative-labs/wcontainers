// x86-64's context switch, as coroutine libraries write it: the callee-saved registers pushed, rsp swapped,
// the other context's registers popped, then ret. Also a call on a stack of the program's own.
#include <stdint.h>

// The stack pointer, with rbp, rbx and r12 to r15 and the return address pushed below it.
struct context {
    uint64_t sp;
};

// Keeps the running context in `from` and resumes `to`.
void swap_context(struct context *from, struct context *to);
__asm__(".pushsection .text\n"
        ".globl swap_context\n"
        "swap_context:\n"
        "  push %rbp\n"
        "  push %rbx\n"
        "  push %r12\n"
        "  push %r13\n"
        "  push %r14\n"
        "  push %r15\n"
        "  mov %rsp, (%rdi)\n"
        "  mov (%rsi), %rsp\n"
        "  pop %r15\n"
        "  pop %r14\n"
        "  pop %r13\n"
        "  pop %r12\n"
        "  pop %rbx\n"
        "  pop %rbp\n"
        "  ret\n"
        ".popsection\n");

// The registers swap_context pops before its return address.
#define SAVED_REGISTERS 6

// Makes `context` start `body` on the stack ending at `top` when it is first resumed: the registers it
// pops, then `body` as the return address, placed so that `body` starts with rsp as after a call.
static inline void start_context(struct context *context, void (*body)(void), void *top) {
    uint64_t *sp = (uint64_t *)((uintptr_t)top & ~(uintptr_t)15) - (SAVED_REGISTERS + 2);
    for (int i = 0; i < SAVED_REGISTERS; i++) sp[i] = 0;
    sp[SAVED_REGISTERS] = (uint64_t)body;
    context->sp = (uint64_t)sp;
}

// Moves to the stack ending at `top` and calls `function`, which never returns.
#define CALL_ON_STACK(top, function) \
    __asm__ volatile("mov %0, %%rsp\n\tcall " #function "\n\tud2" ::"r"(top) : "memory")
