// A fiber on a stack of its own, switched to and from with setjmp and longjmp, SWITCHES round trips.
#include <setjmp.h>
#include <stdio.h>

#ifndef SWITCHES
#define SWITCHES 3000
#endif

static jmp_buf main_context, fiber_context;
static volatile unsigned long total;
static unsigned char fiber_stack[1 << 16] __attribute__((aligned(16)));

__attribute__((noinline, used)) void fiber_body(void) {
    for (long i = 0; i < SWITCHES; i++) {
        total = total * 3 + i;
        if (!setjmp(fiber_context)) longjmp(main_context, 1);
    }
    longjmp(main_context, 2);
}

// Moves to the fiber's stack and runs it; it never returns here.
__attribute__((noinline)) static void start(void) {
    __asm__ volatile("mov sp, %0\n\tbl fiber_body\n\tbrk #0" ::"r"(fiber_stack + sizeof fiber_stack)
                     : "memory", "x30");
}

int main(void) {
    long switches = 0;
    int r = setjmp(main_context);
    if (r == 0) start();
    while (r == 1) {
        switches++;
        r = setjmp(main_context);
        if (r == 0) longjmp(fiber_context, 1);
    }
    printf("%lu %ld %d\n", total, switches, r);
    return 0;
}
