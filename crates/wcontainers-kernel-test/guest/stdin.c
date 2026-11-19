// Standard input as `docker run -i` gives it.
//   all <chunk> <pause-every>: reads everything in reads of <chunk> bytes, sleeping a millisecond
//     every <pause-every> reads (0: never), then reads at the end again and polls it.
//   early <count>: reads <count> bytes, closes descriptor 0 and goes on while input still comes.
//   wait <read|poll|select|epoll>: blocks in that call for each line, until end of file.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/select.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static void say(const char *format, ...) __attribute__((format(printf, 1, 2)));

static void say(const char *format, ...) {
    char line[512];
    va_list arguments;
    va_start(arguments, format);
    int length = vsnprintf(line, sizeof line, format, arguments);
    va_end(arguments);
    if (write(1, line, length) != length) _exit(3);
}

static void pause_a_millisecond(void) {
    struct timespec pause = {0, 1000000};
    nanosleep(&pause, 0);
}

static int all(int chunk, int every) {
    struct stat status;
    fstat(0, &status);
    say("fifo %d mode %o size %d flags %o\n", S_ISFIFO(status.st_mode), status.st_mode & 0777,
        fcntl(0, F_GETPIPE_SZ), fcntl(0, F_GETFL));
    char *buffer = malloc(chunk);
    uint64_t hash = 14695981039346656037ull, total = 0, reads = 0;
    for (;;) {
        ssize_t count = read(0, buffer, chunk);
        if (count < 0) {
            say("read failed: %s\n", strerror(errno));
            return 1;
        }
        if (count == 0) break;
        for (ssize_t i = 0; i < count; i++) hash = (hash ^ (uint8_t)buffer[i]) * 1099511628211ull;
        total += count;
        if (every && ++reads % every == 0) pause_a_millisecond();
    }
    say("total %llu hash %016llx\n", (unsigned long long)total, (unsigned long long)hash);
    say("again %zd\n", read(0, buffer, chunk));
    struct pollfd watched = {0, POLLIN, 0};
    say("poll %d revents %x\n", poll(&watched, 1, 0), watched.revents);
    return 0;
}

static int early(int count) {
    char buffer[4096];
    int taken = 0;
    while (taken < count) {
        ssize_t got = read(0, buffer, count - taken < (int)sizeof buffer ? count - taken : (int)sizeof buffer);
        if (got <= 0) {
            say("read %zd\n", got);
            return 1;
        }
        taken += got;
    }
    say("read %d\n", taken);
    say("close %d\n", close(0));
    for (int i = 0; i < 100; i++) pause_a_millisecond();
    struct stat status;
    say("fstat %d %s\n", fstat(0, &status), strerror(errno));
    return 0;
}

// Reads one line, waiting in `how` before each read.
static int line(const char *how, int epoll, char *into, int room) {
    int length = 0;
    for (;;) {
        if (!strcmp(how, "poll")) {
            struct pollfd watched = {0, POLLIN, 0};
            int ready = poll(&watched, 1, -1);
            if (!length) say("poll %d revents %x\n", ready, watched.revents);
        } else if (!strcmp(how, "select")) {
            fd_set readable;
            FD_ZERO(&readable);
            FD_SET(0, &readable);
            int ready = select(1, &readable, 0, 0, 0);
            if (!length) say("select %d readable %d\n", ready, FD_ISSET(0, &readable));
        } else if (!strcmp(how, "epoll")) {
            struct epoll_event event;
            int ready = epoll_wait(epoll, &event, 1, -1);
            if (!length) say("epoll %d events %x\n", ready, event.events);
        }
        ssize_t got = read(0, into + length, 1);
        if (got <= 0) return length ? length : (int)got;
        length += got;
        if (into[length - 1] == '\n' || length == room) return length;
    }
}

static int wait_in(const char *how) {
    int epoll = epoll_create1(0);
    struct epoll_event event = {.events = EPOLLIN, .data.fd = 0};
    epoll_ctl(epoll, EPOLL_CTL_ADD, 0, &event);
    char buffer[256];
    for (int round = 1;; round++) {
        say("waiting %d\n", round);
        int length = line(how, epoll, buffer, sizeof buffer);
        if (length <= 0) {
            say("end %d\n", length);
            return 0;
        }
        say("got %d: %.*s", length, length, buffer);
    }
}

int main(int argc, char **argv) {
    if (argc >= 4 && !strcmp(argv[1], "all")) return all(atoi(argv[2]), atoi(argv[3]));
    if (argc >= 3 && !strcmp(argv[1], "early")) return early(atoi(argv[2]));
    if (argc >= 3 && !strcmp(argv[1], "wait")) return wait_in(argv[2]);
    return 2;
}
