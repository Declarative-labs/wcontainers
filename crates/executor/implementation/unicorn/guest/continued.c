// When a parent hears that a stopped child was continued: a wait sees it at once, and one SIGCHLD
// with CLD_CONTINUED comes.

#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    sigset_t chld;
    sigemptyset(&chld);
    sigaddset(&chld, SIGCHLD);
    sigprocmask(SIG_BLOCK, &chld, NULL);
    pid_t c = fork();
    if (c == 0) {
        raise(SIGSTOP);
        for (;;)
            pause();
    }
    int status;
    pid_t r = waitpid(c, &status, WUNTRACED);
    printf("stopped %d\n", r == c && WIFSTOPPED(status));
    siginfo_t info;
    struct timespec none = {0, 0}, long_wait = {10, 0};
    r = sigtimedwait(&chld, &info, &none);
    printf("stop signal %d code %d status %d\n", r, info.si_code, info.si_status);
    kill(c, SIGCONT);
    // Whether SIGCHLD is already pending here races the child on the reference's other CPUs (seen
    // both ways), so only what follows is compared.
    r = waitpid(c, &status, WCONTINUED | WNOHANG);
    printf("continued %d\n", r == c && WIFCONTINUED(status));
    r = sigtimedwait(&chld, &info, &long_wait);
    printf("then signal %d code %d status %d\n", r, info.si_code, info.si_status);
    kill(c, SIGKILL);
    waitpid(c, &status, 0);
    printf("killed %d\n", WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL);
    return 0;
}
