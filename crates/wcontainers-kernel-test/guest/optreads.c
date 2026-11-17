// Socket options Linux answers on fresh, bound and connected sockets of each kind, some set and
// read back, and the corners of sends, polls, binds and batched receives around them.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <signal.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

// Newer than the C library's headers.
#define SO_NETNS_COOKIE 71
#define SO_BUF_LOCK 72
#define SO_TXREHASH 74

// Reads one option with `room` bytes of room and prints what came back.
static void show(const char *what, int fd, int level, int name, socklen_t room) {
    unsigned char v[256];
    memset(v, 0xa5, sizeof v);
    socklen_t l = room;
    if (getsockopt(fd, level, name, v, &l) < 0) {
        printf("%s %d %d: %s\n", what, level, name, strerror(errno));
        return;
    }
    printf("%s %d %d: len %u", what, level, name, l);
    for (unsigned i = 0; i < l && i < 40; i++)
        printf(" %02x", v[i]);
    printf("\n");
}

static void show_all(const char *what, int fd, int level, const int *names, int count) {
    for (int i = 0; i < count; i++)
        show(what, fd, level, names[i], 256);
}

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static int set_int(int fd, int level, int name, int value) {
    return setsockopt(fd, level, name, &value, sizeof value);
}

static const int SOCKET_NAMES[] = {1, 11, 14, 26, 34, 40, 41, 43, 44, 45, 47, 48, 56, 59, 69, 71, 72, 73, 74};
static const int TCP_NAMES[] = {17, 19, 20, 21, 26, 27, 28, 29, 31, 33, 37, 42, 43, 44, 45};
// IP_PROTOCOL (52) is the local port, which only a bound socket chooses.
static const int IP_NAMES[] = {5, 6, 7, 9, 18, 20, 21, 23, 25, 26, 50, 51, 64, 65, 66, 67, 80};
static const int IPV6_NAMES[] = {2,  3,  4,  5,  6,  8,  11, 17, 22, 24, 29, 30, 31, 33, 53, 54, 55,
                                 56, 57, 58, 59, 60, 61, 64, 65, 68, 69, 70, 72, 73, 74, 76, 77, 80};
#define COUNT(a) (int)(sizeof(a) / sizeof(a[0]))

static void tcp_pair(int family, int port, int *client, int *server) {
    int l = socket(family, SOCK_STREAM, 0);
    set_int(l, SOL_SOCKET, SO_REUSEADDR, 1);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    struct sockaddr_in6 a6 = {.sin6_family = AF_INET6, .sin6_port = htons(port), .sin6_addr = IN6ADDR_LOOPBACK_INIT};
    struct sockaddr *to = family == AF_INET ? (void *)&a : (void *)&a6;
    socklen_t length = family == AF_INET ? sizeof a : sizeof a6;
    bind(l, to, length);
    listen(l, 4);
    *client = socket(family, SOCK_STREAM, 0);
    connect(*client, to, length);
    *server = accept(l, NULL, NULL);
    close(l);
}

static void sweeps(void) {
    int t = socket(AF_INET, SOCK_STREAM, 0), u = socket(AF_INET, SOCK_DGRAM, 0);
    int t6 = socket(AF_INET6, SOCK_STREAM, 0), u6 = socket(AF_INET6, SOCK_DGRAM, 0);
    int x = socket(AF_UNIX, SOCK_STREAM, 0), xd = socket(AF_UNIX, SOCK_DGRAM, 0);
    show_all("tcp", t, SOL_SOCKET, SOCKET_NAMES, COUNT(SOCKET_NAMES));
    show_all("udp", u, SOL_SOCKET, SOCKET_NAMES, COUNT(SOCKET_NAMES));
    show_all("unix", x, SOL_SOCKET, SOCKET_NAMES, COUNT(SOCKET_NAMES));
    show_all("unixd", xd, SOL_SOCKET, SOCKET_NAMES, COUNT(SOCKET_NAMES));
    show_all("tcp", t, IPPROTO_TCP, TCP_NAMES, COUNT(TCP_NAMES));
    show("tcp window room 20", t, IPPROTO_TCP, TCP_REPAIR_WINDOW, 20);
    show("tcp zerocopy room 8", t, IPPROTO_TCP, TCP_ZEROCOPY_RECEIVE, 8);
    show_all("tcp", t, IPPROTO_IP, IP_NAMES, COUNT(IP_NAMES));
    show_all("udp", u, IPPROTO_IP, IP_NAMES, COUNT(IP_NAMES));
    show("tcp origdst room 16", t, IPPROTO_IP, 80, 16);
    show_all("tcp6", t6, IPPROTO_IPV6, IPV6_NAMES, COUNT(IPV6_NAMES));
    show_all("udp6", u6, IPPROTO_IPV6, IPV6_NAMES, COUNT(IPV6_NAMES));
    show("tcp6 origdst room 8", t6, IPPROTO_IPV6, 80, 8);
    // Rates read into an int, cookies of 8 bytes, and the namespace's cookie wanting exactly 8.
    show("pacing room 4", t, SOL_SOCKET, SO_MAX_PACING_RATE, 4);
    show("cookie room 4", t, SOL_SOCKET, SO_COOKIE, 4);
    unsigned long long c1 = 0, c2 = 0, n1 = 0, n2 = 0;
    socklen_t l = 8;
    getsockopt(t, SOL_SOCKET, SO_COOKIE, &c1, &l);
    getsockopt(u, SOL_SOCKET, SO_COOKIE, &c2, &l);
    unsigned long long again = 0;
    getsockopt(t, SOL_SOCKET, SO_COOKIE, &again, &l);
    printf("cookies nonzero %d different %d stable %d\n", c1 != 0 && c2 != 0, c1 != c2, again == c1);
    getsockopt(t, SOL_SOCKET, SO_NETNS_COOKIE, &n1, &l);
    getsockopt(xd, SOL_SOCKET, SO_NETNS_COOKIE, &n2, &l);
    printf("namespace cookies same %d\n", n1 == n2 && n1 != 0);
    // A bound UDP socket's IP_PROTOCOL is its port.
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7411), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(u, (void *)&a, sizeof a);
    show("udp bound", u, IPPROTO_IP, 52, 256);
    show("tcp fresh", t, IPPROTO_IP, 52, 256);
    // Connected TCP over both families.
    int c, s, c6, s6;
    tcp_pair(AF_INET, 7412, &c, &s);
    tcp_pair(AF_INET6, 7413, &c6, &s6);
    show_all("conn", c, IPPROTO_TCP, TCP_NAMES, COUNT(TCP_NAMES));
    show_all("conn", c, IPPROTO_IP, IP_NAMES, COUNT(IP_NAMES));
    show_all("conn6", c6, IPPROTO_IPV6, IPV6_NAMES, COUNT(IPV6_NAMES));
    show("conn peergroups", c, SOL_SOCKET, SO_PEERGROUPS, 256);
    result("conn queue seq", set_int(c, IPPROTO_TCP, TCP_QUEUE_SEQ, 1));
    result("fresh queue seq", set_int(t, IPPROTO_TCP, TCP_QUEUE_SEQ, 1));
}

static void sets(void) {
    int t = socket(AF_INET, SOCK_STREAM, 0), u = socket(AF_INET, SOCK_DGRAM, 0);
    int t6 = socket(AF_INET6, SOCK_STREAM, 0), x = socket(AF_UNIX, SOCK_STREAM, 0);
    result("debug on", set_int(t, SOL_SOCKET, SO_DEBUG, 1));
    result("debug off", set_int(t, SOL_SOCKET, SO_DEBUG, 0));
    result("busy poll preferred", set_int(t, SOL_SOCKET, SO_PREFER_BUSY_POLL, 1));
    result("no check", set_int(u, SOL_SOCKET, SO_NO_CHECK, 5));
    show("no check", u, SOL_SOCKET, SO_NO_CHECK, 256);
    result("bsd compat", set_int(u, SOL_SOCKET, SO_BSDCOMPAT, 1));
    show("bsd compat", u, SOL_SOCKET, SO_BSDCOMPAT, 256);
    result("detach none", set_int(u, SOL_SOCKET, SO_DETACH_FILTER, 0));
    result("lock filter", set_int(u, SOL_SOCKET, SO_LOCK_FILTER, 1));
    result("unlock filter", set_int(u, SOL_SOCKET, SO_LOCK_FILTER, 0));
    result("detach locked", set_int(u, SOL_SOCKET, SO_DETACH_FILTER, 0));
    show("buffer locks before", t, SOL_SOCKET, SO_BUF_LOCK, 256);
    set_int(t, SOL_SOCKET, SO_SNDBUF, 8192);
    show("buffer locks after sndbuf", t, SOL_SOCKET, SO_BUF_LOCK, 256);
    set_int(x, SOL_SOCKET, SO_RCVBUF, 8192);
    show("unix buffer locks after rcvbuf", x, SOL_SOCKET, SO_BUF_LOCK, 256);
    result("buffer locks 3", set_int(t, SOL_SOCKET, SO_BUF_LOCK, 3));
    show("buffer locks", t, SOL_SOCKET, SO_BUF_LOCK, 256);
    result("buffer locks 4", set_int(t, SOL_SOCKET, SO_BUF_LOCK, 4));
    result("txrehash 0", set_int(t, SOL_SOCKET, SO_TXREHASH, 0));
    show("txrehash", t, SOL_SOCKET, SO_TXREHASH, 256);
    result("txrehash -1", set_int(t, SOL_SOCKET, SO_TXREHASH, -1));
    show("txrehash", t, SOL_SOCKET, SO_TXREHASH, 256);
    result("txrehash 2", set_int(t, SOL_SOCKET, SO_TXREHASH, 2));
    result("udp txrehash", set_int(u, SOL_SOCKET, SO_TXREHASH, 0));
    result("passsec tcp", set_int(t, SOL_SOCKET, SO_PASSSEC, 1));
    result("passsec unix", set_int(x, SOL_SOCKET, SO_PASSSEC, 1));
    show("passsec unix", x, SOL_SOCKET, SO_PASSSEC, 256);
    result("repair", set_int(t, IPPROTO_TCP, TCP_REPAIR, 1));
    result("repair queue", set_int(t, IPPROTO_TCP, TCP_REPAIR_QUEUE, 1));
    result("timestamp", set_int(t, IPPROTO_TCP, TCP_TIMESTAMP, 1));
    result("thin dupack 1", set_int(t, IPPROTO_TCP, TCP_THIN_DUPACK, 1));
    result("thin dupack 2", set_int(t, IPPROTO_TCP, TCP_THIN_DUPACK, 2));
    show("thin dupack", t, IPPROTO_TCP, TCP_THIN_DUPACK, 256);
    result("rto min 1500", set_int(t, IPPROTO_TCP, 45, 1500));
    show("rto min", t, IPPROTO_TCP, 45, 256);
    result("rto min 1000", set_int(t, IPPROTO_TCP, 45, 1000));
    result("rto min 300000", set_int(t, IPPROTO_TCP, 45, 300000));
    result("rto max 5000", set_int(t, IPPROTO_TCP, 44, 5000));
    show("rto max", t, IPPROTO_TCP, 44, 256);
    result("rto max 500", set_int(t, IPPROTO_TCP, 44, 500));
    result("router alert", set_int(u, IPPROTO_IP, IP_ROUTER_ALERT, 1));
    result("recvopts", set_int(u, IPPROTO_IP, IP_RECVOPTS, 3));
    show("recvopts", u, IPPROTO_IP, IP_RECVOPTS, 256);
    result("v6 router alert", set_int(t6, IPPROTO_IPV6, IPV6_ROUTER_ALERT, 1));
    result("v6 recvhopopts", set_int(t6, IPPROTO_IPV6, IPV6_RECVHOPOPTS, 1));
    show("v6 recvhopopts", t6, IPPROTO_IPV6, IPV6_RECVHOPOPTS, 256);
    result("iptables set", set_int(u, IPPROTO_IP, 64, 1));
}

// The other end's groups: too little room gives ERANGE and the room wanted.
static void peer_groups(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    unsigned v[64];
    socklen_t l = 0;
    int r = getsockopt(sv[0], SOL_SOCKET, SO_PEERGROUPS, v, &l);
    printf("peergroups no room: %d %s wanted %u\n", r, r < 0 ? strerror(errno) : "", l);
    l = sizeof v;
    r = getsockopt(sv[0], SOL_SOCKET, SO_PEERGROUPS, v, &l);
    printf("peergroups: %d len %u", r, l);
    for (unsigned i = 0; i < l / 4; i++)
        printf(" %u", v[i]);
    printf("\n");
}

// Datagrams dropped for want of room are counted, and told with SO_RXQ_OVFL.
static void drops(void) {
    int r = socket(AF_INET, SOCK_DGRAM, 0), s = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7414), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(r, (void *)&a, sizeof a);
    set_int(r, SOL_SOCKET, SO_RCVBUF, 4096);
    set_int(r, SOL_SOCKET, SO_RXQ_OVFL, 1);
    char data[1000] = {0};
    for (int i = 0; i < 40; i++)
        sendto(s, data, sizeof data, 0, (void *)&a, sizeof a);
    sendto(s, data, 10, 0, (void *)&a, sizeof a);
    int got = 0;
    unsigned last = 0;
    for (;;) {
        char ctl[64];
        struct iovec io = {data, sizeof data};
        struct msghdr m = {.msg_iov = &io, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
        if (recvmsg(r, &m, MSG_DONTWAIT) < 0)
            break;
        got++;
        for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c))
            if (c->cmsg_level == SOL_SOCKET && c->cmsg_type == SO_RXQ_OVFL)
                memcpy(&last, CMSG_DATA(c), 4);
    }
    printf("received %d, drops told %u\n", got, last);
    // A datagram after the queue was read has the drops so far.
    sendto(s, data, 10, 0, (void *)&a, sizeof a);
    char ctl[64];
    struct iovec io = {data, sizeof data};
    struct msghdr m = {.msg_iov = &io, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
    recvmsg(r, &m, 0);
    struct cmsghdr *c = CMSG_FIRSTHDR(&m);
    printf("next: control %zu type %d\n", (size_t)m.msg_controllen, c ? c->cmsg_type : -1);
}

// SO_SELECT_ERR_QUEUE adds POLLPRI to an error.
static void error_priority(void) {
    int s = socket(AF_INET, SOCK_DGRAM, 0);
    set_int(s, IPPROTO_IP, IP_RECVERR, 1);
    set_int(s, SOL_SOCKET, SO_SELECT_ERR_QUEUE, 1);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7415), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    sendto(s, "x", 1, 0, (void *)&a, sizeof a);
    struct pollfd p = {.fd = s, .events = POLLIN};
    poll(&p, 1, 1000);
    printf("error poll %x\n", p.revents);
}

static void finding_corners(void) {
    // An unconnected TCP socket fails before its control messages are read.
    int t = socket(AF_INET6, SOCK_STREAM | SOCK_NONBLOCK, 0);
    char ctl[24] = {0};
    struct cmsghdr *c = (struct cmsghdr *)ctl;
    c->cmsg_len = 100;
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    char byte = 'x';
    struct iovec io = {&byte, 1};
    struct msghdr m = {.msg_iov = &io, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
    result("unconnected sendmsg bad control", sendmsg(t, &m, MSG_NOSIGNAL));
    // SO_PASSSEC is for local and netlink sockets.
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    show("passsec udp", u, SOL_SOCKET, SO_PASSSEC, 256);
    // poll wants SO_RCVLOWAT's bytes.
    int cl, sv;
    tcp_pair(AF_INET, 7416, &cl, &sv);
    set_int(sv, SOL_SOCKET, SO_RCVLOWAT, 10);
    write(cl, "abc", 3);
    struct pollfd p = {.fd = sv, .events = POLLIN};
    result("poll below lowat", poll(&p, 1, 100));
    write(cl, "defghijkl", 9);
    result("poll at lowat", poll(&p, 1, 100));
    // A second automatic bind keeps the first name.
    int x = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un un = {.sun_family = AF_UNIX};
    result("autobind", bind(x, (void *)&un, sizeof(sa_family_t)));
    result("bind again", bind(x, (void *)&un, sizeof(sa_family_t)));
    struct sockaddr_un named = {.sun_family = AF_UNIX, .sun_path = "\0optreads"};
    int y = socket(AF_UNIX, SOCK_STREAM, 0);
    bind(y, (void *)&named, offsetof(struct sockaddr_un, sun_path) + 9);
    result("automatic bind of a named socket", bind(y, (void *)&un, sizeof(sa_family_t)));
}

// A blocking recvmmsg waits for every message; sendmmsg's later messages wait for room too.
static void batches(void) {
    int r = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7417), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(r, (void *)&a, sizeof a);
    pid_t child = fork();
    if (child == 0) {
        int s = socket(AF_INET, SOCK_DGRAM, 0);
        sendto(s, "first", 5, 0, (void *)&a, sizeof a);
        struct timespec ts = {0, 200000000};
        nanosleep(&ts, NULL);
        sendto(s, "second", 6, 0, (void *)&a, sizeof a);
        _exit(0);
    }
    char b1[16], b2[16];
    struct iovec i1 = {b1, sizeof b1}, i2 = {b2, sizeof b2};
    struct mmsghdr v[2] = {{.msg_hdr = {.msg_iov = &i1, .msg_iovlen = 1}}, {.msg_hdr = {.msg_iov = &i2, .msg_iovlen = 1}}};
    int n = recvmmsg(r, v, 2, 0, NULL);
    printf("recvmmsg blocking: %d lengths %u %u\n", n, v[0].msg_len, v[1].msg_len);
    waitpid(child, NULL, 0);
    // A stream whose room runs out after the first message: the second waits for a reader.
    int c, s;
    tcp_pair(AF_INET, 7418, &c, &s);
    set_int(c, SOL_SOCKET, SO_SNDBUF, 4096);
    child = fork();
    if (child == 0) {
        struct timespec ts = {0, 200000000};
        nanosleep(&ts, NULL);
        char sink[65536];
        long total = 0;
        while (total < 2 * 60000) {
            long got = read(s, sink, sizeof sink);
            if (got <= 0)
                break;
            total += got;
        }
        _exit(0);
    }
    static char big[60000];
    struct iovec o1 = {big, sizeof big}, o2 = {big, sizeof big};
    struct mmsghdr w[2] = {{.msg_hdr = {.msg_iov = &o1, .msg_iovlen = 1}}, {.msg_hdr = {.msg_iov = &o2, .msg_iovlen = 1}}};
    n = sendmmsg(c, w, 2, 0);
    printf("sendmmsg blocking: %d lengths %u %u\n", n, w[0].msg_len, w[1].msg_len);
    waitpid(child, NULL, 0);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    signal(SIGPIPE, SIG_IGN);
    sweeps();
    sets();
    peer_groups();
    drops();
    error_priority();
    finding_corners();
    batches();
    return 0;
}
