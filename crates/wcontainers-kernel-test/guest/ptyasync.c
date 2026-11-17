// A pseudo-terminal master with FIOASYNC set by ioctl, with no owner named: a write to the slave
// sends SIGIO to the process that set it, which by default ends it.
#define _GNU_SOURCE
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile int signals;
static void counted(int signal) { signals++; }

static void run(int handled) {
    int master = open("/dev/ptmx", O_RDWR | O_NOCTTY | O_NONBLOCK);
    int unlock = 0;
    ioctl(master, TIOCSPTLCK, &unlock);
    int slave = open(ptsname(master), O_RDWR | O_NOCTTY | O_NONBLOCK);
    int on = 1;
    ioctl(master, FIOASYNC, &on);
    printf("owner %s\n", fcntl(master, F_GETOWN) == getpid() ? "the caller" : "another");
    fflush(stdout);
    char bytes[256];
    memset(bytes, 0, sizeof bytes);
    struct iovec part = {bytes, sizeof bytes};
    long written = writev(slave, &part, 1);
    // The signal comes from the terminal's work, which may run after the write returns: waited
    // for up to 5 s, which the signal ends (or the process, by default).
    for (int i = 0; i < 5000 && !signals; i++) usleep(1000);
    printf("written %ld, signals %d\n", written, signals);
    if (handled) return;
    exit(0);
}

int main(void) {
    signal(SIGIO, counted);
    run(1);
    pid_t child = fork();
    if (child == 0) {
        // As layer 1's runner: a session leader with no controlling terminal.
        setsid();
        signals = 0;
        signal(SIGIO, SIG_DFL);
        run(0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("child %s %d\n", WIFSIGNALED(status) ? "signal" : "exit",
           WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status));
    return 0;
}
