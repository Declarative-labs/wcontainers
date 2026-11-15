// Writes to standard output and standard error and exits with status 3, with no C library.

#include <sys/syscall.h>

static long call3(long number, long a, long b, long c) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}

void _start(void) {
    static const char out[] = "hello, world\n";
    static const char err[] = "hello, error\n";
    call3(SYS_write, 1, (long)out, sizeof out - 1);
    call3(SYS_write, 2, (long)err, sizeof err - 1);
    call3(SYS_exit_group, 3, 0, 0);
    for (;;) {
    }
}
