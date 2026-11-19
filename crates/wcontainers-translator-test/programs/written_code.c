// Code the program writes while it runs, into memory that held no code in the file, then calls, as a JIT does.
#include <stdio.h>

static unsigned int area_add[16] __attribute__((aligned(64)));
static unsigned int area_mul[16] __attribute__((aligned(64)));

typedef long (*fn)(long, long);

// Arm's full sequence for code written by the program, one 64 byte line (the areas are one line each). The
// C library's __clear_cache leaves out the barrier after `ic ivau`, and then the CPU may still run the old code.
static void written(void *line) {
    __asm__ volatile("dc cvau, %0\n dsb ish\n ic ivau, %0\n dsb ish\n isb" : : "r"(line) : "memory");
}

int main(void) {
    area_add[0] = 0x8b010000; // add x0, x0, x1
    area_add[1] = 0xd65f03c0; // ret
    area_mul[0] = 0x9b017c00; // mul x0, x0, x1
    area_mul[1] = 0xd65f03c0; // ret
    written(area_add);
    written(area_mul);
    fn add = (fn)(void *)area_add;
    fn mul = (fn)(void *)area_mul;
    long total = 0;
    for (long i = 0; i < 1000; i++) total = add(total, mul(i, 3));
    // The same place written again with other code runs the new code.
    area_add[0] = 0xcb010000; // sub x0, x0, x1
    written(area_add);
    total = add(total, 500);
    printf("%ld\n", total);
    return 0;
}
