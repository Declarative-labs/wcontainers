// Processes, pipes, waits and signals: fork, vfork, exec, exit statuses, wait4 and waitid with
// their options, orphans, process groups, pipes between processes, SIGPIPE, handlers with their
// frames, sigsuspend, interrupted and restarted calls, stop and continue, and SIGCHLD.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <ucontext.h>
#include <unistd.h>

static volatile sig_atomic_t caught;
static volatile int last_code, last_status, last_pid_is_child;
static pid_t expected_child;

static void on_signal(int signal, siginfo_t *info, void *context) {
    ucontext_t *uc = context;
    caught = signal;
    last_code = info->si_code;
    last_status = info->si_status;
    last_pid_is_child = info->si_pid == expected_child;
    (void)uc;
}

static void handle(int signal, int flags) {
    struct sigaction action = {0};
    action.sa_sigaction = on_signal;
    action.sa_flags = SA_SIGINFO | flags;
    sigaction(signal, &action, 0);
}

static void status(const char *what, int status) {
    if (WIFEXITED(status)) printf("%s: exited %d\n", what, WEXITSTATUS(status));
    else if (WIFSIGNALED(status)) printf("%s: killed by %d core %d\n", what, WTERMSIG(status), WCOREDUMP(status) != 0);
    else if (WIFSTOPPED(status)) printf("%s: stopped by %d\n", what, WSTOPSIG(status));
    else if (WIFCONTINUED(status)) printf("%s: continued\n", what);
    else printf("%s: status %x\n", what, status);
}

static void forks(void) {
    int value = 1;
    pid_t child = fork();
    if (child == 0) {
        value = 2;
        printf("child sees value %d, parent is mine %d\n", value, getppid() > 0);
        fflush(stdout);
        _exit(7);
    }
    int st;
    pid_t got = waitpid(child, &st, 0);
    printf("parent value %d, waited for child %d\n", value, got == child);
    status("fork", st);
    // Killed children, with and without core.
    child = fork();
    if (child == 0) { raise(SIGTERM); _exit(0); }
    waitpid(child, &st, 0);
    status("sigterm", st);
    child = fork();
    if (child == 0) { *(volatile int *)8 = 1; _exit(0); }
    waitpid(child, &st, 0);
    status("segv", st);
    child = fork();
    if (child == 0) { abort(); }
    waitpid(child, &st, 0);
    status("abort", st);
    // No children left.
    errno = 0;
    printf("wait with none: %d errno %d\n", wait(&st), errno);
    printf("waitpid of a stranger: %d errno %d\n", waitpid(99999, &st, 0), errno);
    // WNOHANG with a child still running.
    int ready[2];
    pipe(ready);
    child = fork();
    if (child == 0) { char c; close(ready[1]); read(ready[0], &c, 1); _exit(3); }
    close(ready[0]);
    printf("nohang: %d\n", waitpid(child, &st, WNOHANG));
    close(ready[1]);
    waitpid(child, &st, 0);
    status("after close", st);
    // waitid, with WNOWAIT first.
    child = fork();
    if (child == 0) _exit(9);
    siginfo_t info = {0};
    int r = waitid(P_PID, child, &info, WEXITED | WNOWAIT);
    printf("waitid nowait %d code %d status %d pid %d\n", r, info.si_code, info.si_status, info.si_pid == child);
    memset(&info, 0, sizeof info);
    r = waitid(P_ALL, 0, &info, WEXITED);
    printf("waitid %d code %d status %d pid %d\n", r, info.si_code, info.si_status, info.si_pid == child);
    printf("waitid bad options %d errno %d\n", waitid(P_ALL, 0, &info, 0), errno);
    // vfork: the parent waits until the child execs or exits.
    volatile int shared = 0;
    child = vfork();
    if (child == 0) { shared = 5; _exit(4); }
    waitpid(child, &st, 0);
    printf("vfork shared %d\n", shared);
    status("vfork", st);
    // Exec in a child.
    child = fork();
    if (child == 0) {
        char *argv[] = {"/bin/sh", "-c", "exit 12", 0};
        execv(argv[0], argv);
        _exit(127);
    }
    waitpid(child, &st, 0);
    status("exec sh", st);
    // Many children, reaped in any order, each status once.
    int total = 0;
    for (int i = 0; i < 5; i++) if (fork() == 0) _exit(i + 1);
    for (int i = 0; i < 5; i++) { wait(&st); total += WEXITSTATUS(st); }
    printf("five children total %d\n", total);
}

static void pipes(void) {
    int p[2];
    printf("pipe %d\n", pipe(p));
    printf("pipe size %d\n", fcntl(p[0], F_GETPIPE_SZ));
    printf("write %zd\n", write(p[1], "hello", 5));
    int n = 0;
    ioctl(p[0], FIONREAD, &n);
    printf("fionread %d\n", n);
    char buffer[100] = {0};
    printf("read %zd %s\n", read(p[0], buffer, sizeof buffer), buffer);
    fcntl(p[0], F_SETFL, O_NONBLOCK);
    printf("empty nonblocking read %zd errno %d\n", read(p[0], buffer, 1), errno);
    // Fill it.
    fcntl(p[1], F_SETFL, O_NONBLOCK);
    static char big[1 << 17];
    ssize_t total = 0, w;
    while ((w = write(p[1], big, 1000)) > 0) total += w;
    printf("filled %zd errno %d\n", total, errno);
    struct pollfd fds[2] = {{p[0], POLLIN | POLLOUT, 0}, {p[1], POLLIN | POLLOUT, 0}};
    printf("poll %d %x %x\n", poll(fds, 2, 0), fds[0].revents, fds[1].revents);
    printf("set size %d\n", fcntl(p[1], F_SETPIPE_SZ, 4096));
    printf("set size big %d errno %d\n", fcntl(p[1], F_SETPIPE_SZ, 1 << 21), errno);
    close(p[0]);
    close(p[1]);
    // A child writes more than the pipe holds; the parent reads it all.
    pipe(p);
    pid_t child = fork();
    if (child == 0) {
        close(p[0]);
        for (int i = 0; i < 300; i++) write(p[1], big, 1000);
        _exit(0);
    }
    close(p[1]);
    total = 0;
    while ((w = read(p[0], big, sizeof big)) > 0) total += w;
    printf("child wrote %zd, end %zd\n", total, w);
    int st;
    waitpid(child, &st, 0);
    close(p[0]);
    // Writing with no reader.
    pipe(p);
    close(p[0]);
    signal(SIGPIPE, SIG_IGN);
    printf("write to no reader %zd errno %d\n", write(p[1], "x", 1), errno);
    signal(SIGPIPE, SIG_DFL);
    child = fork();
    if (child == 0) { write(p[1], "x", 1); _exit(0); }
    waitpid(child, &st, 0);
    status("sigpipe", st);
    close(p[1]);
    // Packet mode keeps writes apart.
    pipe2(p, O_DIRECT);
    write(p[1], "abc", 3);
    write(p[1], "de", 2);
    printf("packet read %zd\n", read(p[0], buffer, 2));
    printf("packet read %zd\n", read(p[0], buffer, 10));
    close(p[0]);
    close(p[1]);
    printf("pipe2 bad flags %d errno %d\n", pipe2(p, O_APPEND), errno);
}

static void signals(void) {
    // A handler with information, and its return.
    handle(SIGUSR1, 0);
    raise(SIGUSR1);
    printf("caught %d code %d\n", caught, last_code);
    // SIGCHLD with its information.
    handle(SIGCHLD, 0);
    caught = 0;
    pid_t child = fork();
    if (child == 0) _exit(6);
    expected_child = child;
    sigset_t none;
    sigemptyset(&none);
    while (!caught) sigsuspend(&none);
    printf("sigchld %d code %d status %d from child %d\n", caught, last_code, last_status, last_pid_is_child);
    int st;
    waitpid(child, &st, 0);
    // Later children's ends race with what the parent prints, so they are not caught.
    signal(SIGCHLD, SIG_DFL);
    // A blocked signal stays pending until unblocked.
    sigset_t block;
    sigemptyset(&block);
    sigaddset(&block, SIGUSR2);
    sigprocmask(SIG_BLOCK, &block, 0);
    handle(SIGUSR2, 0);
    caught = 0;
    kill(getpid(), SIGUSR2);
    sigset_t pending;
    sigpending(&pending);
    printf("pending %d caught %d\n", sigismember(&pending, SIGUSR2), caught);
    sigprocmask(SIG_UNBLOCK, &block, 0);
    printf("after unblock caught %d\n", caught);
    // An interrupted sleep: EINTR without SA_RESTART, with the time left.
    handle(SIGALRM, 0);
    child = fork();
    if (child == 0) {
        struct timespec ten = {0, 50000000};
        nanosleep(&ten, 0);
        kill(getppid(), SIGALRM);
        _exit(0);
    }
    struct timespec long_sleep = {30, 0}, left = {0, 0};
    int r = nanosleep(&long_sleep, &left);
    printf("nanosleep %d errno %d left some %d\n", r, errno, left.tv_sec > 20);
    waitpid(child, &st, 0);
    // A read restarted with SA_RESTART, interrupted without.
    int p[2];
    pipe(p);
    handle(SIGALRM, SA_RESTART);
    child = fork();
    if (child == 0) {
        struct timespec ten = {0, 50000000};
        nanosleep(&ten, 0);
        kill(getppid(), SIGALRM);
        nanosleep(&ten, 0);
        write(p[1], "r", 1);
        _exit(0);
    }
    char c = 0;
    caught = 0;
    r = read(p[0], &c, 1);
    printf("restarted read %d %c caught %d\n", r, c, caught);
    waitpid(child, &st, 0);
    handle(SIGALRM, 0);
    child = fork();
    if (child == 0) {
        struct timespec ten = {0, 50000000};
        nanosleep(&ten, 0);
        kill(getppid(), SIGALRM);
        _exit(0);
    }
    r = read(p[0], &c, 1);
    printf("interrupted read %d errno %d\n", r, errno);
    waitpid(child, &st, 0);
    // Stop and continue, seen by the parent.
    child = fork();
    if (child == 0) { for (;;) pause(); }
    kill(child, SIGSTOP);
    waitpid(child, &st, WUNTRACED);
    status("stop", st);
    kill(child, SIGCONT);
    waitpid(child, &st, WCONTINUED);
    status("cont", st);
    kill(child, SIGKILL);
    waitpid(child, &st, 0);
    status("kill", st);
    // A fault caught by a handler that jumps over the faulting instruction.
    // Process groups.
    child = fork();
    if (child == 0) { setpgid(0, 0); for (;;) pause(); }
    setpgid(child, child);
    printf("group of child is its own %d\n", getpgid(child) == child);
    kill(-child, SIGTERM);
    waitpid(child, &st, 0);
    status("group kill", st);
    printf("kill missing group %d errno %d\n", kill(-99999, SIGTERM), errno);
    // sigtimedwait.
    sigprocmask(SIG_BLOCK, &block, 0);
    kill(getpid(), SIGUSR2);
    siginfo_t info;
    struct timespec zero = {0, 0};
    r = sigtimedwait(&block, &info, &zero);
    printf("sigtimedwait %d code %d\n", r, info.si_code);
    r = sigtimedwait(&block, &info, &zero);
    printf("sigtimedwait empty %d errno %d\n", r, errno);
    sigqueue(getpid(), SIGUSR2, (union sigval){.sival_int = 42});
    r = sigtimedwait(&block, &info, &zero);
    printf("sigqueue %d code %d value %d\n", r, info.si_code, info.si_value.sival_int);
}

static void orphans(void) {
    int p[2];
    pipe(p);
    pid_t child = fork();
    if (child == 0) {
        pid_t grandchild = fork();
        if (grandchild == 0) {
            close(p[1]);
            char c;
            read(p[0], &c, 1);
            printf("orphan's parent is the first process %d\n", getppid() == 1);
            fflush(stdout);
            _exit(0);
        }
        _exit(0);
    }
    close(p[0]);
    int st;
    waitpid(child, &st, 0);
    close(p[1]);
    // The first process reaps whatever comes to it.
    struct timespec t = {0, 100000000};
    nanosleep(&t, 0);
    while (waitpid(-1, &st, WNOHANG) > 0) {}
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    forks();
    pipes();
    signals();
    orphans();
    printf("done\n");
    return 0;
}
