// A process whose child has ended and is not reaped, waiting for ever: the kernel interface's
// round trips find the child's process ended.
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    pid_t child = fork();
    if (child == 0)
        _exit(0);
    siginfo_t ended;
    waitid(P_PID, child, &ended, WEXITED | WNOWAIT);
    for (;;)
        pause();
}
