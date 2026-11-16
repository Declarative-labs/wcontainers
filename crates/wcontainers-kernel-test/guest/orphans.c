// Orphaned process groups and orphans: a group orphaned by its leader's end with a stopped member
// gets SIGHUP and SIGCONT, a group whose outside parent is the container's first process is not
// orphaned, and an orphan's exit signal becomes SIGCHLD.
#define _GNU_SOURCE
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static volatile int got;
static void on_signal(int signal) { got = got * 100 + signal; }

static volatile int chld, usr1;
static void on_chld(int signal) { (void)signal; chld++; }
static void on_usr1(int signal) { (void)signal; usr1++; }

// Reaps every child, waiting for those still running.
static void reap_all(void) {
    int status;
    while (waitpid(-1, &status, __WALL) > 0) {
    }
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int status;

    printf("a group orphaned by its leader's end\n");
    {
        // M leads a session; C leads group G in it; GC in G stops itself; C ends. GC's new parent,
        // the first process, is in another session, so G is orphaned with a stopped member.
        int ends[2];
        pipe(ends);
        pid_t m = fork();
        if (m == 0) {
            setsid();
            pid_t c = fork();
            if (c == 0) {
                setpgid(0, 0);
                pid_t gc = fork();
                if (gc == 0) {
                    close(ends[0]);
                    signal(SIGHUP, on_signal);
                    signal(SIGCONT, on_signal);
                    raise(SIGSTOP);
                    char line[64];
                    int n = snprintf(line, sizeof line, "  grandchild woke, got %d\n", got);
                    write(ends[1], line, n);
                    _exit(0);
                }
                waitpid(gc, &status, WUNTRACED);
                _exit(0);
            }
            waitpid(c, &status, 0);
            close(ends[1]);
            char byte;
            read(ends[0], &byte, 1);
            _exit(0);
        }
        close(ends[1]);
        char line[128];
        struct pollfd poller = {ends[0], POLLIN, 0};
        int ready = poll(&poller, 1, 3000);
        ssize_t n = ready > 0 ? read(ends[0], line, sizeof line - 1) : -1;
        if (n > 0) {
            line[n] = 0;
            printf("%s", line);
        } else {
            printf("  no word from the grandchild: poll %d\n", ready);
        }
        close(ends[0]);
        waitpid(m, &status, 0);
        reap_all();
    }

    printf("a group whose outside parent is the first process\n");
    {
        // C leads group G in the first process's session; GC in G; C ends. GC's new parent is the
        // first process, in another group of the same session, so G is not orphaned and SIGTSTP
        // stops GC.
        pid_t c = fork();
        if (c == 0) {
            setpgid(0, 0);
            pid_t gc = fork();
            if (gc == 0) {
                struct timespec t = {0, 50 * 1000 * 1000};
                nanosleep(&t, NULL);
                raise(SIGTSTP);
                _exit(5);
            }
            _exit(0);
        }
        waitpid(c, &status, 0);
        pid_t waited = waitpid(-1, &status, WUNTRACED);
        printf("  waited %s: stopped %d exited %d value %d\n", waited > 0 ? "a child" : "none",
               WIFSTOPPED(status), WIFEXITED(status),
               WIFEXITED(status) ? WEXITSTATUS(status) : WSTOPSIG(status));
        if (waited > 0 && WIFSTOPPED(status)) {
            kill(waited, SIGKILL);
            waitpid(waited, &status, 0);
        }
    }

    printf("an orphan's exit signal\n");
    {
        signal(SIGCHLD, on_chld);
        signal(SIGUSR1, on_usr1);
        int ends[2];
        pipe(ends);
        pid_t m = fork();
        if (m == 0) {
            // A child that tells its parent of its end by SIGUSR1, orphaned before it ends.
            pid_t c = syscall(SYS_clone, SIGUSR1, 0, 0, 0, 0);
            if (c == 0) {
                char byte;
                close(ends[1]);
                read(ends[0], &byte, 1);
                _exit(6);
            }
            _exit(0);
        }
        waitpid(m, &status, 0);
        int before_chld = chld, before_usr1 = usr1;
        close(ends[0]);
        close(ends[1]);
        pid_t waited;
        while ((waited = waitpid(-1, &status, __WALL)) < 0 && errno == EINTR) {
        }
        printf("  orphan ended %d; sigchld %d sigusr1 %d\n", WEXITSTATUS(status),
               chld - before_chld > 0, usr1 - before_usr1);
    }
    return 0;
}
