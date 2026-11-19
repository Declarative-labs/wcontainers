// A system call answered at once, CALLS times in a loop (GETPPID and EXIT_GROUP are its and exit_group's
// numbers), or with NOTHING defined a nop in each call's place: the difference of the two runs' times is the
// calls' cost. The first process's parent is 0, so the exit status is 1 only when every call was answered.

typedef long i64;

static i64 call(i64 number, i64 first) {
    register i64 x8 __asm__("x8") = number;
    register i64 x0 __asm__("x0") = first;
#ifdef NOTHING
    __asm__ volatile("nop" : "+r"(x0) : "r"(x8) : "memory");
#else
    __asm__ volatile("svc #0" : "+r"(x0) : "r"(x8) : "memory");
#endif
    return x0;
}

__attribute__((noreturn)) static void exit_group(i64 status) {
    register i64 x8 __asm__("x8") = EXIT_GROUP;
    register i64 x0 __asm__("x0") = status;
    __asm__ volatile("svc #0" : : "r"(x0), "r"(x8) : "memory");
    __builtin_unreachable();
}

__attribute__((section(".text._start"), noreturn)) void _start(void) {
    i64 parents = 0;
    for (i64 i = 0; i < CALLS; i++) parents += call(GETPPID, 1);
    exit_group(parents == 0);
}
