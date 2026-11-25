// Two coroutines on stacks of their own, switched by a hand-written context switch as coroutine libraries
// do (`swap_context`, in the CPU's switch.h). Each yields SWITCHES values from inside a recursion, so its
// frames live on across switches.
#include <stdint.h>
#include <stdio.h>

#ifndef SWITCHES
#define SWITCHES 3000
#endif

#include "switch.h"

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
    start_context(&c->context, body, c->stack + sizeof c->stack);
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
