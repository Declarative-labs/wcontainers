// Makes the access its first argument names, prints what it read and exits with status 0: exclusive
// (ldxr from an address that is 1 mod 8), stack (a load through sp when sp is not 16 byte aligned)
// or monitor (ldxr, a system call, then stxr, printing stxr's status: a return to the program clears
// the exclusive monitor). No C library.

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
    case 'm': {
        register long x8 __asm__("x8") = SYS_getpid;
        register long x0 __asm__("x0") = 0;
        __asm__ volatile("ldxr %[v], [%[a]]\n svc 0\n stxr %w[v], x8, [%[a]]"
                         : [v] "=&r"(value), "+r"(x0)
                         : [a] "r"(words), "r"(x8)
                         : "memory");
        break;
    }
    }
    print_number(value);
    exit_group(0);
}
