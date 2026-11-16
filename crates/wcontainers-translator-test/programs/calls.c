// Direct calls between two functions, for timing a call that crosses from one code module to another.
// CALLS is how many calls main makes.

typedef unsigned long u64;
typedef long i64;

#ifndef CALLS
#define CALLS 1000
#endif

static i64 syscall3(i64 number, i64 a, i64 b, i64 c) {
    register i64 x8 __asm__("x8") = number;
    register i64 x0 __asm__("x0") = a;
    register i64 x1 __asm__("x1") = b;
    register i64 x2 __asm__("x2") = c;
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}

__attribute__((noinline)) u64 step(u64 x, u64 i) { return x * 31 + i; }

int main(void) {
    u64 x = 1;
    for (u64 i = 0; i < CALLS; i++) x = step(x, i);
    char digits[17];
    for (int k = 0; k < 16; k++) digits[k] = "0123456789abcdef"[(x >> (60 - 4 * k)) & 15];
    digits[16] = '\n';
    syscall3(64, 1, (i64)digits, 17);
    return 0;
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    int status = main();
    syscall3(94, status, 0, 0);
    __builtin_unreachable();
}
