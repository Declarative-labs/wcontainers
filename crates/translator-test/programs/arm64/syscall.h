// ARM64 Linux system calls for programs without a C library: the numbers they use and a call of up to three
// arguments.
#define SYS_write 64
#define SYS_writev 66
#define SYS_exit 93
#define SYS_exit_group 94

static long syscall3(long number, long a, long b, long c) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}
