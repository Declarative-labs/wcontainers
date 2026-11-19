/* Forks, execs itself and takes signals. Run with no argument it is the parent: it forks a child that execs
   this program with "child", which works for a while (so both run at once and turns end at their slices),
   sends the parent SIGUSR1 and exits 7; the parent spins until its handler has run, then waits. It forks a
   second child that waits in pause() and kills it with SIGTERM. Only the parent prints after each wait, and
   the child before it exits, so the output's order does not depend on the schedule. */

#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile sig_atomic_t taken;

static void on_signal(int signal) { taken = signal; }

static unsigned long work(unsigned long rounds) {
    unsigned long sum = 0;
    for (unsigned long i = 0; i < rounds; i++) sum = sum * 31 + i;
    return sum;
}

static int child(void) {
    printf("child: %lx\n", work(200000));
    fflush(stdout);
    kill(getppid(), SIGUSR1);
    return 7;
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "child") == 0) return child();
    struct sigaction action = {0};
    action.sa_handler = on_signal;
    sigaction(SIGUSR1, &action, 0);
    pid_t first = fork();
    if (first == 0) {
        char *arguments[] = {argv[0], "child", 0};
        execv(argv[0], arguments);
        _exit(127);
    }
    while (!taken) {
    }
    int status = 0;
    waitpid(first, &status, 0);
    printf("parent: signal %d taken, child exited %d\n", taken, WIFEXITED(status) ? WEXITSTATUS(status) : -1);
    pid_t second = fork();
    if (second == 0) {
        pause();
        _exit(0);
    }
    kill(second, SIGTERM);
    waitpid(second, &status, 0);
    printf("parent: second child killed by %d\n", WIFSIGNALED(status) ? WTERMSIG(status) : 0);
    return 3;
}
