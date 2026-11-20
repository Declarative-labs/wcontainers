// clone's remaining combinations: processes that share the caller's memory while both run
// (CLONE_VM without CLONE_VFORK), processes that share its signal handlers (CLONE_SIGHAND), and
// threads with process descriptors (CLONE_PIDFD with CLONE_THREAD, pidfd_open's PIDFD_THREAD).

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/futex.h>
#include <poll.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#define PIDFD_THREAD O_EXCL
#ifndef P_PIDFD
#define P_PIDFD 3
#endif
#define THREAD (CLONE_VM | CLONE_SIGHAND | CLONE_THREAD | CLONE_FILES | CLONE_FS | CLONE_SYSVSEM)

static char stack[1 << 16] __attribute__((aligned(16)));
static char stack2[1 << 16] __attribute__((aligned(16)));
static volatile int word, child_tid, turn, handled;
static volatile long seen[8];
static int pipes[2][2];

static long raw(long number, long a, long b, long c) { return syscall(number, a, b, c); }

// clone as the system call makes it: the child runs `fn` on `top`, then ends with its result.
static long raw_clone(long flags, void *top, void *ptid, volatile int *ctid, int (*fn)(void)) {
    register long x0 __asm__("x0") = flags;
    register long x1 __asm__("x1") = (long)top;
    register long x2 __asm__("x2") = (long)ptid;
    register long x3 __asm__("x3") = 0;
    register long x4 __asm__("x4") = (long)ctid;
    register long x8 __asm__("x8") = SYS_clone;
    register long x9 __asm__("x9") = (long)fn;
    __asm__ volatile("svc #0\n"
                     "cbnz x0, 1f\n"
                     "blr x9\n"
                     "mov x8, #93\n"
                     "svc #0\n"
                     "1:\n"
                     : "+r"(x0)
                     : "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x8), "r"(x9)
                     : "memory", "x30");
    return x0;
}

static void wait_word(volatile int *at, int value) {
    while (*at == value) syscall(SYS_futex, at, FUTEX_WAIT, value, NULL);
}

static void wake_word(volatile int *at) { syscall(SYS_futex, at, FUTEX_WAKE, 1); }

// Waits until a thread id address is cleared, each wait on the value just read.
static void wait_cleared(volatile int *at) {
    int value;
    while ((value = *at) != 0) syscall(SYS_futex, at, FUTEX_WAIT, value, NULL);
}

static short ready(int fd) {
    struct pollfd p = {fd, POLLIN | POLLOUT | POLLPRI, 0};
    int n = poll(&p, 1, 0);
    return n < 0 ? -errno : p.revents;
}

// A process on the caller's memory: it maps a page the caller then sees, unmaps one the caller
// made, and takes turns with the caller through a futex in the memory they share.
static void *given, *made_by_child;
static int shares_memory(void) {
    word = 1;
    made_by_child = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    strcpy(made_by_child, "written by the child");
    seen[0] = raw(SYS_munmap, (long)given, 4096, 0);
    seen[1] = raw(SYS_getpid, 0, 0, 0) != raw(SYS_getppid, 0, 0, 0);
    for (int i = 0; i < 5; i++) {
        wait_word(&turn, 0);
        seen[2] += turn;
        turn = 0;
        wake_word(&turn);
    }
    return 3;
}

static void handler(int signal) { handled += signal; }

// A process sharing the caller's handlers: it sets one the caller then sees, and is sent one. It
// also clears the memory's dumpable flag, which the caller shares.
static int shares_handlers(void) {
    struct sigaction action = {0};
    action.sa_handler = handler;
    sigaction(SIGUSR2, &action, NULL);
    prctl(PR_SET_DUMPABLE, 0);
    word = 2;
    wake_word(&word);
    while (handled == 0) raw(SYS_sched_yield, 0, 0, 0);
    return 0;
}

// A process sharing the caller's handlers starts a program, whose handlers are its own.
static char *self_path;
static int starts_program(void) {
    char *argv[] = {self_path, "program", NULL};
    execve(self_path, argv, NULL);
    return 99;
}

// A thread that waits for the caller to let it end.
static int waits_for_word(void) {
    wait_word(&turn, 0);
    return 0;
}

// The leader of a child process ends alone; its other thread reports that, then waits to be told
// to end.
static volatile int leader_tid;
static int outlives_leader(void) {
    pid_t leader = raw(SYS_getpid, 0, 0, 0);
    wait_word(&leader_tid, leader);
    // The first thread has let go of its memory; wait until it has ended.
    char path[64], stat[256];
    snprintf(path, sizeof path, "/proc/%d/stat", leader);
    for (;;) {
        int fd = raw(SYS_openat, AT_FDCWD, (long)path, O_RDONLY);
        long got = raw(SYS_read, fd, (long)stat, sizeof stat - 1);
        raw(SYS_close, fd, 0, 0);
        stat[got > 0 ? got : 0] = 0;
        char *state = strrchr(stat, ')');
        if (state && state[2] == 'Z')
            break;
        struct timespec pause = {0, 1000 * 1000};
        raw(SYS_nanosleep, (long)&pause, 0, 0);
    }
    char byte = 1;
    write(pipes[0][1], &byte, 1);
    read(pipes[1][0], &byte, 1);
    syscall(SYS_exit_group, 4);
    return 0;
}

#define IOC(dir, nr, size) (((unsigned)(dir) << 30) | ((unsigned)(size) << 16) | (0xffu << 8) | (nr))
static void info(const char *what, int fd, pid_t thread) {
    unsigned char buffer[80] = {0};
    unsigned long long asked = 1 | 2 | 8;
    memcpy(buffer, &asked, 8);
    if (ioctl(fd, IOC(3, 11, 80), buffer) < 0) {
        printf("%s info: errno %d\n", what, errno);
        return;
    }
    unsigned v[3];
    memcpy(v, buffer + 16, 12);
    printf("%s info: pid is the thread %d tgid is the process %d ppid is the parent %d\n", what,
           (pid_t)v[0] == thread, (pid_t)v[1] == getpid(), (pid_t)v[2] == getppid());
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    self_path = argv[0];
    if (argc > 1) {
        struct sigaction now;
        sigaction(SIGUSR1, NULL, &now);
        printf("program: SIGUSR1 %s\n", now.sa_handler == SIG_IGN ? "ignored"
                                        : now.sa_handler == SIG_DFL ? "default" : "handled");
        sigaction(SIGUSR2, NULL, &now);
        printf("program: SIGUSR2 %s\n", now.sa_handler == SIG_DFL ? "default" : "other");
        return 0;
    }

    // CLONE_VM without CLONE_VFORK: both run on one memory.
    given = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    child_tid = -1;
    long child = raw_clone(CLONE_VM | CLONE_CHILD_CLEARTID | SIGCHLD, stack + sizeof stack, 0,
                           &child_tid, shares_memory);
    printf("vm clone made %d\n", child > 0);
    for (int i = 1; i <= 5; i++) {
        turn = i;
        wake_word(&turn);
        wait_word(&turn, i);
    }
    int status;
    printf("waited %d status %x\n", waitpid(child, &status, 0) == child, status);
    printf("child unmapped %ld, own process %ld, turns %ld, word %d, tid cleared %d\n", seen[0],
           seen[1], seen[2], word, child_tid == 0);
    printf("caller reads the child's page: %s\n", (char *)made_by_child);
    printf("caller's page gone: %d\n", msync(given, 4096, MS_ASYNC) < 0 ? errno : 0);

    // CLONE_VM | CLONE_SIGHAND: handlers set in one are the other's.
    word = 0;
    child = raw_clone(CLONE_VM | CLONE_SIGHAND | SIGCHLD, stack + sizeof stack, 0, 0,
                      shares_handlers);
    printf("sighand clone made %d\n", child > 0);
    wait_word(&word, 0);
    struct sigaction now;
    sigaction(SIGUSR2, NULL, &now);
    printf("caller sees the child's handler %d\n", now.sa_handler == handler);
    printf("caller's dumpable after the child cleared it %d\n", prctl(PR_GET_DUMPABLE));
    prctl(PR_SET_DUMPABLE, 1);
    kill(child, SIGUSR2);
    waitpid(child, &status, 0);
    printf("child handled %d status %x\n", handled, status);

    // The child starts a program: its handlers are reset, the caller's stay.
    signal(SIGUSR1, SIG_IGN);
    child = raw_clone(CLONE_VM | CLONE_SIGHAND | SIGCHLD, stack + sizeof stack, 0, 0,
                      starts_program);
    waitpid(child, &status, 0);
    sigaction(SIGUSR2, NULL, &now);
    printf("after the child's program: status %x caller's handler kept %d\n", status,
           now.sa_handler == handler);
    signal(SIGUSR1, SIG_DFL);
    signal(SIGUSR2, SIG_DFL);

    // A process sharing handlers but not memory is refused, as is a thread without them.
    printf("sighand without vm %ld\n", -raw_clone(CLONE_SIGHAND | SIGCHLD, stack + sizeof stack, 0, 0, 0));

    // A thread with a process descriptor.
    int pidfd = -1;
    turn = 0;
    child_tid = -1;
    long thread = raw_clone(THREAD | CLONE_PIDFD | CLONE_CHILD_CLEARTID, stack + sizeof stack,
                            &pidfd, &child_tid, waits_for_word);
    printf("thread with pidfd made %d descriptor %d\n", thread > 0, pidfd > 2);
    printf("  flags %o ready %x\n", fcntl(pidfd, F_GETFL), ready(pidfd));
    printf("  signal 0: %ld\n", syscall(SYS_pidfd_send_signal, pidfd, 0, NULL, 0));
    info("  live", pidfd, thread);
    siginfo_t si;
    printf("  waitid: %d errno %d\n",
           (int)syscall(SYS_waitid, P_PIDFD, pidfd, &si, WEXITED | WNOHANG, NULL), errno);
    int opened = syscall(SYS_pidfd_open, thread, PIDFD_THREAD);
    printf("  pidfd_open thread %d plain %d\n", opened > 2,
           syscall(SYS_pidfd_open, thread, 0) < 0 ? errno : 0);
    turn = 1;
    wake_word(&turn);
    wait_cleared(&child_tid);
    // Its number is cleared before Linux lets go of it; it is gone when its directory is.
    char task[64];
    snprintf(task, sizeof task, "/proc/self/task/%ld", thread);
    while (access(task, F_OK) == 0) raw(SYS_sched_yield, 0, 0, 0);
    struct pollfd p = {pidfd, POLLIN, 0};
    printf("  after its end: poll %d ready %x opened %x\n", poll(&p, 1, -1), ready(pidfd),
           ready(opened));
    errno = 0;
    printf("  signal 0: %ld errno %d\n", syscall(SYS_pidfd_send_signal, pidfd, 0, NULL, 0), errno);
    info("  ended", pidfd, thread);
    printf("  pidfd_open now %d\n", syscall(SYS_pidfd_open, thread, PIDFD_THREAD) < 0 ? errno : 0);
    close(pidfd);
    close(opened);

    // The first thread's descriptor with PIDFD_THREAD, and the process's, when the first thread
    // ends alone and when the process ends.
    pipe(pipes[0]);
    pipe(pipes[1]);
    child = fork();
    if (child == 0) {
        leader_tid = getpid();
        syscall(SYS_set_tid_address, &leader_tid);
        raw_clone(THREAD, stack2 + sizeof stack2, 0, 0, outlives_leader);
        syscall(SYS_exit, 0);
    }
    int as_thread = syscall(SYS_pidfd_open, child, PIDFD_THREAD);
    int as_process = syscall(SYS_pidfd_open, child, 0);
    char byte;
    read(pipes[0][0], &byte, 1);
    printf("first thread ended alone: thread %x process %x\n", ready(as_thread), ready(as_process));
    write(pipes[1][1], &byte, 1);
    p.fd = as_process;
    poll(&p, 1, -1);
    printf("process ended: thread %x process %x\n", ready(as_thread), ready(as_process));
    waitpid(child, &status, 0);
    printf("reaped (status %x): thread %x process %x\n", status, ready(as_thread),
           ready(as_process));
    return 0;
}
