// RLIMIT_CPU: a soft limit sends SIGXCPU and is raised a second each time, the hard limit kills
// with SIGKILL, and a soft limit of zero. Linux looks at the limit at its timer tick, which may come
// at any moment, so nothing is printed between reaching a limit and its signal.

#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile sig_atomic_t taken;

static void on_xcpu(int signal) {
    (void)signal;
    taken++;
}

static void reap(const char *who, pid_t pid) {
    int status;
    waitpid(pid, &status, 0);
    if (WIFSIGNALED(status))
        printf("%s: killed by %d\n", who, WTERMSIG(status));
    else
        printf("%s: exited %d\n", who, WEXITSTATUS(status));
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    struct rlimit limit;
    getrlimit(RLIMIT_CPU, &limit);
    printf("start: soft infinite %d hard infinite %d\n", limit.rlim_cur == RLIM_INFINITY,
           limit.rlim_max == RLIM_INFINITY);

    // A soft limit of zero, with SIGXCPU's default action.
    pid_t child = fork();
    if (child == 0) {
        struct rlimit zero = {0, RLIM_INFINITY};
        setrlimit(RLIMIT_CPU, &zero);
        for (;;) {
        }
    }
    reap("zero", child);

    // SIGXCPU at one second, the soft limit raised to two, then SIGKILL at the hard limit.
    child = fork();
    if (child == 0) {
        struct sigaction action;
        memset(&action, 0, sizeof action);
        action.sa_handler = on_xcpu;
        sigaction(SIGXCPU, &action, NULL);
        struct rlimit one = {1, 2};
        printf("handled: set %d\n", setrlimit(RLIMIT_CPU, &one));
        int seen = 0;
        for (;;) {
            if (taken != seen) {
                seen = taken;
                getrlimit(RLIMIT_CPU, &limit);
                printf("handled: SIGXCPU %d, soft now %ld hard %ld\n", seen, (long)limit.rlim_cur,
                       (long)limit.rlim_max);
            }
        }
    }
    reap("handled", child);
    return 0;
}
