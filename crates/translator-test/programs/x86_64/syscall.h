// x86-64 Linux system calls for programs without a C library: the numbers they use and a call of up to three
// arguments.
#define SYS_write 1
#define SYS_writev 20
#define SYS_exit 60
#define SYS_exit_group 231

static long syscall3(long number, long a, long b, long c) {
    long result;
    __asm__ volatile("syscall"
                     : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory");
    return result;
}
