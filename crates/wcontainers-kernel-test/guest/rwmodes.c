// Which files take reads and writes that may not wait (RWF_NOWAIT), and every fallocate mode up to
// 255 on a tmpfs file.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <signal.h>
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/inotify.h>
#include <sys/mman.h>
#include <sys/signalfd.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/timerfd.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <unistd.h>

static void nowait(const char *what, int fd) {
    char buf[256] = "0123456789";
    struct iovec iov = {buf, 10};
    long r = syscall(SYS_preadv2, fd, &iov, 1, 0L, 0L, (long)RWF_NOWAIT);
    printf("%s: preadv2 %ld errno %d", what, r, r < 0 ? errno : 0);
    r = syscall(SYS_preadv2, fd, &iov, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" current %ld errno %d", r, r < 0 ? errno : 0);
    r = syscall(SYS_pwritev2, fd, &iov, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" pwritev2 %ld errno %d", r, r < 0 ? errno : 0);
    // A buffer that cannot be reached shows whether the flag is refused before the copy.
    struct iovec bad = {(void *)1, 10};
    r = syscall(SYS_preadv2, fd, &bad, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" unreachable %ld errno %d", r, r < 0 ? errno : 0);
    r = syscall(SYS_pwritev2, fd, &bad, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" %ld errno %d\n", r, r < 0 ? errno : 0);
}

int main(void) {
    setvbuf(stdout, 0, _IOLBF, 0);
    mkdir("/dev/shm/w", 0777);
    int fd = open("/dev/shm/w/f", O_RDWR | O_CREAT, 0644);
    nowait("empty shm", fd);
    write(fd, "hello world", 11);
    nowait("written shm", fd);
    nowait("shm read only", open("/dev/shm/w/f", O_RDONLY));
    fd = open("/tmp/f", O_RDWR | O_CREAT, 0644);
    nowait("root tmp", fd);
    nowait("dev null", open("/dev/null", O_RDWR));
    nowait("dev zero", open("/dev/zero", O_RDWR));
    nowait("dev urandom", open("/dev/urandom", O_RDONLY));
    nowait("dev random", open("/dev/random", O_RDWR));
    nowait("dev full", open("/dev/full", O_RDWR));
    nowait("dev full nocty", open("/dev/full", O_RDONLY | O_NOCTTY));
    nowait("dev ptmx", open("/dev/ptmx", O_RDWR | O_NONBLOCK));
    int pair[2];
    socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, pair);
    write(pair[1], "y", 1);
    nowait("local stream", pair[0]);
    nowait("eventfd", eventfd(1, EFD_NONBLOCK));
    nowait("timerfd", timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK));
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGUSR1);
    nowait("signalfd", signalfd(-1, &set, SFD_NONBLOCK));
    nowait("epoll", epoll_create1(0));
    nowait("inotify", inotify_init1(IN_NONBLOCK));
    nowait("pidfd", (int)syscall(SYS_pidfd_open, getpid(), 0));
    nowait("proc dir", open("/proc/self", O_RDONLY | O_DIRECTORY));
    int p[2];
    pipe(p);
    write(p[1], "x", 1);
    nowait("pipe reader", p[0]);
    nowait("pipe writer", p[1]);
    mkfifo("/dev/shm/w/fifo", 0600);
    int named = open("/dev/shm/w/fifo", O_RDWR);
    write(named, "x", 1);
    nowait("named pipe", named);
    nowait("memfd", memfd_create("m", 0));
    nowait("proc comm", open("/proc/self/comm", O_RDWR));
    // Files that would wait answer EAGAIN instead, though not opened nonblocking.
    static char big[70000];
    struct iovec whole = {big, sizeof big};
    pipe(p);
    long r = syscall(SYS_pwritev2, p[1], &whole, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf("full pipe: %ld errno %d", r, r < 0 ? errno : 0);
    r = syscall(SYS_pwritev2, p[1], &whole, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" then %ld errno %d\n", r, r < 0 ? errno : 0);
    socketpair(AF_UNIX, SOCK_STREAM, 0, pair);
    nowait("empty local stream", pair[0]);
    r = syscall(SYS_pwritev2, pair[1], &whole, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf("local stream write: %ld errno %d\n", r, r < 0 ? errno : 0);
    nowait("empty eventfd", eventfd(0, 0));
    nowait("unarmed timerfd", timerfd_create(CLOCK_MONOTONIC, 0));
    struct iovec record = {big, 128};
    r = syscall(SYS_preadv2, signalfd(-1, &set, 0), &record, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf("empty signalfd: %ld errno %d\n", r, r < 0 ? errno : 0);
    fd = open("/dev/shm/w/g", O_RDWR | O_CREAT, 0644);
    for (int mode = 0; mode < 256; mode++) {
        long r = fallocate(fd, mode, 0, 4096);
        printf("fallocate %d: %ld errno %d\n", mode, r, r < 0 ? errno : 0);
    }
    return 0;
}
