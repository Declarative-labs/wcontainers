// The limits that count a user's queued signals (RLIMIT_SIGPENDING) and tasks (RLIMIT_NPROC).

#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

static void queued_signals(void) {
    struct rlimit limit = {5, 5};
    setrlimit(RLIMIT_SIGPENDING, &limit);
    sigset_t all;
    sigfillset(&all);
    sigprocmask(SIG_BLOCK, &all, NULL);
    union sigval value = {.sival_int = 7};
    int queued = 0;
    for (int i = 0; i < 20; i++) {
        if (sigqueue(getpid(), SIGRTMIN + 2, value) != 0) {
            printf("sigqueue stopped after %d with errno %d\n", queued, errno);
            break;
        }
        queued++;
    }
    // Over the limit: kill's real-time signal and a queued standard one go without their information,
    // tkill's real-time one is refused.
    p("kill with a real-time signal", kill(getpid(), SIGRTMIN + 3));
    p("sigqueue of a standard signal", sigqueue(getpid(), SIGUSR1, value));
    p("tkill with a real-time signal", syscall(SYS_tkill, gettid(), SIGRTMIN + 4));
    // A child's SIGCHLD comes from the kernel and always fits.
    pid_t child = fork();
    if (child == 0) {
        _exit(0);
    }
    waitid(P_PID, child, &(siginfo_t){0}, WEXITED | WNOWAIT);
    struct timespec none = {0, 0};
    siginfo_t info;
    int signal;
    while ((signal = sigtimedwait(&all, &info, &none)) > 0) {
        printf("took %d code %d from ourselves %d value %d\n", signal, info.si_code, info.si_pid == getpid(),
               info.si_code == SI_QUEUE ? info.si_value.sival_int : -1);
    }
    waitpid(child, NULL, 0);
    // Taking them gave the places back.
    p("sigqueue once taken", sigqueue(getpid(), SIGRTMIN + 2, value));
}

static void tasks(void) {
    int ready[2], release[2];
    pipe(ready);
    pipe(release);
    // Another task of the user.
    pid_t other = fork();
    if (other == 0) {
        setuid(4321);
        write(ready[1], "x", 1);
        char byte;
        read(release[0], &byte, 1);
        _exit(0);
    }
    char byte;
    read(ready[0], &byte, 1);

    pid_t child = fork();
    if (child == 0) {
        struct rlimit limit = {1, 1};
        setrlimit(RLIMIT_NPROC, &limit);
        pid_t grandchild = fork();
        if (grandchild == 0) {
            _exit(0);
        }
        p("root's fork over the limit", grandchild > 0);
        waitpid(grandchild, NULL, 0);
        p("setuid", setuid(4321));
        p("fork over the limit", fork());
        _exit(0);
    }
    waitpid(child, NULL, 0);

    child = fork();
    if (child == 0) {
        struct rlimit limit = {1, 1};
        setrlimit(RLIMIT_NPROC, &limit);
        p("setuid with one other task", setuid(4321));
        char *argv[] = {"none", NULL};
        execve("/nonexistent", argv, argv + 1);
        printf("execve at the limit errno %d\n", errno);
        _exit(0);
    }
    waitpid(child, NULL, 0);

    child = fork();
    if (child == 0) {
        struct rlimit limit = {0, 0};
        setrlimit(RLIMIT_NPROC, &limit);
        p("setuid over the limit", setuid(4321));
        char *argv[] = {"none", NULL};
        execve("/nonexistent", argv, argv + 1);
        printf("execve over the limit errno %d\n", errno);
        execve("/nonexistent", argv, argv + 1);
        printf("execve again errno %d\n", errno);
        _exit(0);
    }
    waitpid(child, NULL, 0);

    // A task that has ended and is not yet reaped still counts.
    write(release[1], "x", 1);
    waitid(P_PID, other, &(siginfo_t){0}, WEXITED | WNOWAIT);
    child = fork();
    if (child == 0) {
        struct rlimit limit = {2, 2};
        setrlimit(RLIMIT_NPROC, &limit);
        setuid(4321);
        pid_t grandchild = fork();
        if (grandchild == 0) {
            _exit(0);
        }
        p("fork beside an ended task", grandchild);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    waitpid(other, NULL, 0);
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc > 1 && argv[1][0] == 's') {
        queued_signals();
    } else {
        tasks();
    }
    return 0;
}
