// Standard output as `docker run` takes it.
//   much <bytes> <block|nonblock>: writes <bytes> of a pattern in 8 KiB writes; nonblocking, it
//     waits in poll for space after each EAGAIN and says on standard error whether one came.
//   fill: writes without blocking until the pipe is full and says how much it took, and the
//     pipe's size, on standard error.
//   closed <client|self> <default|ignore>: says "ready" on standard error, waits, then writes to
//     standard output after its reader has gone: the client's, or a pipe it closed itself (the
//     reference's way, since Docker's daemon always reads).
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static void report(const char *text) {
    if (write(2, text, strlen(text)) < 0) _exit(3);
}

static int much(long bytes, int nonblocking) {
    static char chunk[8192];
    if (nonblocking) fcntl(1, F_SETFL, fcntl(1, F_GETFL) | O_NONBLOCK);
    long done = 0, again = 0;
    while (done < bytes) {
        long size = bytes - done < (long)sizeof chunk ? bytes - done : (long)sizeof chunk;
        for (long i = 0; i < size; i++) chunk[i] = 'a' + (done + i) % 26;
        ssize_t wrote = write(1, chunk, size);
        if (wrote < 0 && errno == EAGAIN) {
            again++;
            struct pollfd watched = {1, POLLOUT, 0};
            poll(&watched, 1, -1);
            continue;
        }
        if (wrote <= 0) {
            report("write failed\n");
            return 1;
        }
        done += wrote;
    }
    char line[128];
    snprintf(line, sizeof line, "wrote %ld, would block %s\n", done, again ? "yes" : "no");
    report(line);
    return 0;
}

static int fill(void) {
    static char chunk[4096];
    fcntl(1, F_SETFL, fcntl(1, F_GETFL) | O_NONBLOCK);
    long done = 0;
    ssize_t wrote;
    while ((wrote = write(1, chunk, sizeof chunk)) > 0) done += wrote;
    char line[128];
    snprintf(line, sizeof line, "pipe %d filled %ld %s\n", fcntl(1, F_GETPIPE_SZ), done,
             strerror(errno));
    report(line);
    return 0;
}

static int closed(int self, int ignore) {
    if (ignore) signal(SIGPIPE, SIG_IGN);
    if (self) {
        int ends[2];
        if (pipe(ends)) return 4;
        close(ends[0]);
        dup2(ends[1], 1);
        close(ends[1]);
    }
    report("ready\n");
    struct timespec pause = {0, 100000000};
    nanosleep(&pause, 0);
    ssize_t wrote = write(1, "x\n", 2);
    char line[128];
    snprintf(line, sizeof line, "write %zd %s\n", wrote, wrote < 0 ? strerror(errno) : "");
    report(line);
    return 0;
}

int main(int argc, char **argv) {
    if (argc >= 4 && !strcmp(argv[1], "much")) return much(atol(argv[2]), !strcmp(argv[3], "nonblock"));
    if (argc >= 2 && !strcmp(argv[1], "fill")) return fill();
    if (argc >= 4 && !strcmp(argv[1], "closed"))
        return closed(!strcmp(argv[2], "self"), !strcmp(argv[3], "ignore"));
    return 2;
}
