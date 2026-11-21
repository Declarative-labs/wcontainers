// Event descriptors and epoll: counters, semaphores and their limits; epoll over pipes, event,
// signal and timer descriptors and other instances, level and edge triggered, one-shot, closing a
// watched descriptor, the checks of epoll_ctl and epoll_wait, waits that time out or are
// interrupted, and poll and select of both kinds.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/ioctl.h>
#include <sys/select.h>
#include <sys/signalfd.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/timerfd.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static void show(const char *what, int ep, int timeout) {
    struct epoll_event events[8];
    memset(events, 0xab, sizeof events);
    int n = epoll_wait(ep, events, 8, timeout);
    printf("%s: %d", what, n);
    if (n < 0) printf(" %s", strerror(errno));
    for (int i = 0; i < n; i++) printf(" [%llu %#x]", (unsigned long long)events[i].data.u64, events[i].events);
    printf("\n");
}

static void add(int ep, int fd, uint32_t events, uint64_t data) {
    struct epoll_event event = {.events = events, .data.u64 = data};
    if (epoll_ctl(ep, EPOLL_CTL_ADD, fd, &event) < 0) printf("add %d: %s\n", fd, strerror(errno));
}

static void counters(void) {
    uint64_t value = 0;
    int fd = eventfd(3, EFD_NONBLOCK);
    result("read", read(fd, &value, 8));
    printf("value %llu\n", (unsigned long long)value);
    result("read empty", read(fd, &value, 8));
    result("read short", read(fd, &value, 4));
    value = 5;
    result("write", write(fd, &value, 8));
    result("write short", write(fd, &value, 7));
    value = UINT64_MAX;
    result("write max", write(fd, &value, 8));
    value = UINT64_MAX - 6;
    result("write over", write(fd, &value, 8));
    value = UINT64_MAX - 7;
    result("write up to", write(fd, &value, 8));
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT};
    result("poll full", poll(&p, 1, 0));
    printf("revents %#x\n", p.revents);
    result("lseek", lseek(fd, 0, SEEK_SET));
    close(fd);
    fd = eventfd(2, EFD_SEMAPHORE | EFD_NONBLOCK);
    for (int i = 0; i < 3; i++) {
        value = 0;
        result("semaphore read", read(fd, &value, 8));
        printf("value %llu\n", (unsigned long long)value);
    }
    close(fd);
    result("bad flags", eventfd(0, 4));
    struct stat st;
    fd = eventfd(0, 0);
    fstat(fd, &st);
    printf("mode %o size %lld\n", st.st_mode, (long long)st.st_size);
    char link[64] = {0}, path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    readlink(path, link, sizeof link - 1);
    printf("link %s, flags %#x\n", link, fcntl(fd, F_GETFL));
    // A reader waits for a writer in another process.
    if (fork() == 0) {
        usleep(10000);
        value = 9;
        write(fd, &value, 8);
        _exit(0);
    }
    value = 0;
    result("blocking read", read(fd, &value, 8));
    printf("value %llu\n", (unsigned long long)value);
    wait(0);
    close(fd);
}

static void triggers(void) {
    int p[2];
    pipe2(p, O_NONBLOCK);
    int level = epoll_create1(0), edge = epoll_create1(EPOLL_CLOEXEC);
    printf("cloexec %d %d\n", fcntl(level, F_GETFD), fcntl(edge, F_GETFD));
    add(level, p[0], EPOLLIN, 1);
    add(edge, p[0], EPOLLIN | EPOLLET, 2);
    add(level, p[1], EPOLLOUT, 3);
    show("level empty", level, 0);
    show("edge empty", edge, 0);
    write(p[1], "abc", 3);
    show("level after write", level, 0);
    show("level again", level, 0);
    show("edge after write", edge, 0);
    show("edge again", edge, 0);
    char buf[8];
    read(p[0], buf, 1);
    show("edge after partial read", edge, 0);
    write(p[1], "d", 1);
    show("edge after more", edge, 0);
    read(p[0], buf, 8);
    show("level drained", level, 0);
    // One-shot: reported once, then nothing until changed.
    int once = epoll_create1(0);
    add(once, p[0], EPOLLIN | EPOLLONESHOT, 4);
    write(p[1], "e", 1);
    show("oneshot", once, 0);
    show("oneshot again", once, 0);
    write(p[1], "f", 1);
    show("oneshot after write", once, 0);
    struct epoll_event event = {.events = EPOLLIN | EPOLLONESHOT, .data.u64 = 5};
    result("rearm", epoll_ctl(once, EPOLL_CTL_MOD, p[0], &event));
    show("rearmed", once, 0);
    // Closing the write end hangs the read end up.
    close(p[1]);
    show("level hung up", level, 0);
    // A duplicate keeps a closed descriptor's item.
    int copy = dup(p[0]);
    close(p[0]);
    show("closed but duplicated", level, 0);
    result("del closed", epoll_ctl(level, EPOLL_CTL_DEL, p[0], 0));
    close(copy);
    show("all closed", level, 0);
    close(level);
    close(edge);
    close(once);
}

static void checks(void) {
    int ep = epoll_create1(0), efd = eventfd(0, 0);
    struct epoll_event event = {.events = EPOLLIN};
    result("create bad flags", epoll_create1(1));
    result("ctl bad epfd", epoll_ctl(99, EPOLL_CTL_ADD, efd, &event));
    result("ctl bad fd", epoll_ctl(ep, EPOLL_CTL_ADD, 99, &event));
    result("ctl not epoll", epoll_ctl(efd, EPOLL_CTL_ADD, ep, &event));
    result("ctl self", epoll_ctl(ep, EPOLL_CTL_ADD, ep, &event));
    int file = open("/tmp/plain", O_CREAT | O_RDWR, 0600);
    result("ctl regular file", epoll_ctl(ep, EPOLL_CTL_ADD, file, &event));
    int null = open("/dev/null", O_RDWR);
    result("ctl /dev/null", epoll_ctl(ep, EPOLL_CTL_ADD, null, &event));
    int dir = open("/tmp", O_RDONLY);
    result("ctl directory", epoll_ctl(ep, EPOLL_CTL_ADD, dir, &event));
    int random = open("/dev/random", O_RDONLY);
    result("ctl /dev/random", epoll_ctl(ep, EPOLL_CTL_ADD, random, &event));
    result("ctl bad op", epoll_ctl(ep, 7, efd, &event));
    result("ctl mod missing", epoll_ctl(ep, EPOLL_CTL_MOD, efd, &event));
    result("ctl del missing", epoll_ctl(ep, EPOLL_CTL_DEL, efd, 0));
    result("ctl fault", epoll_ctl(ep, EPOLL_CTL_ADD, efd, (void *)8));
    result("ctl add", epoll_ctl(ep, EPOLL_CTL_ADD, efd, &event));
    result("ctl add again", epoll_ctl(ep, EPOLL_CTL_ADD, efd, &event));
    event.events = EPOLLIN | EPOLLEXCLUSIVE;
    result("ctl mod exclusive", epoll_ctl(ep, EPOLL_CTL_MOD, efd, &event));
    int other = eventfd(0, 0);
    result("ctl add exclusive", epoll_ctl(ep, EPOLL_CTL_ADD, other, &event));
    result("ctl mod of exclusive", epoll_ctl(ep, EPOLL_CTL_MOD, other, &event));
    event.events = EPOLLIN | EPOLLEXCLUSIVE | EPOLLPRI;
    int third = eventfd(0, 0);
    result("ctl exclusive bad bits", epoll_ctl(ep, EPOLL_CTL_ADD, third, &event));
    event.events = EPOLLIN | EPOLLWAKEUP;
    result("ctl wakeup", epoll_ctl(ep, EPOLL_CTL_ADD, third, &event));
    uint64_t one = 1;
    write(third, &one, 8);
    show("wakeup bit dropped", ep, 0);
    struct epoll_event events[2];
    result("wait zero", epoll_wait(ep, events, 0, 0));
    result("wait bad fd", epoll_wait(99, events, 1, 0));
    result("wait not epoll", epoll_wait(efd, events, 1, 0));
    result("wait fault", epoll_wait(ep, (void *)8, 1, 0));
    result("wait huge", epoll_wait(ep, events, 0x7fffffff, 0));
    // Nesting: an instance in another, a loop, and too deep.
    int outer = epoll_create1(0);
    event.events = EPOLLIN;
    result("nest", epoll_ctl(outer, EPOLL_CTL_ADD, ep, &event));
    result("loop", epoll_ctl(ep, EPOLL_CTL_ADD, outer, &event));
    event.events = EPOLLIN | EPOLLEXCLUSIVE;
    int spare = epoll_create1(0);
    result("exclusive nest", epoll_ctl(spare, EPOLL_CTL_ADD, ep, &event));
    event.events = EPOLLIN;
    int chain[7];
    chain[0] = epoll_create1(0);
    for (int i = 1; i < 7; i++) {
        chain[i] = epoll_create1(0);
        printf("depth %d: ", i);
        result("nest", epoll_ctl(chain[i], EPOLL_CTL_ADD, chain[i - 1], &event));
    }
    show("outer sees inner", outer, 0);
    struct stat st;
    fstat(ep, &st);
    char link[64] = {0}, path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", ep);
    readlink(path, link, sizeof link - 1);
    printf("link %s, mode %o, flags %#x\n", link, st.st_mode, fcntl(ep, F_GETFL));
    char byte;
    result("read epoll", read(ep, &byte, 1));
    result("write epoll", write(ep, &byte, 1));
}

static void sources(void) {
    int ep = epoll_create1(0);
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGUSR1);
    sigprocmask(SIG_BLOCK, &set, 0);
    int sfd = signalfd(-1, &set, SFD_NONBLOCK);
    add(ep, sfd, EPOLLIN, 10);
    show("signals none", ep, 0);
    kill(getpid(), SIGUSR1);
    show("signal pending", ep, 0);
    struct signalfd_siginfo info;
    read(sfd, &info, sizeof info);
    show("signal taken", ep, 0);
    int tfd = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
    add(ep, tfd, EPOLLIN | EPOLLET, 11);
    struct itimerspec spec = {.it_value = {0, 20000000}};
    timerfd_settime(tfd, 0, &spec, 0);
    show("timer waited", ep, 1000);
    show("timer edge again", ep, 0);
    int efd = eventfd(0, EFD_NONBLOCK);
    add(ep, efd, EPOLLIN | EPOLLOUT, 12);
    show("event writable", ep, 0);
    show("timeout", ep, 30);
    close(ep);
}

static int shared_ep, shared_efd;

static void *waker(void *arg) {
    usleep(20000);
    uint64_t one = 1;
    write(shared_efd, &one, 8);
    return 0;
}

static void on_usr2(int signal) {}

static void waits(void) {
    shared_ep = epoll_create1(0);
    shared_efd = eventfd(0, EFD_NONBLOCK);
    add(shared_ep, shared_efd, EPOLLIN, 20);
    pthread_t thread;
    pthread_create(&thread, 0, waker, 0);
    show("woken by another thread", shared_ep, -1);
    pthread_join(thread, 0);
    uint64_t value;
    read(shared_efd, &value, 8);
    // A handled signal ends a wait with EINTR whatever SA_RESTART says.
    struct sigaction action = {.sa_handler = on_usr2, .sa_flags = SA_RESTART};
    sigaction(SIGUSR2, &action, 0);
    if (fork() == 0) {
        usleep(20000);
        kill(getppid(), SIGUSR2);
        _exit(0);
    }
    show("interrupted", shared_ep, -1);
    wait(0);
    // epoll_pwait's mask is in place only while it waits.
    sigset_t block, during, after;
    sigemptyset(&block);
    sigaddset(&block, SIGUSR2);
    sigprocmask(SIG_BLOCK, &block, 0);
    sigemptyset(&during);
    struct epoll_event events[1];
    if (fork() == 0) {
        usleep(20000);
        kill(getppid(), SIGUSR2);
        _exit(0);
    }
    int n = epoll_pwait(shared_ep, events, 1, -1, &during);
    printf("pwait: %d %s\n", n, n < 0 ? strerror(errno) : "");
    wait(0);
    sigprocmask(SIG_SETMASK, 0, &after);
    printf("usr2 still blocked %d\n", sigismember(&after, SIGUSR2));
    struct timespec zero = {0, 0};
    n = syscall(SYS_epoll_pwait2, shared_ep, events, 1, &zero, 0, 8);
    printf("pwait2 zero: %d\n", n);
    struct timespec bad = {0, 2000000000};
    n = syscall(SYS_epoll_pwait2, shared_ep, events, 1, &bad, 0, 8);
    printf("pwait2 bad: %d %s\n", n, strerror(errno));
    n = syscall(SYS_epoll_pwait, shared_ep, events, 1, 0, &during, 4);
    printf("pwait bad size: %d %s\n", n, strerror(errno));
    // Poll and select see an instance readable while an item is ready.
    struct pollfd p = {.fd = shared_ep, .events = POLLIN};
    printf("poll idle %d\n", poll(&p, 1, 0));
    uint64_t one = 1;
    write(shared_efd, &one, 8);
    printf("poll ready %d %#x\n", poll(&p, 1, 0), p.revents);
    fd_set readable;
    FD_ZERO(&readable);
    FD_SET(shared_efd, &readable);
    struct timeval none = {0, 0};
    printf("select event %d\n", select(shared_efd + 1, &readable, 0, 0, &none));
}

// epoll's busy polling parameters and its refusal of other requests.
static void requests(void) {
    struct params {
        uint32_t usecs;
        uint16_t budget;
        uint8_t prefer, pad;
    } got;
    int ep = epoll_create1(0);
    memset(&got, 0xab, sizeof got);
    result("get", ioctl(ep, 0x80088a02, &got));
    printf("params %u %u %u %u\n", got.usecs, got.budget, got.prefer, got.pad);
    struct params set = {100, 8, 1, 0}, big = {1, 200, 0, 0}, prefer = {1, 8, 2, 0};
    struct params pad = {1, 8, 0, 1}, usecs = {0x80000000u, 8, 0, 0};
    result("set", ioctl(ep, 0x40088a01, &set));
    memset(&got, 0, sizeof got);
    result("get again", ioctl(ep, 0x80088a02, &got));
    printf("params %u %u %u %u\n", got.usecs, got.budget, got.prefer, got.pad);
    result("set big budget", ioctl(ep, 0x40088a01, &big));
    result("set prefer 2", ioctl(ep, 0x40088a01, &prefer));
    result("set pad", ioctl(ep, 0x40088a01, &pad));
    result("set usecs", ioctl(ep, 0x40088a01, &usecs));
    result("other", ioctl(ep, 0x5420, 0));
    result("bytes waiting", ioctl(ep, FIONREAD, &got));
    result("get to a bad address", ioctl(ep, 0x80088a02, (void *)8));
    close(ep);
}

// An event descriptor takes exactly one value per write, a vector a segment at a time.
static void sizes(void) {
    char z[32] = {0};
    int e = eventfd(0xffffffff, EFD_NONBLOCK | EFD_SEMAPHORE), f = eventfd(0, EFD_NONBLOCK);
    result("write 13", write(f, z, 13));
    result("write 8", write(f, z, 8));
    result("write 7", write(f, z, 7));
    result("semaphore write 13", write(e, z, 13));
    result("semaphore write 8", write(e, z, 8));
    struct iovec halves[2] = {{z, 4}, {z, 4}}, wide[1] = {{z, 16}}, two[2] = {{z, 8}, {z, 8}};
    result("writev 4+4", writev(f, halves, 2));
    result("writev 16", writev(f, wide, 1));
    result("writev 8+8", writev(f, two, 2));
    result("read 16", read(f, z, 16));
    result("read 7", read(f, z, 7));
    struct iovec parts[2] = {{z, 4}, {z + 4, 4}};
    result("readv 4+4", readv(f, parts, 2));
    write(f, z, 8);
    struct pollfd every = {.fd = f, .events = POLLIN | POLLOUT | POLLRDNORM | POLLWRNORM | POLLERR};
    poll(&every, 1, 0);
    printf("poll every bit %#x\n", every.revents);
    close(e);
    close(f);
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    sizes();
    requests();
    counters();
    triggers();
    checks();
    sources();
    waits();
    return 0;
}
