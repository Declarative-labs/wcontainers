// Local sockets passed over themselves: cycles nothing outside reaches are collected once a local
// socket is released, and the files they carry are let go (a pipe's writer inside one ends the
// pipe). Linux collects on a work queue, at the release of any local socket on the machine, so
// only what holds once the program has released one is shown, waiting for it as long as it takes.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static long pass(int over, const int *fds, int count) {
    char space[CMSG_SPACE(sizeof(int) * 8)];
    memset(space, 0, sizeof space);
    struct iovec iov = {.iov_base = "x", .iov_len = 1};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = space,
                         .msg_controllen = CMSG_SPACE(sizeof(int) * count)};
    struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    c->cmsg_len = CMSG_LEN(sizeof(int) * count);
    memcpy(CMSG_DATA(c), fds, sizeof(int) * count);
    return sendmsg(over, &msg, MSG_DONTWAIT);
}

static int take(int from) {
    char space[CMSG_SPACE(sizeof(int) * 8)];
    char byte;
    struct iovec iov = {.iov_base = &byte, .iov_len = 1};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = space,
                         .msg_controllen = sizeof space};
    if (recvmsg(from, &msg, MSG_DONTWAIT) < 0)
        return -1;
    struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
    if (!c)
        return -1;
    int fd;
    memcpy(&fd, CMSG_DATA(c), sizeof fd);
    return fd;
}

static void pipe_now(const char *what, int reader) {
    char byte;
    long r = read(reader, &byte, 1);
    printf("%s: read %ld %s\n", what, r, r < 0 ? strerror(errno) : "");
}

static void pipe_end(const char *what, int reader) {
    char byte;
    long r = -1;
    for (int tries = 0; tries < 1000; tries++) {
        r = read(reader, &byte, 1);
        if (r >= 0 || errno != EAGAIN)
            break;
        usleep(10000);
    }
    printf("%s: read %ld %s\n", what, r, r < 0 ? strerror(errno) : "");
}

int main(void) {
    int kinds[] = {SOCK_STREAM, SOCK_DGRAM, SOCK_SEQPACKET};
    for (int k = 0; k < 3; k++) {
        printf("kind %d\n", kinds[k]);
        // A socket in its own queue with a pipe's writer.
        int sv[2], p[2];
        socketpair(AF_UNIX, kinds[k], 0, sv);
        pipe2(p, O_NONBLOCK);
        int fds[2] = {sv[1], p[1]};
        printf("pass %ld\n", pass(sv[0], fds, 2));
        close(p[1]);
        pipe_now("still held", p[0]);
        close(sv[1]);
        close(sv[0]);
        pipe_end("own queue", p[0]);
        close(p[0]);
        // Two pairs, each in the other's queue.
        int a[2], b[2];
        socketpair(AF_UNIX, kinds[k], 0, a);
        socketpair(AF_UNIX, kinds[k], 0, b);
        pipe2(p, O_NONBLOCK);
        int one[2] = {b[1], p[1]};
        pass(a[0], one, 2);
        pass(b[0], &a[1], 1);
        close(p[1]);
        close(a[1]);
        close(b[1]);
        close(a[0]);
        close(b[0]);
        pipe_end("crossed closed", p[0]);
        close(p[0]);
        // A cycle something outside still reaches stays: taking the socket back gives the pipe.
        socketpair(AF_UNIX, kinds[k], 0, sv);
        pipe2(p, O_NONBLOCK);
        int inner[2];
        socketpair(AF_UNIX, kinds[k], 0, inner);
        int carried[2] = {inner[1], p[1]};
        pass(inner[0], carried, 2);
        pass(sv[0], &inner[0], 1);
        close(inner[0]);
        close(inner[1]);
        close(p[1]);
        close(sv[0]);
        pipe_end("reached", p[0]);
        int back = take(sv[1]);
        printf("took back %d\n", back >= 0);
        int again = take(back);
        printf("took inner %d\n", again >= 0);
        int writer = take(again >= 0 ? back : -1);
        printf("inner second %d\n", writer >= 0);
        close(back);
        close(again);
        close(sv[1]);
        pipe_end("all closed", p[0]);
        close(p[0]);
    }
    // Too many descriptors in flight for the limit on open files.
    struct rlimit limit = {16, 16};
    setrlimit(RLIMIT_NOFILE, &limit);
    int sv[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    int fds[4] = {0, 1, 2, sv[0]};
    int sent = 0;
    long r;
    while ((r = pass(sv[0], fds, 4)) > 0 && sent < 100)
        sent++;
    printf("in flight: %d sends then %ld errno %d\n", sent, r, r < 0 ? errno : 0);
    // Taking some back makes room again.
    int got = take(sv[1]);
    printf("took %d\n", got >= 0);
    printf("after a take: %ld\n", pass(sv[0], fds, 4));
    printf("then: %ld errno %d\n", pass(sv[0], fds, 4), errno);
    return 0;
}
