// Makes the system call numbered by its first argument, with all six arguments zero, prints what it
// returned in decimal and exits with status 0. No C library.

#include <sys/syscall.h>

__asm__(".globl _start\n_start:\n mov x0, sp\n bl start\n");

static long call(long number, long a, long b, long c) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    register long x3 __asm__("x3") = 0;
    register long x4 __asm__("x4") = 0;
    register long x5 __asm__("x5") = 0;
    __asm__ volatile("svc 0"
                     : "+r"(x0)
                     : "r"(x8), "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x5)
                     : "memory");
    return x0;
}

void start(long *stack) {
    char **argv = (char **)(stack + 1);
    long number = 0;
    for (const char *digit = stack[0] > 1 ? argv[1] : ""; *digit; digit++) {
        number = number * 10 + (*digit - '0');
    }
    long result = call(number, 0, 0, 0);

    char text[24];
    char *end = text + sizeof text;
    char *at = end;
    *--at = '\n';
    unsigned long magnitude = result < 0 ? -(unsigned long)result : (unsigned long)result;
    do {
        *--at = '0' + magnitude % 10;
        magnitude /= 10;
    } while (magnitude);
    if (result < 0) {
        *--at = '-';
    }
    call(SYS_write, 1, (long)at, end - at);
    call(SYS_exit_group, 0, 0, 0);
}
