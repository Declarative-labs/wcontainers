// SO_TIMESTAMPING: setting and reading the flags, and software receive timestamps on UDP, TCP
// and local sockets. Linux turns stamping on for the whole machine a moment after a socket asks
// (a deferred static key), so each receiver asks and waits before anything is sent to it.
#define _GNU_SOURCE
#include <errno.h>
#include <time.h>
#include <linux/errqueue.h>
#include <linux/net_tstamp.h>
#include <poll.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define TIMESTAMPING_OLD 37
#define TIMESTAMPING_NEW 65
#define TIMESTAMPNS_OLD 35
#ifndef SCM_TS_OPT_ID
#define SCM_TS_OPT_ID 81
#endif

static void set(const char *what, int fd, int name, unsigned flags, int phc, int len) {
    struct so_timestamping t = {.flags = flags, .bind_phc = phc};
    int r = setsockopt(fd, SOL_SOCKET, name, &t, len);
    unsigned char got[16];
    memset(got, 0xee, sizeof got);
    socklen_t length = sizeof got;
    int g = getsockopt(fd, SOL_SOCKET, name, got, &length);
    struct so_timestamping now;
    memcpy(&now, got, sizeof now);
    printf("%s: %d %s; read %d length %u flags %#x clock %d\n", what, r, r ? strerror(errno) : "",
           g, length, now.flags, now.bind_phc);
}

// Reads what a socket holds, saying of each time only whether it is now or empty.
static void drain_with(const char *who, int fd, int flags) {
    for (;;) {
        char buf[200], ctl[512];
        struct iovec v = {buf, sizeof buf};
        struct msghdr m = {.msg_iov = &v, .msg_iovlen = 1, .msg_control = ctl,
                           .msg_controllen = sizeof ctl};
        int n = recvmsg(fd, &m, MSG_DONTWAIT | flags);
        if (n < 0) {
            printf("  %s: %s\n", who, strerror(errno));
            return;
        }
        printf("  %s %d flags %#x:", who, n, m.msg_flags);
        struct timespec now;
        clock_gettime(CLOCK_REALTIME, &now);
        for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
            printf(" [%d/%d length %lu", c->cmsg_level, c->cmsg_type, (unsigned long)c->cmsg_len);
            if (c->cmsg_type == IP_RECVERR || c->cmsg_type == IPV6_RECVERR) {
                struct sock_extended_err e;
                memcpy(&e, CMSG_DATA(c), sizeof e);
                printf(" errno %u origin %u type %u code %u info %u data %u]", e.ee_errno,
                       e.ee_origin, e.ee_type, e.ee_code, e.ee_info, e.ee_data);
                continue;
            }
            long long t[6];
            int k = (c->cmsg_len - CMSG_LEN(0)) / 16;
            memcpy(t, CMSG_DATA(c), k * 16);
            for (int i = 0; i < k; i++) {
                const char *kind = t[2 * i] == 0 && t[2 * i + 1] == 0 ? "empty"
                                   : now.tv_sec - t[2 * i] < 10   ? "now"
                                                                  : "other";
                printf(" %s", kind);
            }
            printf("]");
        }
        printf("\n");
    }
}

static void drain(const char *who, int fd) { drain_with(who, fd, 0); }

static const unsigned RX = SOF_TIMESTAMPING_RX_SOFTWARE | SOF_TIMESTAMPING_SOFTWARE;

int main(void) {
    int u = socket(AF_INET, SOCK_DGRAM, 0), t = socket(AF_INET, SOCK_STREAM, 0);
    int x = socket(AF_UNIX, SOCK_DGRAM, 0);
    set("udp, an int", u, TIMESTAMPING_OLD, RX, 0, 4);
    set("udp, new", u, TIMESTAMPING_NEW, RX, 0, 4);
    set("udp, the structure", u, TIMESTAMPING_OLD, RX, 0, 8);
    set("udp, short", u, TIMESTAMPING_OLD, SOF_TIMESTAMPING_RX_SOFTWARE, 0, 2);
    set("udp, an unknown bit", u, TIMESTAMPING_OLD, 1 << 19, 0, 4);
    set("udp, stats without tsonly", u, TIMESTAMPING_OLD, SOF_TIMESTAMPING_OPT_STATS, 0, 4);
    set("udp, id_tcp without id", u, TIMESTAMPING_OLD, SOF_TIMESTAMPING_OPT_ID_TCP, 0, 4);
    set("udp, a bound clock", u, TIMESTAMPING_OLD, SOF_TIMESTAMPING_BIND_PHC, 0, 8);
    set("udp, hardware", u, TIMESTAMPING_OLD,
        SOF_TIMESTAMPING_TX_HARDWARE | SOF_TIMESTAMPING_RX_HARDWARE |
            SOF_TIMESTAMPING_RAW_HARDWARE,
        0, 4);
    set("udp, id", u, TIMESTAMPING_OLD, SOF_TIMESTAMPING_OPT_ID, 0, 4);
    set("tcp unconnected, id", t, TIMESTAMPING_OLD, SOF_TIMESTAMPING_OPT_ID, 0, 4);
    set("tcp, receive", t, TIMESTAMPING_OLD, SOF_TIMESTAMPING_RX_SOFTWARE, 0, 4);
    set("local, receive", x, TIMESTAMPING_OLD, RX, 0, 4);
    int ts = -1;
    socklen_t tl = sizeof ts;
    getsockopt(u, SOL_SOCKET, SO_TIMESTAMP, &ts, &tl);
    printf("udp so_timestamp %d\n", ts);

    // UDP receive timestamps.
    int r = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7600)};
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    bind(r, (void *)&a, sizeof a);
    set("receiver", r, TIMESTAMPING_OLD, RX, 0, 4);
    usleep(100000);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    drain("udp", r);
    set("receiver, new", r, TIMESTAMPING_NEW, RX, 0, 4);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    drain("udp", r);
    int on = 1;
    setsockopt(r, SOL_SOCKET, TIMESTAMPNS_OLD, &on, sizeof on);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    drain("udp with so_timestampns", r);
    set("receiver, receive only", r, TIMESTAMPING_OLD, SOF_TIMESTAMPING_RX_SOFTWARE, 0, 4);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    drain("udp", r);
    set("receiver, filtered", r, TIMESTAMPING_OLD, RX | SOF_TIMESTAMPING_OPT_RX_FILTER, 0, 4);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    drain("udp", r);

    // TCP: the time of the last buffer read.
    int l = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in ta = a;
    ta.sin_port = htons(7601);
    bind(l, (void *)&ta, sizeof ta);
    listen(l, 1);
    int c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&ta, sizeof ta);
    int s = accept(l, 0, 0);
    set("tcp accepted", s, TIMESTAMPING_OLD, RX, 0, 4);
    set("tcp connected, id", c, TIMESTAMPING_OLD, SOF_TIMESTAMPING_OPT_ID, 0, 4);
    usleep(100000);
    write(c, "hello", 5);
    usleep(20000);
    drain("tcp", s);

    // Local datagrams: only beside SO_TIMESTAMP.
    int pair[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, pair);
    set("local receiver", pair[1], TIMESTAMPING_OLD, RX, 0, 4);
    send(pair[0], "y", 1, 0);
    drain("local", pair[1]);
    setsockopt(pair[1], SOL_SOCKET, SO_TIMESTAMP, &on, sizeof on);
    send(pair[0], "y", 1, 0);
    drain("local with so_timestamp", pair[1]);
    int stream[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, stream);
    set("local stream receiver", stream[1], TIMESTAMPING_OLD, RX, 0, 4);
    setsockopt(stream[1], SOL_SOCKET, SO_TIMESTAMP, &on, sizeof on);
    send(stream[0], "z", 1, 0);
    drain("local stream", stream[1]);

    // Per-send flags: only the recording ones, and an id only with OPT_ID.
    char ctl[CMSG_SPACE(4)] = {0};
    struct iovec io = {"m", 1};
    struct msghdr m = {.msg_name = &a, .msg_namelen = sizeof a, .msg_iov = &io, .msg_iovlen = 1,
                       .msg_control = ctl, .msg_controllen = sizeof ctl};
    struct cmsghdr *h = CMSG_FIRSTHDR(&m);
    h->cmsg_level = SOL_SOCKET;
    h->cmsg_len = CMSG_LEN(4);
    unsigned values[] = {0, SOF_TIMESTAMPING_TX_HARDWARE, SOF_TIMESTAMPING_RX_SOFTWARE};
    for (int i = 0; i < 3; i++) {
        h->cmsg_type = TIMESTAMPING_OLD;
        memcpy(CMSG_DATA(h), &values[i], 4);
        int n = sendmsg(u, &m, 0);
        printf("send with flags %#x: %d %s\n", values[i], n, n < 0 ? strerror(errno) : "");
    }
    h->cmsg_type = SCM_TS_OPT_ID;
    int fresh = socket(AF_INET, SOCK_DGRAM, 0);
    int n = sendmsg(fresh, &m, 0);
    printf("send with an id, no OPT_ID: %d %s\n", n, n < 0 ? strerror(errno) : "");
    n = sendmsg(u, &m, 0);
    printf("send with an id: %d %s\n", n, n < 0 ? strerror(errno) : "");
    drain("udp", r);

    // Software send timestamps on UDP, only the times (OPT_TSONLY).
    int v = socket(AF_INET6, SOCK_DGRAM, 0);
    unsigned tx = SOF_TIMESTAMPING_TX_SOFTWARE | SOF_TIMESTAMPING_TX_SCHED |
                  SOF_TIMESTAMPING_SOFTWARE | SOF_TIMESTAMPING_OPT_ID | SOF_TIMESTAMPING_OPT_TSONLY;
    set("udp send stamps", u, TIMESTAMPING_OLD, tx, 0, 4);
    set("udp6 send stamps", v, TIMESTAMPING_OLD, tx & ~SOF_TIMESTAMPING_TX_SCHED, 0, 4);
    sendto(u, "abc", 3, 0, (void *)&a, sizeof a);
    sendto(u, "defg", 4, 0, (void *)&a, sizeof a);
    struct pollfd p = {.fd = u, .events = POLLIN};
    printf("poll %d revents %#x\n", poll(&p, 1, 0), p.revents);
    drain_with("udp errors", u, MSG_ERRQUEUE);
    drain("udp", r);
    struct sockaddr_in6 a6 = {.sin6_family = AF_INET6, .sin6_port = htons(7600)};
    inet_pton(AF_INET6, "::ffff:127.0.0.1", &a6.sin6_addr);
    sendto(v, "v6", 2, 0, (void *)&a6, sizeof a6);
    drain_with("udp6 errors", v, MSG_ERRQUEUE);
    drain("udp", r);
    h->cmsg_type = TIMESTAMPING_OLD;
    unsigned sched = SOF_TIMESTAMPING_TX_SCHED;
    memcpy(CMSG_DATA(h), &sched, 4);
    n = sendmsg(u, &m, 0);
    printf("send asking only sched: %d\n", n);
    drain_with("udp errors", u, MSG_ERRQUEUE);
    h->cmsg_type = SCM_TS_OPT_ID;
    unsigned key = 77;
    memcpy(CMSG_DATA(h), &key, 4);
    n = sendmsg(u, &m, 0);
    printf("send with key 77: %d\n", n);
    sendto(u, "next", 4, 0, (void *)&a, sizeof a);
    drain_with("udp errors", u, MSG_ERRQUEUE);
    drain("udp", r);
    set("udp send stamps, no OPT_ID", u, TIMESTAMPING_OLD,
        SOF_TIMESTAMPING_TX_SOFTWARE | SOF_TIMESTAMPING_OPT_TSONLY, 0, 4);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    drain_with("udp errors", u, MSG_ERRQUEUE);
    drain("udp", r);
    set("local send stamps", x, TIMESTAMPING_OLD, SOF_TIMESTAMPING_TX_SOFTWARE, 0, 4);
    return 0;
}
