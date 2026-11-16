// Two coroutines on stacks of their own, switched by a hand-written context switch as coroutine libraries
// do (callee-saved registers, sp and the return address swapped, then ret). Each yields SWITCHES values
// from inside a recursion, so its frames live on across switches.
#include <stdint.h>
#include <stdio.h>

#ifndef SWITCHES
#define SWITCHES 3000
#endif

// x19 to x30, sp, d8 to d15.
struct context {
    uint64_t saved[21];
};

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

struct coroutine {
    struct context context;
    unsigned long step;
    unsigned long value;
    long left;
    int finished;
    unsigned char stack[1 << 16] __attribute__((aligned(16)));
};

static struct context main_context;
static struct coroutine coroutines[2];
static struct coroutine *running;

static void yield(unsigned long value) {
    running->value = value;
    swap_context(&running->context, &main_context);
}

// Yields from `depth` frames down, then goes back up yielding again.
__attribute__((noinline)) static unsigned long descend(struct coroutine *self, unsigned long depth,
                                                       unsigned long seed) {
    if (self->left <= 0) return seed;
    if (depth == 0) {
        self->left--;
        yield(seed * self->step + 1);
        return seed + 1;
    }
    unsigned long below = descend(self, depth - 1, seed * 7 + depth);
    if (self->left > 0) {
        self->left--;
        yield(below ^ depth);
    }
    return below + depth;
}

__attribute__((noreturn, used)) static void body(void) {
    struct coroutine *self = running;
    unsigned long seed = self->step;
    while (self->left > 0) seed = descend(self, seed % 9, seed);
    self->finished = 1;
    for (;;) swap_context(&self->context, &main_context);
}

static void make(struct coroutine *c, unsigned long step) {
    c->step = step;
    c->left = SWITCHES;
    c->context.saved[11] = (uint64_t)body;  // x30
    c->context.saved[12] = (uint64_t)(c->stack + sizeof c->stack);  // sp
}

int main(void) {
    make(&coroutines[0], 3);
    make(&coroutines[1], 5);
    unsigned long total = 0;
    long switches = 0;
    for (int done = 0; done != 3;) {
        for (int k = 0; k < 2; k++) {
            if (done & (1 << k)) continue;
            running = &coroutines[k];
            swap_context(&main_context, &running->context);
            switches++;
            if (running->finished)
                done |= 1 << k;
            else
                total = total * 31 + running->value;
        }
    }
    printf("%lu %ld\n", total, switches);
    return 0;
}
