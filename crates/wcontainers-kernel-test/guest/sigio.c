// I/O signals: a pipe's reader told of writes and of the last writer's close, its writer of room, with
// SIGIO or a chosen signal carrying the band and descriptor; O_ASYNC given at open arms nothing; a
// terminal's input signals its side once a line (or, raw, any byte) is in, and a terminal with no
// owner takes the caller as owner when O_ASYNC is set.

#define _GNU_SOURCE
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <termios.h>
#include <unistd.h>

static volatile int count;
static volatile int last_signal, last_code, last_fd;
static volatile long last_band;

static void on_signal(int signal, siginfo_t *info, void *context) {
    (void)context;
    count++;
    last_signal = signal;
    last_code = info->si_code;
    last_band = info->si_band;
    last_fd = info->si_fd;
}

static void report(const char *what) {
    printf("%s: %d signals", what, count);
    if (count) {
        printf(", last %d code %d", last_signal == SIGIO ? 0 : last_signal - SIGRTMIN, last_code);
        if (last_signal != SIGIO)
            printf(" band %#lx fd %d", last_band, last_fd);
    }
    printf("\n");
    count = 0;
}

static void settle(int fd) {
    struct pollfd entry = {.fd = fd, .events = POLLIN};
    poll(&entry, 1, 0);
}

static void pipes(void) {
    int ends[2];
    pipe(ends);
    fcntl(ends[0], F_SETOWN, getpid());
    printf("set async: %d\n", fcntl(ends[0], F_SETFL, O_ASYNC | O_NONBLOCK));
    write(ends[1], "ab", 2);
    report("write");
    char buffer[8];
    read(ends[0], buffer, sizeof buffer);
    report("read with no async writer");
    read(ends[0], buffer, sizeof buffer);
    report("nonblocking read of nothing");
    fcntl(ends[1], F_SETOWN, getpid());
    fcntl(ends[1], F_SETSIG, SIGRTMIN + 1);
    fcntl(ends[1], F_SETFL, O_ASYNC);
    write(ends[1], "c", 1);
    report("write with an async writer");
    read(ends[0], buffer, sizeof buffer);
    report("read");
    fcntl(ends[0], F_SETSIG, SIGRTMIN + 2);
    int extra = dup(ends[0]);
    close(extra);
    report("closing a duplicate");
    close(ends[1]);
    report("last writer closed");
    close(ends[0]);

    // A chosen signal with codes of its own gets SI_SIGIO.
    pipe(ends);
    fcntl(ends[0], F_SETOWN, getpid());
    fcntl(ends[0], F_SETSIG, SIGCHLD);
    fcntl(ends[0], F_SETFL, O_ASYNC);
    write(ends[1], "d", 1);
    report("SIGCHLD chosen");
    close(ends[0]);
    close(ends[1]);

    // FASYNC given at open is shown but arms nothing.
    unlink("/tmp/sigio");
    mkfifo("/tmp/sigio", 0600);
    int both = open("/tmp/sigio", O_RDWR | O_ASYNC | O_NONBLOCK);
    fcntl(both, F_SETOWN, getpid());
    printf("async at open: %d\n", (fcntl(both, F_GETFL) & O_ASYNC) != 0);
    write(both, "e", 1);
    report("write to a fifo opened async");
    close(both);
    unlink("/tmp/sigio");
}

static void terminals(void) {
    int master = posix_openpt(O_RDWR | O_NOCTTY);
    grantpt(master);
    unlockpt(master);
    int slave = open(ptsname(master), O_RDWR | O_NOCTTY | O_NONBLOCK);
    fcntl(slave, F_SETOWN, getpid());
    fcntl(slave, F_SETFL, O_ASYNC | O_NONBLOCK);
    write(master, "ab", 2);
    settle(slave);
    report("part of a line");
    write(master, "c\n", 2);
    settle(slave);
    report("line end");
    char buffer[64];
    read(slave, buffer, sizeof buffer);
    // The echo waits in the master; it is taken so a later poll there takes in what comes next.
    fcntl(master, F_SETFL, O_NONBLOCK);
    settle(master);
    printf("echo: %ld\n", (long)read(master, buffer, sizeof buffer));
    struct termios settings;
    tcgetattr(slave, &settings);
    cfmakeraw(&settings);
    tcsetattr(slave, TCSANOW, &settings);
    write(master, "x", 1);
    settle(slave);
    report("raw byte");
    read(slave, buffer, sizeof buffer);
    // The master, with no owner, takes the caller when async is set.
    int on = 1;
    printf("FIOASYNC: %d\n", ioctl(master, FIOASYNC, &on));
    printf("owner is the caller: %d\n", fcntl(master, F_GETOWN) == getpid());
    write(slave, "y", 1);
    settle(master);
    report("output to the master");
    close(slave);
    close(master);
}

// A writer that found no room hears of it once the other side's reader has made some, once for each
// write that found none (n_tty_write_wakeup). How much fits is not shown: it differs (see the kernel
// status, departures).
static void terminal_room(void) {
    int master = posix_openpt(O_RDWR | O_NOCTTY | O_NONBLOCK);
    grantpt(master);
    unlockpt(master);
    int slave = open(ptsname(master), O_RDWR | O_NOCTTY | O_NONBLOCK);
    struct termios settings;
    tcgetattr(slave, &settings);
    cfmakeraw(&settings);
    tcsetattr(slave, TCSANOW, &settings);
    fcntl(master, F_SETOWN, getpid());
    fcntl(master, F_SETFL, O_ASYNC | O_NONBLOCK);
    static char block[1024];
    memset(block, 'z', sizeof block);
    for (int round = 0; round < 2; round++) {
        int full = 0;
        for (int tries = 0; tries < 100 && !full; tries++) {
            full = write(master, block, sizeof block) < (long)sizeof block;
            settle(slave);
        }
        report(full ? "filled" : "never filled");
        char buffer[4096];
        while (read(slave, buffer, sizeof buffer) > 0) {
            settle(slave);
        }
        report("emptied");
        write(master, block, 10);
        settle(slave);
        read(slave, buffer, sizeof buffer);
        report("a write that fitted, then a read");
    }
    close(slave);
    close(master);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    struct sigaction action = {0};
    action.sa_sigaction = on_signal;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGIO, &action, NULL);
    sigaction(SIGRTMIN + 1, &action, NULL);
    sigaction(SIGRTMIN + 2, &action, NULL);
    sigaction(SIGCHLD, &action, NULL);
    pipes();
    terminals();
    terminal_room();
    return 0;
}
