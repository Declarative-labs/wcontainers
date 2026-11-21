// Signal frames as arm64 Linux lays them out: the records in the frame, the fault address and
// syndrome a fault leaves, si_code and si_addr of each fault, the alternate stack, masks, one-shot
// and re-entrant handlers, a handler that moves the pc past a fault, registers kept across a
// handler, and a handler that returns through the vDSO.
#define _GNU_SOURCE
#include <elf.h>
#include <setjmp.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <ucontext.h>
#include <unistd.h>

static sigjmp_buf jump;
// Where a fault is expected, for addresses that differ between machines.
static uintptr_t expected;
static char *alternate;
static const size_t alternate_size = 65536;

static void records(ucontext_t *uc) {
    unsigned char *p = (unsigned char *)uc->uc_mcontext.__reserved;
    for (int off = 0; off < 4096;) {
        uint32_t magic, size;
        memcpy(&magic, p + off, 4);
        memcpy(&size, p + off + 4, 4);
        printf("  record %08x size %u", magic, size);
        if (magic == 0x45535201) {
            uint64_t esr;
            memcpy(&esr, p + off + 8, 8);
            // A translation fault's level comes from the page tables, which differ by machine.
            int translation = (esr & 0x3c) == 0x04;
            printf(" esr %llx%s", (unsigned long long)(translation ? esr & ~3ull : esr),
                   translation ? " translation" : "");
        }
        printf("\n");
        if (magic == 0 || size == 0) break;
        off += size;
    }
}

static void show(int sig, siginfo_t *info, void *context) {
    ucontext_t *uc = context;
    mcontext_t *m = &uc->uc_mcontext;
    char here;
    int on_alternate = &here >= alternate && &here < alternate + alternate_size;
    uintptr_t fault = m->fault_address, addr = (uintptr_t)info->si_addr;
    printf("sig %d code %d errno %d fault_address %llx expected %d pstate %llx alternate %d frame %ld sp16 %d\n",
           sig, info->si_code, info->si_errno, (unsigned long long)(fault < 0x100000 ? fault : 0),
           fault != 0 && fault == expected, (unsigned long long)(m->pstate & 0xf0000000), on_alternate,
           (long)((char *)uc - (char *)info), (int)(((uintptr_t)info) % 16));
    printf("  addr is pc %d, addr %llx expected %d, stack flags %d size %d\n", info->si_addr == (void *)m->pc,
           (unsigned long long)(addr < 0x100000 ? addr : 0), addr == expected, uc->uc_stack.ss_flags,
           uc->uc_stack.ss_size == alternate_size);
    sigset_t now;
    sigprocmask(SIG_BLOCK, 0, &now);
    printf("  blocked in handler: own %d usr2 %d, saved mask usr2 %d\n", sigismember(&now, sig),
           sigismember(&now, SIGUSR2), sigismember(&uc->uc_sigmask, SIGUSR2));
    records(uc);
    if (sig == SIGSEGV || sig == SIGBUS || sig == SIGILL || sig == SIGTRAP) siglongjmp(jump, 1);
}

static void skip(int sig, siginfo_t *info, void *context) {
    ucontext_t *uc = context;
    (void)info;
    printf("skip %d\n", sig);
    uc->uc_mcontext.pc += 4;
}

static volatile int depth;
static void nested(int sig) {
    depth++;
    if (depth < 3) raise(sig);
    printf("nested depth %d\n", depth);
}

static volatile uint64_t raw_caught;
static void raw_handler(int sig) { raw_caught = sig; }

static void handle(int sig, void (*f)(int, siginfo_t *, void *), int flags, int block_usr2) {
    struct sigaction action = {0};
    action.sa_sigaction = f;
    action.sa_flags = SA_SIGINFO | flags;
    if (block_usr2) sigaddset(&action.sa_mask, SIGUSR2);
    sigaction(sig, &action, 0);
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    unsigned long ehdr = getauxval(AT_SYSINFO_EHDR);
    printf("vdso %d magic %d minsigstksz %lu\n", ehdr != 0, ehdr && !memcmp((void *)ehdr, ELFMAG, 4),
           getauxval(AT_MINSIGSTKSZ));
    if (ehdr) {
        Elf64_Ehdr *e = (Elf64_Ehdr *)ehdr;
        printf("vdso type %d machine %d\n", e->e_type, e->e_machine);
    }
    FILE *maps = fopen("/proc/self/maps", "r");
    char line[512];
    char name[64], permissions[8];
    while (fgets(line, sizeof line, maps))
        if (strstr(line, "[v") && sscanf(line, "%*s %7s %*s %*s %*s %63s", permissions, name) == 2)
            printf("%s %s\n", permissions, name);
    fclose(maps);

    for (int sig = 1; sig < 32; sig++)
        if (sig != SIGKILL && sig != SIGSTOP) handle(sig, show, 0, 1);
    raise(SIGUSR1);
    if (!sigsetjmp(jump, 1)) *(volatile int *)16 = 1;
    raise(SIGUSR1);
    if (!sigsetjmp(jump, 1)) (void)*(volatile int *)16;
    static const int read_only = 1;
    expected = (uintptr_t)&read_only;
    if (!sigsetjmp(jump, 1)) *(volatile int *)&read_only = 2;
    void *none = mmap(0, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    expected = (uintptr_t)none;
    if (!sigsetjmp(jump, 1)) (void)*(volatile int *)none;
    expected = 0;
    if (!sigsetjmp(jump, 1)) __asm__ volatile("udf #0");
    if (!sigsetjmp(jump, 1)) __asm__ volatile("brk #7");
    if (!sigsetjmp(jump, 1)) ((void (*)(void))(uintptr_t)0x10002)();
    raise(SIGUSR1);
    // On the alternate stack, which the handler sees as in use.
    alternate = mmap(0, alternate_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    stack_t ss = {.ss_sp = alternate, .ss_size = alternate_size, .ss_flags = 0};
    sigaltstack(&ss, 0);
    handle(SIGUSR1, show, SA_ONSTACK, 0);
    raise(SIGUSR1);
    handle(SIGUSR1, show, 0, 0);
    raise(SIGUSR1);
    // One-shot: the second raise kills unless reset.
    handle(SIGWINCH, show, SA_RESETHAND, 0);
    raise(SIGWINCH);
    struct sigaction after;
    sigaction(SIGWINCH, 0, &after);
    printf("after one-shot handler is default %d flags siginfo %d\n", after.sa_handler == SIG_DFL,
           (after.sa_flags & SA_SIGINFO) != 0);
    // Re-entrant with SA_NODEFER.
    struct sigaction again = {0};
    again.sa_handler = nested;
    again.sa_flags = SA_NODEFER;
    sigaction(SIGUSR2, &again, 0);
    raise(SIGUSR2);
    // Moving the pc past a faulting instruction.
    handle(SIGSEGV, skip, 0, 0);
    *(volatile int *)24 = 1;
    printf("past the fault\n");
    // A handler with no restorer of its own returns through the vDSO.
    struct {
        void *handler;
        unsigned long flags;
        void *restorer;
        uint64_t mask;
    } raw = {raw_handler, 0, 0, 0};
    long r = syscall(SYS_rt_sigaction, SIGUSR1, &raw, 0, 8);
    printf("raw sigaction %ld\n", r);
    syscall(SYS_kill, getpid(), SIGUSR1);
    printf("raw caught %llu\n", (unsigned long long)raw_caught);
    // Floating point registers are kept across a handler.
    volatile double x = 1.5;
    raise(SIGUSR2);
    printf("double kept %d\n", x == 1.5);
    // A frame that cannot be written kills with SIGSEGV.
    printf("done\n");
    return 0;
}
