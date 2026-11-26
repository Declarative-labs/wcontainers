// x86-64 Linux system calls for programs without a C library: the numbers they use and calls of up to three
// and six arguments.
#define SYS_write 1
#define SYS_writev 20
#define SYS_exit 60
#define SYS_exit_group 231
#define SYS_mmap 9
#define SYS_munmap 11
#define SYS_brk 12

static long syscall3(long number, long a, long b, long c) {
    long result;
    __asm__ volatile("syscall"
                     : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory");
    return result;
}

static long syscall6(long number, long a, long b, long c, long d, long e, long f) {
    long result;
    register long r10 __asm__("r10") = d;
    register long r8 __asm__("r8") = e;
    register long r9 __asm__("r9") = f;
    __asm__ volatile("syscall"
                     : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9)
                     : "rcx", "r11", "memory");
    return result;
}
