// Securebits: setting them and their locks, keeping powers across a change of ids, no fix-up of
// powers at all, root gaining nothing from a program, ambient powers that may not be raised, and
// what an exec keeps.
#define _GNU_SOURCE
#include <errno.h>
#include <linux/capability.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/fsuid.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#define R(what, call)                                                                              \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long r = (call);                                                                           \
        printf("%s = %ld %s\n", what, r, r < 0 ? strerror(errno) : "");                            \
    } while (0)

static void powers(const char *label) {
    struct __user_cap_header_struct header = {_LINUX_CAPABILITY_VERSION_3, 0};
    struct __user_cap_data_struct data[2];
    syscall(SYS_capget, &header, data);
    printf("%s: uid %d euid %d effective %08x permitted %08x bits %ld\n", label, getuid(), geteuid(),
           data[0].effective, data[0].permitted, (long)prctl(PR_GET_SECUREBITS));
}

static void child(void (*body)(void)) {
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        body();
        fflush(stdout);
        _exit(0);
    }
    waitpid(pid, NULL, 0);
}

static void keep_by_bits(void) {
    R("set keep caps bit", prctl(PR_SET_SECUREBITS, 1 << 4));
    R("get keepcaps", prctl(PR_GET_KEEPCAPS));
    R("setresuid 1000", setresuid(1000, 1000, 1000));
    powers("kept across setresuid");
}

static void locks(void) {
    R("set keep caps and its lock", prctl(PR_SET_SECUREBITS, (1 << 4) | (1 << 5)));
    R("keepcaps 0 when locked", prctl(PR_SET_KEEPCAPS, 0));
    R("lift the lock", prctl(PR_SET_SECUREBITS, 1 << 4));
    R("change the locked bit", prctl(PR_SET_SECUREBITS, 1 << 5));
    R("add another bit", prctl(PR_SET_SECUREBITS, (1 << 4) | (1 << 5) | (1 << 0)));
    R("all bits and locks", prctl(PR_SET_SECUREBITS, 0xfff));
    R("unknown bit", prctl(PR_SET_SECUREBITS, 1 << 12));
    powers("locked");
}

static void no_fixup(void) {
    R("no setuid fixup", prctl(PR_SET_SECUREBITS, 1 << 2));
    R("seteuid 1000", seteuid(1000));
    powers("after seteuid");
    R("setfsuid", setfsuid(1000));
    powers("after setfsuid");
    R("setresuid 1000", setresuid(1000, 1000, 1000));
    powers("after setresuid");
}

static void noroot(void) {
    R("noroot", prctl(PR_SET_SECUREBITS, 1 << 0));
    execl("/proc/self/exe", "securebits", "after-exec", (char *)NULL);
}

static void keep_then_exec(void) {
    R("keep caps, no ambient raise", prctl(PR_SET_SECUREBITS, (1 << 4) | (1 << 6) | (1 << 8)));
    R("ambient raise", prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, CAP_CHOWN, 0, 0));
    execl("/proc/self/exe", "securebits", "after-exec", (char *)NULL);
}

static void without_setpcap(void) {
    R("drop setpcap", prctl(PR_CAPBSET_DROP, CAP_SETPCAP));
    struct __user_cap_header_struct header = {_LINUX_CAPABILITY_VERSION_3, 0};
    struct __user_cap_data_struct data[2];
    syscall(SYS_capget, &header, data);
    data[0].effective &= ~(1u << CAP_SETPCAP);
    data[0].permitted &= ~(1u << CAP_SETPCAP);
    R("capset", syscall(SYS_capset, &header, data));
    R("set bits without setpcap", prctl(PR_SET_SECUREBITS, 1 << 4));
    R("keepcaps without setpcap", prctl(PR_SET_KEEPCAPS, 1));
    powers("without setpcap");
}

int main(int argc, char **argv) {
    if (argc > 1) {
        powers("after exec");
        return 0;
    }
    powers("start");
    child(keep_by_bits);
    child(locks);
    child(no_fixup);
    child(noroot);
    child(keep_then_exec);
    child(without_setpcap);
    return 0;
}
