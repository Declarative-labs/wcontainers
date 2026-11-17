// sendfile from and to descriptors with no file behind them: timer, signal and process descriptors and
// secret memory, with nothing and with a byte to move.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <sys/sendfile.h>
#include <sys/signalfd.h>
#include <sys/syscall.h>
#include <sys/timerfd.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

int main(void) {
    int t = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
    sigset_t s; sigemptyset(&s); sigaddset(&s, SIGUSR1);
    int sf = signalfd(-1, &s, SFD_NONBLOCK);
    int pf = syscall(SYS_pidfd_open, getpid(), 0);
    int sec = syscall(447, 0);
    int file = open("/tmp/out", O_RDWR | O_CREAT, 0600);
    int names[] = {t, sf, pf, sec};
    const char *what[] = {"timerfd", "signalfd", "pidfd", "secret"};
    for (int i = 0; i < 4; i++) {
        char b[64];
        snprintf(b, sizeof b, "%s to file 0", what[i]); p(b, sendfile(file, names[i], NULL, 0));
        snprintf(b, sizeof b, "%s to file 1", what[i]); p(b, sendfile(file, names[i], NULL, 1));
        snprintf(b, sizeof b, "%s to itself 0", what[i]); p(b, sendfile(names[i], names[i], NULL, 0));
        off_t o = 0;
        snprintf(b, sizeof b, "%s to file 0 offset", what[i]); p(b, sendfile(file, names[i], &o, 0));
        snprintf(b, sizeof b, "%s lseek", what[i]); p(b, lseek(names[i], 0, SEEK_CUR));
    }
    // Pipes and timer descriptors are on pseudo file systems, whose largest file is the 64-bit one.
    int zero = open("/dev/zero", O_RDONLY);
    int ends[2];
    pipe(ends);
    off_t far = 0x80000000LL;
    p("zero to pipe at 2 GiB", sendfile(ends[1], zero, &far, 1));
    far = 0x80000000LL;
    p("zero to timerfd at 2 GiB", sendfile(t, zero, &far, 1));
    p("file to timerfd 0", sendfile(t, file, NULL, 0));
    p("file to timerfd 1", sendfile(t, file, NULL, 1));
    return 0;
}
