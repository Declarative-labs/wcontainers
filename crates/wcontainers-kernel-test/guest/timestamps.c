// SO_TIMESTAMP and SO_TIMESTAMPNS, old and new: the options read back as each is set, and the
// control message each kind of socket gives with what it receives, its time checked to fall
// between the send and the receive.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#ifndef SO_TIMESTAMP_NEW
#define SO_TIMESTAMP_NEW 63
#define SO_TIMESTAMPNS_NEW 64
#endif

static void options(const char *what, int s) {
    int names[] = {SO_TIMESTAMP, SO_TIMESTAMPNS, SO_TIMESTAMP_NEW, SO_TIMESTAMPNS_NEW};
    printf("%s:", what);
    for (int i = 0; i < 4; i++) {
        int value = -1;
        socklen_t length = sizeof value;
        getsockopt(s, SOL_SOCKET, names[i], &value, &length);
        printf(" %d", value);
    }
    printf("\n");
}

static long long now_ns(void) {
    struct timespec t;
    clock_gettime(CLOCK_REALTIME, &t);
    return t.tv_sec * 1000000000LL + t.tv_nsec;
}

static void receive(const char *what, int r, long long before) {
    char data[64], control[256];
    struct iovec iov = {data, sizeof data};
    struct msghdr m = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control,
                       .msg_controllen = sizeof control};
    long n = recvmsg(r, &m, MSG_DONTWAIT);
    long long after = now_ns();
    printf("%s: %ld controllen %zu\n", what, n, (size_t)m.msg_controllen);
    for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
        long long *t = (void *)CMSG_DATA(c);
        int ns = c->cmsg_type == SO_TIMESTAMPNS || c->cmsg_type == SO_TIMESTAMPNS_NEW;
        long long at = t[0] * 1000000000LL + t[1] * (ns ? 1 : 1000);
        // Microseconds lose up to one.
        int fits = at >= before - 1000 && at <= after;
        printf(" level %d type %d len %zu in range %d\n", c->cmsg_level, c->cmsg_type,
               (size_t)c->cmsg_len, fits);
    }
}

static void pair_test(const char *what, int s, int r, int option) {
    int one = 1;
    setsockopt(r, SOL_SOCKET, option, &one, sizeof one);
    long long before = now_ns();
    send(s, "x", 1, 0);
    usleep(1000);
    receive(what, r, before);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    options("fresh", u);
    int one = 1, zero = 0;
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMP, &one, sizeof one);
    options("timestamp", u);
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMPNS, &one, sizeof one);
    options("timestampns", u);
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMP_NEW, &one, sizeof one);
    options("timestamp new", u);
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMPNS_NEW, &one, sizeof one);
    options("timestampns new", u);
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMP, &zero, sizeof zero);
    options("timestamp off", u);
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMPNS, &one, sizeof one);
    options("timestampns again", u);
    setsockopt(u, SOL_SOCKET, SO_TIMESTAMPNS_NEW, &zero, sizeof zero);
    options("timestampns new off", u);

    int names[] = {SO_TIMESTAMP, SO_TIMESTAMPNS, SO_TIMESTAMP_NEW, SO_TIMESTAMPNS_NEW};
    for (int i = 0; i < 4; i++) {
        int r = socket(AF_INET, SOCK_DGRAM, 0);
        struct sockaddr_in a = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
        bind(r, (void *)&a, sizeof a);
        socklen_t length = sizeof a;
        getsockname(r, (void *)&a, &length);
        int s = socket(AF_INET, SOCK_DGRAM, 0);
        connect(s, (void *)&a, sizeof a);
        pair_test("udp", s, r, names[i]);
        close(r);
        close(s);
    }
    int d[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, d);
    pair_test("unix dgram", d[0], d[1], SO_TIMESTAMPNS);
    int st[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, st);
    pair_test("unix stream", st[0], st[1], SO_TIMESTAMP);
    int l = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(l, (void *)&a, sizeof a);
    socklen_t length = sizeof a;
    getsockname(l, (void *)&a, &length);
    listen(l, 1);
    int c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&a, sizeof a);
    int s = accept(l, 0, 0);
    pair_test("tcp", c, s, SO_TIMESTAMP);
    // Bytes sent before the option was set carry no time.
    int u2 = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in b = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(u2, (void *)&b, sizeof b);
    length = sizeof b;
    getsockname(u2, (void *)&b, &length);
    int sender = socket(AF_INET, SOCK_DGRAM, 0);
    sendto(sender, "y", 1, 0, (void *)&b, sizeof b);
    setsockopt(u2, SOL_SOCKET, SO_TIMESTAMP, &one, sizeof one);
    receive("sent before", u2, 0);
    return 0;
}
