// TCP_INFO through a connection's life: the fields Linux gives the same on every loopback run
// exactly, and the ones that depend on timing only as plausible or not.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define INFO_SIZE 280

static uint32_t u32_at(const unsigned char *b, int at) {
    uint32_t v;
    memcpy(&v, b + at, 4);
    return v;
}

static uint64_t u64_at(const unsigned char *b, int at) {
    uint64_t v;
    memcpy(&v, b + at, 8);
    return v;
}

static int get(int fd, unsigned char *b) {
    memset(b, 0xaa, INFO_SIZE + 16);
    socklen_t len = INFO_SIZE + 16;
    if (getsockopt(fd, IPPROTO_TCP, TCP_INFO, b, &len) < 0) {
        printf("  getsockopt: %s\n", strerror(errno));
        return -1;
    }
    printf("  length %u, past it %s\n", len, b[len] == 0xaa ? "untouched" : "written");
    return 0;
}

// What a socket that never connected, or a listener, shows.
static void unconnected(const char *what, int fd) {
    unsigned char b[INFO_SIZE + 16];
    printf("%s\n", what);
    if (get(fd, b))
        return;
    printf("  state %u ca %u options %u wscale %#x flags %#x\n", b[0], b[1], b[5], b[6], b[7]);
    printf("  rto %u ato %u snd_mss %u rcv_mss %u unacked %u sacked %u\n", u32_at(b, 8),
           u32_at(b, 12), u32_at(b, 16), u32_at(b, 20), u32_at(b, 24), u32_at(b, 28));
    printf("  last sent, received, acked %s\n",
           u32_at(b, 44) == u32_at(b, 52) && u32_at(b, 52) == u32_at(b, 56) ? "equal" : "differ");
    printf("  pmtu %u rcv_ssthresh %u rtt %u rttvar %u ssthresh %u cwnd %u advmss %u reordering %u "
           "rcv_space %u\n",
           u32_at(b, 60), u32_at(b, 64), u32_at(b, 68), u32_at(b, 72), u32_at(b, 76), u32_at(b, 80),
           u32_at(b, 84), u32_at(b, 88), u32_at(b, 96));
    printf("  pacing %#llx max %#llx segs out %u min_rtt %u snd_wnd %u rcv_wnd %u\n",
           (unsigned long long)u64_at(b, 104), (unsigned long long)u64_at(b, 112), u32_at(b, 136),
           u32_at(b, 148), u32_at(b, 228), u32_at(b, 232));
    int zero = 1;
    for (int i = 236; i < INFO_SIZE; i++)
        zero &= b[i] == 0;
    printf("  tail zero %d\n", zero);
}

// What a connection shows: exact fields, then the timing ones as plausible or not.
static void connected(const char *what, int fd) {
    unsigned char b[INFO_SIZE + 16];
    printf("%s\n", what);
    if (get(fd, b))
        return;
    printf("  state %u ca %u retransmits %u options %u wscale %#x\n", b[0], b[1], b[2], b[5],
           b[6]);
    printf("  unacked %u sacked %u lost %u retrans %u last_ack_sent %u\n", u32_at(b, 24),
           u32_at(b, 28), u32_at(b, 32), u32_at(b, 36), u32_at(b, 48));
    printf("  pmtu %u ssthresh %u cwnd %u advmss %u reordering %u total_retrans %u\n",
           u32_at(b, 60), u32_at(b, 76), u32_at(b, 80), u32_at(b, 84), u32_at(b, 88),
           u32_at(b, 100));
    printf("  bytes acked %llu received %llu sent %llu notsent %u max pacing %#llx\n",
           (unsigned long long)u64_at(b, 120), (unsigned long long)u64_at(b, 128),
           (unsigned long long)u64_at(b, 200), u32_at(b, 144), (unsigned long long)u64_at(b, 112));
    uint32_t rto = u32_at(b, 8), rtt = u32_at(b, 68), min_rtt = u32_at(b, 148);
    int timely = u32_at(b, 44) < 10000 && u32_at(b, 52) < 10000 && u32_at(b, 56) < 10000;
    printf("  rto plausible %d rtt plausible %d recent %d pacing set %d\n",
           rto >= 200000 && rto <= 1000000, rtt > 0 && rtt < 200000 && min_rtt <= rtt, timely,
           u64_at(b, 104) > 0 && u64_at(b, 104) != ~0ULL);
}

static int listener(int family, int port, int backlog) {
    int l = socket(family, SOCK_STREAM, 0);
    int on = 1;
    setsockopt(l, SOL_SOCKET, SO_REUSEADDR, &on, sizeof on);
    if (family == AF_INET) {
        struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port)};
        a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        bind(l, (void *)&a, sizeof a);
    } else {
        struct sockaddr_in6 a = {.sin6_family = AF_INET6, .sin6_port = htons(port)};
        a.sin6_addr = in6addr_loopback;
        bind(l, (void *)&a, sizeof a);
    }
    listen(l, backlog);
    return l;
}

static int connect_to(int family, int port, int flags) {
    int c = socket(family, SOCK_STREAM | flags, 0);
    int r;
    if (family == AF_INET) {
        struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port)};
        a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        r = connect(c, (void *)&a, sizeof a);
    } else {
        struct sockaddr_in6 a = {.sin6_family = AF_INET6, .sin6_port = htons(port)};
        a.sin6_addr = in6addr_loopback;
        r = connect(c, (void *)&a, sizeof a);
    }
    if (r < 0)
        printf("connect: %s\n", strerror(errno));
    return c;
}

static void maxseg(const char *what, int fd) {
    int mss = 0;
    socklen_t len = sizeof mss;
    getsockopt(fd, IPPROTO_TCP, TCP_MAXSEG, &mss, &len);
    printf("%s maxseg %d\n", what, mss);
}

// A connection of one family through its life, with small sends that go in one segment.
static void life(int family, int port) {
    char buf[2000];
    memset(buf, 'x', sizeof buf);
    int l = listener(family, port, 4);
    unconnected("listener", l);
    int c = connect_to(family, port, 0);
    unconnected("listener with one waiting", l);
    int s = accept(l, 0, 0);
    connected("connector", c);
    connected("accepted", s);
    maxseg("connector", c);
    maxseg("accepted", s);
    write(c, buf, 10);
    usleep(20000);
    connected("connector after sending 10", c);
    connected("accepted with 10 unread", s);
    read(s, buf, sizeof buf);
    write(s, buf, 1000);
    usleep(20000);
    read(c, buf, sizeof buf);
    connected("connector after a reply", c);
    connected("accepted after replying", s);
    maxseg("connector", c);
    // Long enough for a delayed acknowledgement of the FIN (40 ms at most here).
    shutdown(c, SHUT_WR);
    usleep(100000);
    connected("connector shut", c);
    connected("accepted, its peer shut", s);
    shutdown(s, SHUT_WR);
    usleep(100000);
    connected("connector, both shut", c);
    connected("accepted, both shut", s);
    close(c);
    close(s);
    close(l);
}

int main(void) {
    int fresh = socket(AF_INET, SOCK_STREAM, 0);
    unconnected("fresh", fresh);
    maxseg("fresh", fresh);
    int fresh6 = socket(AF_INET6, SOCK_STREAM, 0);
    unconnected("fresh ipv6", fresh6);

    life(AF_INET, 6101);
    life(AF_INET6, 6102);

    // Short and empty lengths, a datagram socket and a negative length.
    int l = listener(AF_INET, 6103, 1);
    int c = connect_to(AF_INET, 6103, 0);
    unsigned char small[8];
    socklen_t len = sizeof small;
    int r = getsockopt(c, IPPROTO_TCP, TCP_INFO, small, &len);
    printf("eight bytes: %d length %u state %u options %u\n", r, len, small[0], small[5]);
    len = 0;
    r = getsockopt(c, IPPROTO_TCP, TCP_INFO, small, &len);
    printf("no room: %d length %u\n", r, len);
    unsigned char big[INFO_SIZE + 16];
    int negative = -1;
    r = getsockopt(c, IPPROTO_TCP, TCP_INFO, big, (socklen_t *)&negative);
    printf("negative: %d %s length %d\n", r, r < 0 ? strerror(errno) : "", negative);
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    len = sizeof small;
    r = getsockopt(u, IPPROTO_TCP, TCP_INFO, small, &len);
    printf("udp: %d %s\n", r, r < 0 ? strerror(errno) : "");
    close(c);
    close(l);

    // A connect waiting for a full backlog's room has sent its SYN.
    int full = listener(AF_INET, 6104, 0);
    int first = connect_to(AF_INET, 6104, 0);
    int waiting = connect_to(AF_INET, 6104, SOCK_NONBLOCK);
    maxseg("waiting", waiting);
    unconnected("waiting for room", waiting);
    close(waiting);
    close(first);
    close(full);
    return 0;
}
