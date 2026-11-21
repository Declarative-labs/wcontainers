// System calls and number printing for test programs with no C library.

#include <sys/syscall.h>

// `_start` hands the initial stack pointer (argc, argv, envp, auxv) to `start(long *stack)`.
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

// Writes `value` in decimal and a newline to standard output.
static void print_number(long value) {
    char text[24];
    char *end = text + sizeof text;
    char *at = end;
    *--at = '\n';
    unsigned long magnitude = value < 0 ? -(unsigned long)value : (unsigned long)value;
    do {
        *--at = '0' + magnitude % 10;
        magnitude /= 10;
    } while (magnitude);
    if (value < 0) {
        *--at = '-';
    }
    call(SYS_write, 1, (long)at, end - at);
}

// Writes a string literal with no newline to standard output.
#define print(literal) call(SYS_write, 1, (long)(literal), sizeof(literal) - 1)

static __attribute__((noreturn)) void exit_group(long status) {
    call(SYS_exit_group, status, 0, 0);
    __builtin_unreachable();
}
