// Code another thread changes while this one runs it, with no announcement: this thread spins on a page of
// code of its own until the immediate it compares is written, then prints what it read. The other thread is
// started by clone and writes once this one has entered the loop; where clone is refused (the translator's
// test runners give one thread), the runner writes the same bytes as that thread would, at a stop on the page.
// From the build: PAGE_SIZE, IMMEDIATE (where in the spinning code its immediate is) and NEW (what the other
// thread writes there).
#define _GNU_SOURCE
#include <sched.h>
#include <stdio.h>
#include <sys/syscall.h>

#define OLD 1
#define THREAD (CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM)

static unsigned char page[PAGE_SIZE] __attribute__((aligned(PAGE_SIZE)));
static unsigned char stack[PAGE_SIZE] __attribute__((aligned(16)));
static volatile int entered;

static const unsigned char spin_code[] = {
    0xb8, OLD, 0, 0, 0, // mov eax, imm32: the immediate the other thread writes
    0x3d, OLD, 0, 0, 0, // cmp eax, OLD
    0x74, 0xf4,         // je to the start
    0xc3,               // ret
};

__attribute__((used, noinline)) void writer(void) {
    while (!entered) {
    }
    unsigned value = NEW;
    __builtin_memcpy(page + IMMEDIATE, &value, sizeof value);
}

// Start `writer` on its own stack in a thread of this process, which then exits. Gives clone's result.
static long start_writer(void) {
    long result;
    register long parent_tid __asm__("r10") = 0;
    register long tls __asm__("r8") = 0;
    __asm__ volatile("syscall\n"
                     "test %%rax, %%rax\n"
                     "jnz 1f\n"
                     "call writer\n"
                     "mov %[exit], %%eax\n"
                     "xor %%edi, %%edi\n"
                     "syscall\n"
                     "1:\n"
                     : "=a"(result)
                     : "a"(SYS_clone), "D"(THREAD), "S"(stack + sizeof stack), "d"(0), "r"(parent_tid),
                       "r"(tls), [exit] "i"(SYS_exit)
                     : "rcx", "r11", "memory");
    return result;
}

int main(void) {
    for (unsigned i = 0; i < sizeof spin_code; i++) page[i] = spin_code[i];
    start_writer();
    entered = 1;
    unsigned read;
    __asm__ volatile("call *%1" : "=a"(read) : "r"(page) : "memory", "cc");
    printf("spun until the immediate was %u\n", read);
    return 0;
}
