// Writes a line, then dies the way its first argument names, with no C library and no handlers:
// read (an unmapped address), write (a read-only page), execute (a page without execute),
// breakpoint (brk) or undefined (udf).

#include <sys/syscall.h>

__asm__(".globl _start\n_start:\n mov x0, sp\n bl start\n");

static long call3(long number, long a, long b, long c) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}

static const char line[] = "about to fault\n";
static unsigned int data[4] = {0xd65f03c0}; // ret, in a page that cannot run

void start(long *stack) {
    char **argv = (char **)(stack + 1);
    call3(SYS_write, 1, (long)line, sizeof line - 1);
    switch (stack[0] > 1 ? argv[1][0] : 0) {
    case 'r':
        call3(SYS_exit_group, *(volatile long *)0x10, 0, 0);
        break;
    case 'w':
        *(volatile char *)line = 0;
        break;
    case 'e':
        ((void (*)(void))data)();
        break;
    case 'b':
        __asm__ volatile("brk #5");
        break;
    case 'u':
        __asm__ volatile("udf #0");
        break;
    }
    call3(SYS_exit_group, 0, 0, 0);
}
