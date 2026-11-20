// A child a fatal signal is sent to ends when it next runs, not inside the sender's kill.
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

static void report(const char *name, pid_t child, int signal) {
    kill(child, signal);
    int status = 0;
    pid_t first = waitpid(child, &status, WNOHANG);
    waitpid(child, &status, 0);
    printf("%s: reaped at once %d, killed by %d\n", name, first == child,
           WIFSIGNALED(status) ? WTERMSIG(status) : 0);
}

static pid_t sleeper(void) {
    pid_t child = fork();
    if (child == 0) {
        sleep(3);
        _exit(0);
    }
    // Lets the child reach its sleep.
    usleep(100000);
    return child;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    report("sleeping, SIGTERM", sleeper(), SIGTERM);
    report("sleeping, SIGKILL", sleeper(), SIGKILL);

    pid_t stopped = sleeper();
    kill(stopped, SIGSTOP);
    int status;
    waitpid(stopped, &status, WUNTRACED);
    report("stopped, SIGKILL", stopped, SIGKILL);

    pid_t busy = fork();
    if (busy == 0)
        for (volatile unsigned long i = 0;; i++) {
        }
    report("running, SIGTERM", busy, SIGTERM);
    return 0;
}
