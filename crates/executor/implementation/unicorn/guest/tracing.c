// ptrace as the six LTP tests and a tracer like them use it: PTRACE_TRACEME and PTRACE_ATTACH, the
// stop at each signal and how a wait and SIGCHLD report it, PTRACE_CONT, PTRACE_KILL and
// PTRACE_DETACH with and without a signal, PEEK and POKE, GETSIGINFO and SETSIGINFO, the refusals,
// /proc's TracerPid and state, and a tracer that ends. Only the parent prints; children write what
// they saw into a pipe the parent prints after them. Numbers that change from run to run (process
// ids) are printed as who they name.
#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define TRACEME 0
#define PEEKTEXT 1
#define PEEKDATA 2
#define PEEKUSR 3
#define POKETEXT 4
#define POKEDATA 5
#define POKEUSR 6
#define CONT 7
#define KILL 8
#define ATTACH 16
#define DETACH 17
#define GETSIGINFO 0x4202
#define SETSIGINFO 0x4203

static long pt(long request, long pid, long address, long data) {
    return syscall(SYS_ptrace, request, pid, address, data);
}

static const char *name(int error) {
    switch (error) {
    case EPERM: return "EPERM";
    case ESRCH: return "ESRCH";
    case EIO: return "EIO";
    case EFAULT: return "EFAULT";
    case EINVAL: return "EINVAL";
    case ECHILD: return "ECHILD";
    default: return "other";
    }
}

// Prints what a call answered: its value, or -1 and the error's name.
#define SHOW(label, call)                                        \
    do {                                                         \
        errno = 0;                                               \
        long answer_ = (call);                                   \
        if (answer_ == -1)                                       \
            printf("%s: -1 %s\n", label, name(errno));          \
        else                                                     \
            printf("%s: %ld\n", label, answer_);                 \
    } while (0)

static void describe(const char *label, int status) {
    if (WIFEXITED(status))
        printf("%s: exited %d\n", label, WEXITSTATUS(status));
    else if (WIFSIGNALED(status))
        printf("%s: killed %d\n", label, WTERMSIG(status));
    else if (WIFSTOPPED(status))
        printf("%s: stopped %d\n", label, WSTOPSIG(status));
    else
        printf("%s: other %x\n", label, status);
}

static void wait_for(const char *label, pid_t pid, int options) {
    int status = 0;
    pid_t got = waitpid(pid, &status, options);
    if (got == pid)
        describe(label, status);
    else if (got == 0)
        printf("%s: nothing\n", label);
    else
        printf("%s: -1 %s\n", label, name(errno));
}

// Prints what children wrote into `fd`, then closes it.
static void print_pipe(int fd) {
    char text[4096];
    ssize_t length = read(fd, text, sizeof text - 1);
    text[length > 0 ? length : 0] = 0;
    fputs(text, stdout);
    close(fd);
}

// Waits until `pid` sleeps in a call (its /proc state is S), so a signal sent now interrupts it.
static void wait_sleeping(pid_t pid) {
    char path[64], line[512];
    snprintf(path, sizeof path, "/proc/%d/stat", pid);
    for (int tries = 0; tries < 500; tries++) {
        FILE *file = fopen(path, "r");
        char state = 0;
        if (file && fgets(line, sizeof line, file)) {
            char *end = strrchr(line, ')');
            if (end && end[1] == ' ')
                state = end[2];
        }
        if (file)
            fclose(file);
        if (state == 'S')
            return;
        usleep(10000);
    }
}

static pid_t fork_flushed(void) {
    fflush(stdout);
    return fork();
}

static volatile uint64_t marker[2] = {0x1122334455667788ull, 0x8877665544332211ull};
static volatile int handled;
static volatile int handled_code, handled_errno, handled_from_parent;

static void count(int signal) {
    (void)signal;
    handled++;
}

static void record(int signal, siginfo_t *info, void *context) {
    (void)signal;
    (void)context;
    handled++;
    handled_code = info->si_code;
    handled_errno = info->si_errno;
    handled_from_parent = info->si_pid == getppid();
}

static void on(int signal, void (*handler)(int, siginfo_t *, void *)) {
    struct sigaction action = {0};
    action.sa_sigaction = handler;
    action.sa_flags = SA_SIGINFO | SA_RESTART;
    sigaction(signal, &action, 0);
}

// A stop, the wait and SIGCHLD reports of it, the requests on a stopped child, and going on
// without the signal, with it, and past a fatal one.
static volatile int chld_code, chld_status, chld_count;
static volatile pid_t chld_pid;

static void on_child(int signal, siginfo_t *info, void *context) {
    (void)signal;
    (void)context;
    chld_count++;
    chld_code = info->si_code;
    chld_status = info->si_status;
    chld_pid = info->si_pid;
}

static void stops_and_requests(void) {
    printf("== stops and requests\n");
    int pipe_fds[2];
    pipe(pipe_fds);
    chld_count = 0;
    on(SIGCHLD, on_child);
    pid_t child = fork_flushed();
    if (child == 0) {
        close(pipe_fds[0]);
        signal(SIGUSR1, count);
        dprintf(pipe_fds[1], "child traceme: %ld\n", pt(TRACEME, 0, 0, 0));
        errno = 0;
        long again = pt(TRACEME, 0, 0, 0);
        dprintf(pipe_fds[1], "child traceme again: %ld %s\n", again, name(errno));
        kill(getpid(), SIGUSR1);
        dprintf(pipe_fds[1], "child handled after going on without it: %d\n", handled);
        kill(getpid(), SIGUSR1);
        dprintf(pipe_fds[1], "child handled after going on with it: %d\n", handled);
        kill(getpid(), SIGTERM);
        dprintf(pipe_fds[1], "child lived past SIGTERM, markers %llx %llx\n",
                (unsigned long long)marker[0], (unsigned long long)marker[1]);
        _exit(3);
    }
    close(pipe_fds[1]);
    siginfo_t info = {0};
    int answer = waitid(P_PID, child, &info, WEXITED | WSTOPPED | WNOWAIT);
    printf("waitid kept: %d code %d status %d is child %d\n", answer, info.si_code,
           info.si_status, info.si_pid == child);
    wait_for("wait without WUNTRACED", child, 0);
    wait_for("wait again", child, WNOHANG);
    printf("SIGCHLD: count %d code %d status %d is child %d\n", chld_count, chld_code,
           chld_status, chld_pid == child);
    siginfo_t taken;
    memset(&taken, 0, sizeof taken);
    SHOW("getsiginfo", pt(GETSIGINFO, child, 0, (long)&taken));
    printf("siginfo: signal %d code %d from child %d\n", taken.si_signo, taken.si_code,
           taken.si_pid == child);
    SHOW("getsiginfo to a bad address", pt(GETSIGINFO, child, 0, 1));
    SHOW("setsiginfo from a bad address", pt(SETSIGINFO, child, 0, -1));
    uint64_t word = 0;
    SHOW("peekdata", pt(PEEKDATA, child, (long)&marker[0], (long)&word));
    printf("peeked %llx\n", (unsigned long long)word);
    word = 0;
    SHOW("peektext", pt(PEEKTEXT, child, (long)&marker[1], (long)&word));
    printf("peeked %llx\n", (unsigned long long)word);
    SHOW("peekdata to a bad address", pt(PEEKDATA, child, (long)&marker[0], 1));
    SHOW("peekdata at 0", pt(PEEKDATA, child, 0, (long)&word));
    SHOW("peektext at -1", pt(PEEKTEXT, child, -1, (long)&word));
    SHOW("pokedata", pt(POKEDATA, child, (long)&marker[0], 0x0102030405060708l));
    SHOW("poketext", pt(POKETEXT, child, (long)&marker[1], 0x0a0b0c0d0e0f1011l));
    SHOW("pokedata at 0", pt(POKEDATA, child, 0, 1));
    SHOW("poketext at 3", pt(POKETEXT, child, 3, 1));
    SHOW("peekusr", pt(PEEKUSR, child, 0, (long)&word));
    SHOW("pokeusr", pt(POKEUSR, child, 0, 0));
    SHOW("an unknown request", pt(12345, child, 0, 0));
    char path[64], line[256];
    snprintf(path, sizeof path, "/proc/%d/status", child);
    FILE *status = fopen(path, "r");
    while (status && fgets(line, sizeof line, status)) {
        if (strncmp(line, "State:", 6) == 0)
            fputs(line, stdout);
        if (strncmp(line, "TracerPid:", 10) == 0)
            printf("TracerPid is me: %d\n", atoi(line + 10) == getpid());
    }
    if (status)
        fclose(status);
    SHOW("cont with signal 65", pt(CONT, child, 0, 65));
    SHOW("cont", pt(CONT, child, 0, 0));
    wait_for("second stop", child, 0);
    SHOW("cont with SIGUSR1", pt(CONT, child, 0, SIGUSR1));
    wait_for("third stop", child, 0);
    SHOW("cont past SIGTERM", pt(CONT, child, 0, 0));
    wait_for("end", child, 0);
    print_pipe(pipe_fds[0]);
    signal(SIGCHLD, SIG_DFL);
}

// A signal the tracer gives that the child blocks waits for it, and stops it again once
// unblocked; a signal other than the one it stopped for comes from the tracer.
static void blocked_and_changed(void) {
    printf("== blocked and changed\n");
    int pipe_fds[2];
    pipe(pipe_fds);
    pid_t child = fork_flushed();
    if (child == 0) {
        close(pipe_fds[0]);
        on(SIGUSR1, record);
        on(SIGUSR2, record);
        sigset_t set;
        sigemptyset(&set);
        sigaddset(&set, SIGUSR2);
        sigprocmask(SIG_BLOCK, &set, 0);
        pt(TRACEME, 0, 0, 0);
        kill(getpid(), SIGUSR1);
        sigset_t pending;
        sigpending(&pending);
        dprintf(pipe_fds[1], "child: handled %d, SIGUSR2 pending %d\n", handled,
                sigismember(&pending, SIGUSR2));
        sigprocmask(SIG_UNBLOCK, &set, 0);
        dprintf(pipe_fds[1], "child: handled %d, code %d, from the tracer %d\n", handled,
                handled_code, handled_from_parent);
        _exit(0);
    }
    close(pipe_fds[1]);
    wait_for("stop", child, 0);
    SHOW("cont with the blocked SIGUSR2", pt(CONT, child, 0, SIGUSR2));
    wait_for("stop once unblocked", child, 0);
    siginfo_t taken;
    memset(&taken, 0, sizeof taken);
    pt(GETSIGINFO, child, 0, (long)&taken);
    printf("siginfo: signal %d code %d from me %d\n", taken.si_signo, taken.si_code,
           taken.si_pid == getpid());
    SHOW("cont with SIGUSR2", pt(CONT, child, 0, SIGUSR2));
    wait_for("end", child, 0);
    print_pipe(pipe_fds[0]);
}

// The information the tracer sets is the one the handler gets.
static void information_set(void) {
    printf("== information set\n");
    int pipe_fds[2];
    pipe(pipe_fds);
    pid_t child = fork_flushed();
    if (child == 0) {
        close(pipe_fds[0]);
        on(SIGUSR1, record);
        pt(TRACEME, 0, 0, 0);
        kill(getpid(), SIGUSR1);
        dprintf(pipe_fds[1], "child: handled %d, errno %d, code %d\n", handled, handled_errno,
                handled_code);
        _exit(0);
    }
    close(pipe_fds[1]);
    wait_for("stop", child, 0);
    siginfo_t taken;
    memset(&taken, 0, sizeof taken);
    pt(GETSIGINFO, child, 0, (long)&taken);
    taken.si_errno = 42;
    SHOW("setsiginfo", pt(SETSIGINFO, child, 0, (long)&taken));
    SHOW("cont with SIGUSR1", pt(CONT, child, 0, SIGUSR1));
    wait_for("end", child, 0);
    print_pipe(pipe_fds[0]);
}

// An untraced child drops a signal it ignores, but keeps one it blocks (Linux's `sig_ignored`);
// the traced child of `ending` stops even for one it ignores.
static void untraced_ignored(void) {
    printf("== untraced, ignored\n");
    int ready[2], go[2];
    pipe(ready);
    pipe(go);
    pid_t child = fork_flushed();
    if (child == 0) {
        signal(SIGUSR1, SIG_IGN);
        signal(SIGUSR2, SIG_IGN);
        sigset_t set;
        sigemptyset(&set);
        sigaddset(&set, SIGUSR2);
        sigprocmask(SIG_BLOCK, &set, 0);
        write(ready[1], "", 1);
        char byte;
        read(go[0], &byte, 1);
        sigset_t pending;
        sigpending(&pending);
        dprintf(ready[1], "child: SIGUSR1 pending %d, SIGUSR2 pending %d\n",
                sigismember(&pending, SIGUSR1), sigismember(&pending, SIGUSR2));
        _exit(0);
    }
    char byte;
    read(ready[0], &byte, 1);
    kill(child, SIGUSR1);
    kill(child, SIGUSR2);
    write(go[1], "", 1);
    wait_for("end", child, 0);
    close(ready[1]);
    print_pipe(ready[0]);
    close(go[0]);
    close(go[1]);
    // An untraced child stops for SIGSTOP; this process, the container's first, has no handler
    // for SIGTERM, which its child's kill therefore leaves it alive through.
    child = fork_flushed();
    if (child == 0) {
        for (;;)
            pause();
    }
    kill(child, SIGSTOP);
    wait_for("untraced, SIGSTOP", child, WUNTRACED);
    kill(child, SIGKILL);
    wait_for("untraced, SIGKILL", child, 0);
    printf("this process is the first: %d\n", getpid() == 1);
    child = fork_flushed();
    if (child == 0) {
        kill(getppid(), SIGTERM);
        _exit(0);
    }
    wait_for("its child sent it SIGTERM", child, 0);
    printf("lived\n");
}

// A signal sent to a child stopped for its tracer waits: the stop stays the one it was, and the
// child stops for the new signal once it goes on.
static void signal_while_stopped(void) {
    printf("== signal while stopped\n");
    pid_t child = fork_flushed();
    if (child == 0) {
        signal(SIGUSR2, count);
        pt(TRACEME, 0, 0, 0);
        kill(getpid(), SIGUSR1);
        _exit(handled);
    }
    wait_for("stop", child, 0);
    kill(child, SIGUSR2);
    usleep(100000);
    wait_for("while stopped", child, WNOHANG);
    siginfo_t taken;
    memset(&taken, 0, sizeof taken);
    pt(GETSIGINFO, child, 0, (long)&taken);
    printf("siginfo: signal %d\n", taken.si_signo);
    SHOW("cont", pt(CONT, child, 0, 0));
    wait_for("stop for the signal sent meanwhile", child, 0);
    SHOW("cont with it", pt(CONT, child, 0, SIGUSR2));
    wait_for("end (its handler ran once)", child, 0);
}

// An ignored signal stops a traced child too; SIGKILL ends a stopped one; PTRACE_KILL ends a
// running one, which no other request reaches; a fatal signal stops first and ends it when given.
static void ending(void) {
    printf("== ending\n");
    pid_t child = fork_flushed();
    if (child == 0) {
        signal(SIGUSR1, SIG_IGN);
        pt(TRACEME, 0, 0, 0);
        kill(getpid(), SIGUSR1);
        _exit(0);
    }
    wait_for("ignored SIGUSR1", child, 0);
    kill(child, SIGKILL);
    wait_for("SIGKILL while stopped", child, 0);

    int pipe_fds[2];
    pipe(pipe_fds);
    child = fork_flushed();
    if (child == 0) {
        close(pipe_fds[0]);
        pt(TRACEME, 0, 0, 0);
        write(pipe_fds[1], "", 1);
        for (;;)
            pause();
    }
    close(pipe_fds[1]);
    char byte;
    read(pipe_fds[0], &byte, 1);
    close(pipe_fds[0]);
    uint64_t word;
    SHOW("peekdata while running", pt(PEEKDATA, child, (long)&marker[0], (long)&word));
    SHOW("cont while running", pt(CONT, child, 0, 0));
    SHOW("detach while running", pt(DETACH, child, 0, 0));
    SHOW("kill while running", pt(KILL, child, 0, 0));
    wait_for("after PTRACE_KILL", child, 0);

    child = fork_flushed();
    if (child == 0) {
        pt(TRACEME, 0, 0, 0);
        kill(getpid(), SIGTERM);
        _exit(0);
    }
    wait_for("SIGTERM", child, 0);
    SHOW("cont with SIGTERM", pt(CONT, child, 0, SIGTERM));
    wait_for("given SIGTERM", child, 0);
}

// PTRACE_ATTACH: who may be attached to, the SIGSTOP it sends, a second tracer refused, and
// PTRACE_DETACH with and without a signal.
static void attaching(void) {
    printf("== attaching\n");
    SHOW("attach to my own process", pt(ATTACH, getpid(), 0, 0));
    SHOW("attach to 0", pt(ATTACH, 0, 0, 0));
    SHOW("attach to nobody", pt(ATTACH, 4194000, 0, 0));
    SHOW("cont of nobody", pt(CONT, 4194000, 0, 0));

    pid_t zombie = fork_flushed();
    if (zombie == 0)
        _exit(5);
    siginfo_t info;
    waitid(P_PID, zombie, &info, WEXITED | WNOWAIT);
    SHOW("attach to a zombie", pt(ATTACH, zombie, 0, 0));
    SHOW("cont of a zombie", pt(CONT, zombie, 0, 0));
    wait_for("zombie", zombie, 0);

    int pipe_fds[2];
    pipe(pipe_fds);
    pid_t child = fork_flushed();
    if (child == 0) {
        close(pipe_fds[0]);
        signal(SIGUSR1, count);
        write(pipe_fds[1], "", 1);
        for (;;) {
            pause();
            dprintf(pipe_fds[1], "child: handled %d\n", handled);
        }
    }
    close(pipe_fds[1]);
    char ready;
    read(pipe_fds[0], &ready, 1);
    wait_sleeping(child);
    SHOW("cont before attaching", pt(CONT, child, 0, 0));
    SHOW("attach", pt(ATTACH, child, 0, 0));
    SHOW("attach again", pt(ATTACH, child, 0, 0));
    wait_for("stop", child, 0);
    siginfo_t taken;
    memset(&taken, 0, sizeof taken);
    pt(GETSIGINFO, child, 0, (long)&taken);
    printf("siginfo: signal %d code %d\n", taken.si_signo, taken.si_code);
    SHOW("detach with signal 65", pt(DETACH, child, 0, 65));
    SHOW("detach with SIGUSR1", pt(DETACH, child, 0, SIGUSR1));
    char text[64];
    ssize_t length = read(pipe_fds[0], text, sizeof text - 1);
    text[length > 0 ? length : 0] = 0;
    fputs(text, stdout);
    SHOW("cont after detaching", pt(CONT, child, 0, 0));
    wait_sleeping(child);
    kill(child, SIGUSR1);
    length = read(pipe_fds[0], text, sizeof text - 1);
    text[length > 0 ? length : 0] = 0;
    fputs(text, stdout);
    wait_sleeping(child);
    SHOW("attach once more", pt(ATTACH, child, 0, 0));
    wait_for("stop once more", child, 0);
    SHOW("detach", pt(DETACH, child, 0, 0));
    kill(child, SIGKILL);
    wait_for("end", child, 0);
    close(pipe_fds[0]);

    pid_t root = fork_flushed();
    if (root == 0) {
        for (;;)
            pause();
    }
    pid_t other = fork_flushed();
    if (other == 0) {
        if (setuid(65534) != 0)
            _exit(100);
        errno = 0;
        long answer = pt(ATTACH, root, 0, 0);
        _exit(answer == -1 && errno == EPERM ? 1 : 2);
    }
    wait_for("another user's attach (1 is EPERM)", other, 0);
    kill(root, SIGKILL);
    wait_for("root's child", root, 0);
}

// A tracer of a process it did not make (its grandchild): waits by its number, its group, any,
// or a group it is not in; another thread of the tracer neither sees it with __WNOTHREAD nor may
// resume it; then the tracer ends with it stopped, and it goes on without the signal it stopped
// for (its handler would end it with 7; SIGUSR2's ends it with 8).
static pid_t grandchild;
static int report_fd;

static void end_eight(int signal) {
    (void)signal;
    _exit(8);
}

// Waits up to three seconds for a child of this process to end; prints how, or that none did.
static int wait_any(const char *label) {
    int status = 0;
    pid_t got = 0;
    for (int tries = 0; tries < 300 && got <= 0; tries++) {
        got = waitpid(-1, &status, WNOHANG);
        if (got <= 0)
            usleep(10000);
    }
    if (got > 0)
        describe(label, status);
    else
        printf("%s: still there\n", label);
    return got > 0;
}

static void *other_thread(void *unused) {
    (void)unused;
    siginfo_t info;
    memset(&info, 0, sizeof info);
    errno = 0;
    int answer = waitid(P_PID, grandchild, &info, WEXITED | WSTOPPED | WNOHANG | __WNOTHREAD);
    dprintf(report_fd, "other thread: own tracees only: %d %s\n", answer,
            answer == -1 ? name(errno) : "");
    memset(&info, 0, sizeof info);
    answer = waitid(P_PID, grandchild, &info, WEXITED | WSTOPPED | WNOWAIT);
    dprintf(report_fd, "other thread: kept: %d code %d status %d\n", answer, info.si_code,
            info.si_status);
    errno = 0;
    long resumed = pt(CONT, grandchild, 0, 0);
    dprintf(report_fd, "other thread: cont %ld %s\n", resumed, resumed == -1 ? name(errno) : "");
    return 0;
}

static void another_tracer(void) {
    printf("== another tracer\n");
    prctl(PR_SET_CHILD_SUBREAPER, 1);
    int pipe_fds[2], numbers[2];
    pipe(pipe_fds);
    pipe(numbers);
    pid_t tracer = fork_flushed();
    if (tracer == 0) {
        close(pipe_fds[0]);
        report_fd = pipe_fds[1];
        pid_t middle = fork();
        if (middle == 0) {
            pid_t traced = fork();
            if (traced == 0) {
                signal(SIGUSR1, count);
                signal(SIGUSR2, end_eight);
                pid_t self = getpid();
                write(numbers[1], &self, sizeof self);
                write(numbers[1], &self, sizeof self);
                for (;;) {
                    pause();
                    if (handled)
                        _exit(7);
                }
            }
            int status = 0;
            waitpid(traced, &status, 0);
            dprintf(report_fd, "middle: its child %s %d\n",
                    WIFEXITED(status) ? "exited" : "other",
                    WIFEXITED(status) ? WEXITSTATUS(status) : status);
            _exit(0);
        }
        read(numbers[0], &grandchild, sizeof grandchild);
        int status = 0;
        dprintf(report_fd, "tracer: attach %ld\n", pt(ATTACH, grandchild, 0, 0));
        pthread_t thread;
        pthread_create(&thread, 0, other_thread, 0);
        pthread_join(thread, 0);
        siginfo_t info;
        memset(&info, 0, sizeof info);
        int kept = waitid(P_PGID, 0, &info, WEXITED | WSTOPPED | WNOWAIT);
        dprintf(report_fd, "tracer: own group kept: %d is it %d code %d status %d\n", kept,
                info.si_pid == grandchild, info.si_code, info.si_status);
        pid_t got = waitpid(-getpgrp(), &status, 0);
        dprintf(report_fd, "tracer: by group: is it %d, stopped %d\n", got == grandchild,
                WIFSTOPPED(status) ? WSTOPSIG(status) : -1);
        errno = 0;
        got = waitpid(-1, &status, WNOHANG);
        dprintf(report_fd, "tracer: any: %d %s\n", got, got == -1 ? name(errno) : "");
        errno = 0;
        got = waitpid(-999999, &status, WNOHANG);
        dprintf(report_fd, "tracer: another group: %d %s\n", got,
                got == -1 ? name(errno) : "");
        dprintf(report_fd, "tracer: cont %ld\n", pt(CONT, grandchild, 0, 0));
        kill(grandchild, SIGUSR1);
        got = waitpid(grandchild, &status, 0);
        dprintf(report_fd, "tracer: by number: is it %d, stopped %d\n", got == grandchild,
                WIFSTOPPED(status) ? WSTOPSIG(status) : -1);
        _exit(0);
    }
    wait_for("tracer", tracer, 0);
    // The middle process comes to this one when the tracer ends; it ends once its child does.
    // Its child, if it went on without the signal it stopped for, ends at SIGUSR2.
    close(pipe_fds[1]);
    pid_t traced;
    read(numbers[0], &traced, sizeof traced);
    if (!wait_any("middle, once the tracer ended")) {
        kill(traced, SIGUSR2);
        if (!wait_any("middle, once its child was sent SIGUSR2")) {
            kill(traced, SIGKILL);
            wait_any("middle, once its child was killed");
        }
    }
    print_pipe(pipe_fds[0]);
    prctl(PR_SET_CHILD_SUBREAPER, 0);
}

// A tracer that asked not to hear of stops gets SIGCHLD only for the end.
static void no_stop_signal(void) {
    printf("== no stop signal\n");
    struct sigaction action = {0};
    action.sa_sigaction = on_child;
    action.sa_flags = SA_SIGINFO | SA_NOCLDSTOP | SA_RESTART;
    sigaction(SIGCHLD, &action, 0);
    chld_count = 0;
    pid_t child = fork_flushed();
    if (child == 0) {
        pt(TRACEME, 0, 0, 0);
        kill(getpid(), SIGUSR1);
        _exit(4);
    }
    wait_for("stop", child, 0);
    printf("SIGCHLD count after the stop: %d\n", chld_count);
    pt(CONT, child, 0, 0);
    wait_for("end", child, 0);
    printf("SIGCHLD count after the end: %d code %d\n", chld_count, chld_code);
    signal(SIGCHLD, SIG_DFL);
}

int main(void) {
    setvbuf(stdout, 0, _IOLBF, 0);
    stops_and_requests();
    blocked_and_changed();
    information_set();
    signal_while_stopped();
    untraced_ignored();
    ending();
    attaching();
    another_tracer();
    no_stop_signal();
    return 0;
}
