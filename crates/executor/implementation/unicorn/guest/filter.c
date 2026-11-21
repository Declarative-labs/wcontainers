// A program's own seccomp filters: the seccomp call's operations and checks, filters that are refused,
// what installed filters do to calls (alone, stacked and with Docker's profile), and, by the first
// argument, how a call a filter stops ends the program: trap, trapignored, killprocess, killthread,
// unknownaction, divzero, exec (the filter outlives execve) or limit (the instruction limit).

#include <errno.h>
#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <signal.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <unistd.h>

#define NR offsetof(struct seccomp_data, nr)
#define ARCH offsetof(struct seccomp_data, arch)
#define IP offsetof(struct seccomp_data, instruction_pointer)
#define ARG(n) (offsetof(struct seccomp_data, args) + 8 * (n))

#define STMT(code, k) ((struct sock_filter)BPF_STMT(code, k))
#define JUMP(code, k, jt, jf) ((struct sock_filter)BPF_JUMP(code, k, jt, jf))

static long install(unsigned flags, struct sock_filter *program, unsigned short length) {
    struct sock_fprog fprog = {length, program};
    errno = 0;
    long r = syscall(SYS_seccomp, SECCOMP_SET_MODE_FILTER, flags, &fprog);
    return r < 0 ? -errno : r;
}

static long call4(long number, long a, long b, long c, long d) {
    errno = 0;
    long r = syscall(number, a, b, c, d);
    return r < 0 ? -errno : r;
}

static long call(long number, long a, long b, long c) {
    errno = 0;
    long r = syscall(number, a, b, c);
    return r < 0 ? -errno : r;
}

// A filter that gives `action` to `number` and lets everything else through.
static long only(long number, unsigned action) {
    struct sock_filter program[] = {
        STMT(BPF_LD | BPF_W | BPF_ABS, NR),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, number, 0, 1),
        STMT(BPF_RET | BPF_K, action),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    return install(0, program, 4);
}

static void status(void) {
    FILE *file = fopen("/proc/self/status", "r");
    char line[256];
    while (fgets(line, sizeof line, file)) {
        if (!strncmp(line, "NoNewPrivs", 10) || !strncmp(line, "Seccomp", 7)) {
            fputs(line, stdout);
        }
    }
    fclose(file);
    printf("PR_GET_SECCOMP %d\n", prctl(PR_GET_SECCOMP, 0, 0, 0, 0));
}

static void operations(void) {
    static const unsigned actions[] = {
        SECCOMP_RET_KILL_PROCESS, SECCOMP_RET_KILL_THREAD, SECCOMP_RET_TRAP,  SECCOMP_RET_ERRNO,
        SECCOMP_RET_USER_NOTIF,   SECCOMP_RET_TRACE,       SECCOMP_RET_LOG,   SECCOMP_RET_ALLOW,
        SECCOMP_RET_ERRNO | 5,    0x00010000,              0x7fff0001,        0xffffffff,
    };
    for (unsigned i = 0; i < sizeof actions / sizeof actions[0]; i++) {
        printf("avail %x %ld\n", actions[i], call(SYS_seccomp, SECCOMP_GET_ACTION_AVAIL, 0, (long)&actions[i]));
    }
    printf("avail flags %ld\n", call(SYS_seccomp, SECCOMP_GET_ACTION_AVAIL, 1, (long)&actions[0]));
    printf("avail fault %ld\n", call(SYS_seccomp, SECCOMP_GET_ACTION_AVAIL, 0, 8));
    struct seccomp_notif_sizes sizes = {0};
    printf("sizes %ld %u %u %u\n", call(SYS_seccomp, SECCOMP_GET_NOTIF_SIZES, 0, (long)&sizes),
           sizes.seccomp_notif, sizes.seccomp_notif_resp, sizes.seccomp_data);
    printf("sizes flags %ld\n", call(SYS_seccomp, SECCOMP_GET_NOTIF_SIZES, 2, (long)&sizes));
    printf("sizes fault %ld\n", call(SYS_seccomp, SECCOMP_GET_NOTIF_SIZES, 0, 8));
    printf("op 4 %ld\n", call(SYS_seccomp, 4, 0, 0));
    printf("op -1 %ld\n", call(SYS_seccomp, -1, 0, 0));
    printf("op high %ld\n", call(SYS_seccomp, (1L << 32) | SECCOMP_GET_ACTION_AVAIL, 0, (long)&actions[0]));
    printf("strict %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_STRICT, 0, 0));
    printf("strict flags %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_STRICT, 1, 0));
    printf("strict args %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_STRICT, 0, 8));
    printf("prctl strict %d %d\n", prctl(PR_SET_SECCOMP, SECCOMP_MODE_STRICT, 0, 0, 0), errno);
    printf("prctl mode 0 %d %d\n", prctl(PR_SET_SECCOMP, 0, 0, 0, 0), errno);
    printf("prctl mode 3 %d %d\n", prctl(PR_SET_SECCOMP, 3, 0, 0, 0), errno);
    printf("prctl filter null %d %d\n", prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, 0, 0, 0), errno);
}

static void before_no_new_privs(void) {
    struct sock_filter allow[] = {STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)};
    struct sock_filter bad[] = {STMT(0xffff, 0)};
    printf("no nnp %ld\n", install(0, allow, 1));
    printf("no nnp bad %ld\n", install(0, bad, 1));
    printf("no nnp len 0 %ld\n", install(0, allow, 0));
    printf("no nnp len 4097 %ld\n", install(0, allow, 4097));
    printf("no nnp fprog fault %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, 8));
    printf("no nnp unknown flag %ld\n", install(64, allow, 1));
    struct sock_fprog null = {1, 0};
    printf("no nnp null %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, (long)&null));
}

static void refused(void) {
    struct {
        const char *name;
        struct sock_filter program[6];
        unsigned short length;
    } cases[] = {
        {"unknown code", {STMT(0xffff, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
        {"ret x", {STMT(BPF_RET | BPF_X, 0)}, 1},
        {"no ret", {STMT(BPF_LD | BPF_IMM, 0)}, 1},
        {"half load", {STMT(BPF_LD | BPF_H | BPF_ABS, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"byte load", {STMT(BPF_LD | BPF_B | BPF_ABS, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"load past", {STMT(BPF_LD | BPF_W | BPF_ABS, 64), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"load unaligned", {STMT(BPF_LD | BPF_W | BPF_ABS, 2), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"load ancillary", {STMT(BPF_LD | BPF_W | BPF_ABS, 0xfffff000), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"load unknown ancillary", {STMT(BPF_LD | BPF_W | BPF_ABS, 0xfffff100), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"indirect", {STMT(BPF_LD | BPF_W | BPF_IND, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"msh", {STMT(BPF_LDX | BPF_B | BPF_MSH, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"mod k", {STMT(BPF_ALU | BPF_MOD | BPF_K, 3), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"mod x", {STMT(BPF_ALU | BPF_MOD | BPF_X, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"div zero", {STMT(BPF_ALU | BPF_DIV | BPF_K, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"lsh 32", {STMT(BPF_ALU | BPF_LSH | BPF_K, 32), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"rsh 33", {STMT(BPF_ALU | BPF_RSH | BPF_K, 33), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"mem 16", {STMT(BPF_ST, 16), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"load unset", {STMT(BPF_LD | BPF_MEM, 3), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"load one path",
         {JUMP(BPF_JMP | BPF_JEQ | BPF_K, 0, 0, 1), STMT(BPF_ST, 0), STMT(BPF_LDX | BPF_MEM, 0),
          STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)},
         4},
        {"ja past", {JUMP(BPF_JMP | BPF_JA, 1, 0, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
        {"jeq past", {JUMP(BPF_JMP | BPF_JEQ | BPF_K, 0, 1, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
        {"jset x past", {JUMP(BPF_JMP | BPF_JSET | BPF_X, 0, 0, 2), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
        {"alu with stray bit", {STMT(BPF_ALU | BPF_ADD | BPF_K | 0x100, 0), STMT(BPF_RET | BPF_A, 0)}, 2},
        {"misc unknown", {STMT(BPF_MISC | 0x40, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
        {"ldx abs", {STMT(BPF_LDX | BPF_W | BPF_ABS, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
        {"jmp code 0x75", {STMT(0x75, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)}, 2},
    };
    for (unsigned i = 0; i < sizeof cases / sizeof cases[0]; i++) {
        printf("refused %s %ld\n", cases[i].name, install(0, cases[i].program, cases[i].length));
    }
    struct sock_filter allow[] = {STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)};
    struct sock_fprog null = {1, 0};
    printf("null %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, (long)&null));
    struct sock_fprog fault = {1, (struct sock_filter *)8};
    printf("filter fault %ld\n", call(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, (long)&fault));
    printf("tsync listener %ld\n", install(SECCOMP_FILTER_FLAG_TSYNC | SECCOMP_FILTER_FLAG_NEW_LISTENER, allow, 1));
    printf("killable alone %ld\n", install(SECCOMP_FILTER_FLAG_WAIT_KILLABLE_RECV, allow, 1));
    printf("unknown flag %ld\n", install(64, allow, 1));
}

static void accepted(void) {
    struct sock_filter allow[] = {STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)};
    struct sock_filter unreachable[] = {
        STMT(BPF_LD | BPF_IMM, 0), STMT(BPF_ST, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
        STMT(BPF_LD | BPF_MEM, 0), STMT(BPF_RET | BPF_A, 0)};
    struct sock_filter lengths[] = {
        STMT(BPF_LD | BPF_W | BPF_LEN, 0), STMT(BPF_LDX | BPF_W | BPF_LEN, 0),
        JUMP(BPF_JMP | BPF_JA, 0, 0, 0), STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW)};
    printf("allow %ld\n", install(0, allow, 1));
    printf("tsync %ld\n", install(SECCOMP_FILTER_FLAG_TSYNC, allow, 1));
    printf("log flag %ld\n", install(SECCOMP_FILTER_FLAG_LOG, allow, 1));
    printf("spec allow %ld\n", install(SECCOMP_FILTER_FLAG_SPEC_ALLOW, allow, 1));
    printf("tsync esrch %ld\n", install(SECCOMP_FILTER_FLAG_TSYNC_ESRCH, allow, 1));
    printf("unreachable load %ld\n", install(0, unreachable, 5));
    printf("lengths %ld\n", install(0, lengths, 4));
    struct sock_fprog fprog = {1, allow};
    printf("prctl filter %d\n", prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &fprog, 7, 7));
}

// Installs a filter that runs `op` on getppid's first two arguments, as A and X (or K), and answers
// getppid with the 12 bits of the result its third argument shifts down to. Each filter answers only
// calls whose fourth argument is its own tag, so earlier ones let later calls through.
static void alu(unsigned op, unsigned k, unsigned a, unsigned x) {
    static unsigned tag;
    tag++;
    struct sock_filter program[] = {
        STMT(BPF_LD | BPF_W | BPF_ABS, NR),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_getppid, 0, 14),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(3)),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, tag, 0, 12),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(1)),
        STMT(BPF_MISC | BPF_TAX, 0),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(0)),
        STMT(op, k),
        STMT(BPF_ST, 5),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(2)),
        STMT(BPF_MISC | BPF_TAX, 0),
        STMT(BPF_LD | BPF_MEM, 5),
        STMT(BPF_ALU | BPF_RSH | BPF_X, 0),
        STMT(BPF_ALU | BPF_AND | BPF_K, 0xfff),
        STMT(BPF_ALU | BPF_OR | BPF_K, SECCOMP_RET_ERRNO),
        STMT(BPF_RET | BPF_A, 0),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    long installed = install(0, program, 17);
    unsigned result = 0;
    for (int shift = 0; shift < 32; shift += 12) {
        long r = call4(SYS_getppid, a, x, shift, tag);
        result |= (unsigned)(-r) << shift;
    }
    printf("alu %x %x %x %x: %ld %x\n", op, k, a, x, installed, result);
}

static void arithmetic(void) {
    static const unsigned values[][2] = {
        {0, 0}, {1, 1}, {7, 3}, {0xffffffff, 1}, {0x80000000, 31}, {0x12345678, 32}, {0xdeadbeef, 33},
        {5, 0xffffffff}, {0xffffffff, 0xffffffff},
    };
    static const unsigned ops[] = {BPF_ADD, BPF_SUB, BPF_MUL, BPF_DIV, BPF_OR, BPF_AND, BPF_XOR, BPF_LSH, BPF_RSH};
    for (unsigned o = 0; o < sizeof ops / sizeof ops[0]; o++) {
        for (unsigned v = 0; v < sizeof values / sizeof values[0]; v++) {
            unsigned a = values[v][0], x = values[v][1];
            if (ops[o] != BPF_DIV || x != 0) {
                alu(BPF_ALU | ops[o] | BPF_X, 0, a, x);
            }
            unsigned k = x;
            if ((ops[o] == BPF_DIV && k == 0) || ((ops[o] == BPF_LSH || ops[o] == BPF_RSH) && k >= 32)) {
                continue;
            }
            alu(BPF_ALU | ops[o] | BPF_K, k, a, 0);
        }
    }
    alu(BPF_ALU | BPF_NEG, 0, 5, 0);
    alu(BPF_ALU | BPF_NEG, 0, 0, 0);
    alu(BPF_MISC | BPF_TXA, 0, 1, 0x4321);
    alu(BPF_LD | BPF_IMM, 0xabcdef12, 1, 2);
}

// Reads the seccomp data: the architecture, the call number's low half, an argument's high half and
// the address after the system call instruction.
static void data(void) {
    struct sock_filter program[] = {
        STMT(BPF_LD | BPF_W | BPF_ABS, NR),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_getpgid, 0, 16),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(1)),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, 1, 0, 4),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARCH),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_AARCH64, 0, 1),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | 1),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | 2),
        JUMP(BPF_JMP | BPF_JEQ | BPF_K, 2, 0, 3),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(0) + 4),
        STMT(BPF_ALU | BPF_OR | BPF_K, SECCOMP_RET_ERRNO),
        STMT(BPF_RET | BPF_A, 0),
        STMT(BPF_LD | BPF_W | BPF_ABS, IP),
        STMT(BPF_MISC | BPF_TAX, 0),
        STMT(BPF_LD | BPF_W | BPF_ABS, ARG(0)),
        JUMP(BPF_JMP | BPF_JEQ | BPF_X, 0, 0, 1),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | 3),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | 4),
        STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    printf("data filter %ld\n", install(0, program, 19));
    printf("data arch %ld\n", call(SYS_getpgid, 0, 1, 0));
    printf("data high arg %ld\n", call(SYS_getpgid, 0x00070000000L, 2, 0));
    printf("data high arg 2 %ld\n", call(SYS_getpgid, 0xfff00000000L, 2, 0));
    register long x8 __asm__("x8") = (1L << 32) | SYS_getpgid;
    register long x0 __asm__("x0");
    register long x1 __asm__("x1") = 3;
    __asm__ volatile("adr x0, 1f\n svc 0\n1:" : "=&r"(x0) : "r"(x8), "r"(x1) : "memory");
    printf("data address and high number %ld\n", x0);
}

static void stacking(void) {
    printf("errno 11 %ld\n", only(SYS_getuid, SECCOMP_RET_ERRNO | 11));
    printf("getuid %ld\n", call(SYS_getuid, 0, 0, 0));
    printf("trace %ld\n", only(SYS_getuid, SECCOMP_RET_TRACE));
    printf("getuid %ld\n", call(SYS_getuid, 0, 0, 0));
    printf("errno 22 %ld\n", only(SYS_getuid, SECCOMP_RET_ERRNO | 22));
    printf("getuid %ld\n", call(SYS_getuid, 0, 0, 0));
    printf("log %ld\n", only(SYS_getuid, SECCOMP_RET_LOG));
    printf("getuid %ld\n", call(SYS_getuid, 0, 0, 0));
    printf("trace gid %ld\n", only(SYS_getgid, SECCOMP_RET_TRACE));
    printf("getgid %ld\n", call(SYS_getgid, 0, 0, 0));
    printf("notify egid %ld\n", only(SYS_getegid, SECCOMP_RET_USER_NOTIF));
    printf("getegid %ld\n", call(SYS_getegid, 0, 0, 0));
    printf("log euid %ld\n", only(SYS_geteuid, SECCOMP_RET_LOG));
    printf("geteuid %ld\n", call(SYS_geteuid, 0, 0, 0));
    printf("errno big %ld\n", only(SYS_gettid, SECCOMP_RET_ERRNO | 5000));
    printf("gettid %ld\n", call(SYS_gettid, 0, 0, 0));
    printf("errno zero %ld\n", only(SYS_getpid, SECCOMP_RET_ERRNO));
    printf("getpid %ld\n", call(SYS_getpid, 0, 0, 0));
    printf("mount allow %ld\n", call(SYS_mount, 0, 0, 0));
    printf("errno mount %ld\n", only(SYS_mount, SECCOMP_RET_ERRNO | 33));
    printf("mount %ld\n", call(SYS_mount, 0, 0, 0));
    printf("errno 500 %ld\n", only(500, SECCOMP_RET_ERRNO | 44));
    printf("500 %ld\n", call(500, 0, 0, 0));
    printf("errno 1000 %ld\n", only(1000, SECCOMP_RET_TRACE));
    printf("1000 %ld\n", call(1000, 0, 0, 0));
    printf("errno seccomp %ld\n", only(SYS_seccomp, SECCOMP_RET_ERRNO | 55));
    printf("seccomp %ld\n", only(SYS_getuid, SECCOMP_RET_ALLOW));
}

static void ending(const char *how) {
    if (!strcmp(how, "trap")) {
        printf("trap %ld\n", only(SYS_getppid, SECCOMP_RET_TRAP | 9));
    } else if (!strcmp(how, "trapignored")) {
        signal(SIGSYS, SIG_IGN);
        sigset_t set;
        sigemptyset(&set);
        sigaddset(&set, SIGSYS);
        sigprocmask(SIG_BLOCK, &set, 0);
        printf("trap %ld\n", only(SYS_getppid, SECCOMP_RET_TRAP));
    } else if (!strcmp(how, "killprocess")) {
        printf("kill %ld\n", only(SYS_getppid, SECCOMP_RET_KILL_PROCESS));
    } else if (!strcmp(how, "killthread")) {
        printf("kill %ld\n", only(SYS_getppid, SECCOMP_RET_KILL_THREAD | 3));
    } else if (!strcmp(how, "unknownaction")) {
        printf("unknown %ld\n", only(SYS_getppid, 0x00010000));
    } else if (!strcmp(how, "divzero")) {
        alu(BPF_ALU | BPF_DIV | BPF_X, 0, 7, 1);
        alu(BPF_ALU | BPF_DIV | BPF_X, 0, 7, 0);
    } else if (!strcmp(how, "limit")) {
        static struct sock_filter program[4096];
        for (int i = 0; i < 4095; i++) {
            program[i] = STMT(BPF_LD | BPF_IMM, i);
        }
        program[4095] = STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW);
        for (int i = 0; i < 7; i++) {
            printf("limit 4096 %ld\n", install(0, program, 4096));
        }
        printf("limit 3033 %ld\n", install(0, program + 4096 - 3033, 3033));
        printf("limit 3032 %ld\n", install(0, program + 4096 - 3032, 3032));
        printf("limit 1 after %ld\n", install(0, program + 4095, 1));
        status();
        return;
    } else if (!strcmp(how, "execve")) {
        printf("errno ppid %ld\n", only(SYS_getppid, SECCOMP_RET_ERRNO | 66));
        execl("/proc/self/exe", "filter", "after", (char *)0);
        printf("exec failed %d\n", errno);
        return;
    } else if (!strcmp(how, "after")) {
        printf("after exec getppid %ld\n", call(SYS_getppid, 0, 0, 0));
        status();
        return;
    } else {
        return;
    }
    printf("getppid %ld\n", call(SYS_getppid, 0, 0, 0));
}

int main(int argc, char **argv) {
    setvbuf(stdout, 0, _IONBF, 0);
    if (argc > 1) {
        if (strcmp(argv[1], "after")) {
            prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
        }
        ending(argv[1]);
        return 0;
    }
    status();
    operations();
    before_no_new_privs();
    printf("nnp %d\n", prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0));
    refused();
    status();
    accepted();
    status();
    arithmetic();
    data();
    stacking();
    status();
    return 0;
}
