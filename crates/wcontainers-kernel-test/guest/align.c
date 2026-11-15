// Makes the misaligned access its first argument names, prints what it read and exits with status 0:
// exclusive (ldxr from an address that is 1 mod 8) or stack (a load through sp when sp is not 16 byte
// aligned). No C library.

#include "bare.h"

static volatile unsigned long words[4];

void start(long *stack) {
    char **argv = (char **)(stack + 1);
    unsigned long value = 0;
    switch (stack[0] > 1 ? argv[1][0] : 0) {
    case 'e':
        __asm__ volatile("ldxr %0, [%1]\n clrex" : "=r"(value) : "r"((char *)words + 1) : "memory");
        break;
    case 's':
        __asm__ volatile("sub sp, sp, #4\n ldr %0, [sp]\n add sp, sp, #4" : "=r"(value) : : "memory");
        value = 0;
        break;
    }
    print_number(value);
    exit_group(0);
}
