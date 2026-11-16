// Restartable sequences aborted by a handler: a signal delivered on the return from a system call made
// inside a critical section, and a fault inside one, move the thread to the abort address before the
// handler runs (the handler's context shows it, and the handler returns there); a signal outside the
// section leaves the thread where it was. Each time the area's rseq_cs is cleared.

#define _GNU_SOURCE
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <ucontext.h>
#include <unistd.h>

#define RSEQ_SIG 0xd428bc00

struct rseq_cs {
    uint32_t version;
    uint32_t flags;
    uint64_t start_ip;
    uint64_t post_commit_offset;
    uint64_t abort_ip;
} __attribute__((aligned(32)));

struct rseq_area {
    uint32_t cpu_id_start;
    uint32_t cpu_id;
    uint64_t rseq_cs;
    uint32_t flags;
    uint32_t node_id;
    uint32_t mm_cid;
    char end[];
} __attribute__((aligned(32)));

static struct rseq_area area;

// in_section(&area.rseq_cs, &descriptor, a, b): arms the section, then inside it makes the call kill(a,
// b) (signal_start) or loads from address a (fault_start). Returns 1 when it ran to its end, 2 from the
// abort address.
long signal_section(uint64_t *armed, struct rseq_cs *cs, long pid, long signal);
long fault_section(uint64_t *armed, struct rseq_cs *cs, long address, long unused);
extern char signal_start[], signal_end[], signal_abort[];
extern char fault_start[], fault_end[], fault_abort[];

__asm__(".text\n"
        ".global signal_section\n"
        "signal_section:\n"
        "    str x1, [x0]\n"
        ".global signal_start\n"
        "signal_start:\n"
        "    mov x0, x2\n"
        "    mov x1, x3\n"
        "    mov x8, #129\n"
        "    svc #0\n"
        "    mov x0, #1\n"
        ".global signal_end\n"
        "signal_end:\n"
        "    ret\n"
        "    .inst 0xd428bc00\n"
        ".global signal_abort\n"
        "signal_abort:\n"
        "    mov x0, #2\n"
        "    ret\n"
        ".global fault_section\n"
        "fault_section:\n"
        "    str x1, [x0]\n"
        ".global fault_start\n"
        "fault_start:\n"
        "    ldr x4, [x2]\n"
        "    mov x0, #1\n"
        ".global fault_end\n"
        "fault_end:\n"
        "    ret\n"
        "    .inst 0xd428bc00\n"
        ".global fault_abort\n"
        "fault_abort:\n"
        "    mov x0, #2\n"
        "    ret\n");

static volatile uint64_t handler_pc;
static volatile int handled;

static void on_signal(int signal, siginfo_t *info, void *context) {
    (void)signal;
    (void)info;
    handler_pc = ((ucontext_t *)context)->uc_mcontext.pc;
    handled++;
}

static const char *where(uint64_t pc) {
    if (pc == (uint64_t)signal_abort || pc == (uint64_t)fault_abort)
        return "the abort address";
    if (pc == (uint64_t)signal_start + 16)
        return "after the call";
    if (pc == (uint64_t)fault_start)
        return "the faulting load";
    return "elsewhere";
}

static void section(const char *label, struct rseq_cs *cs, long (*run)(uint64_t *, struct rseq_cs *, long, long),
                    long a, long b) {
    handled = 0;
    handler_pc = 0;
    long result = run(&area.rseq_cs, cs, a, b);
    printf("%s: returned %ld, handled %d, handler saw %s, rseq_cs %s\n", label, result, handled,
           handled ? where(handler_pc) : "-", area.rseq_cs ? "set" : "cleared");
    area.rseq_cs = 0;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    long registered = syscall(SYS_rseq, &area, 32, 0, RSEQ_SIG);
    printf("rseq: %ld\n", registered);
    struct sigaction action = {0};
    action.sa_sigaction = on_signal;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGUSR1, &action, NULL);
    sigaction(SIGSEGV, &action, NULL);

    struct rseq_cs signal_cs = {0, 0, (uint64_t)signal_start, (uint64_t)(signal_end - signal_start),
                                (uint64_t)signal_abort};
    struct rseq_cs fault_cs = {0, 0, (uint64_t)fault_start, (uint64_t)(fault_end - fault_start),
                               (uint64_t)fault_abort};
    section("signal in the section", &signal_cs, signal_section, getpid(), SIGUSR1);
    section("fault in the section", &fault_cs, fault_section, 0, 0);
    // A section that ends before the call: the signal comes outside it.
    struct rseq_cs short_cs = {0, 0, (uint64_t)signal_start, 4, (uint64_t)signal_abort};
    section("signal after the section", &short_cs, signal_section, getpid(), SIGUSR1);
    // A section whose abort address lacks the signature: the process is killed with SIGSEGV.
    struct rseq_cs bad_cs = {0, 0, (uint64_t)signal_start, (uint64_t)(signal_end - signal_start),
                             (uint64_t)signal_start};
    signal(SIGSEGV, SIG_DFL);
    section("abort address without the signature", &bad_cs, signal_section, getpid(), SIGUSR1);
    return 0;
}
