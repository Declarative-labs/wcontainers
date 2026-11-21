// TCP and UDP between processes in the container: creating sockets, binding and the port rules,
// listening, connecting and accepting, addresses, reading and writing, peeking, shutting down,
// resets, ports held after a close, options and readiness for poll and epoll.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static short revents(int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLPRI | POLLRDHUP};
    poll(&p, 1, 0);
    return p.revents;
}

static int opt(int fd, int level, int name) {
    int v = -7;
    socklen_t l = sizeof v;
    if (getsockopt(fd, level, name, &v, &l) < 0)
        return -1000 * errno;
    return v;
}

static struct sockaddr_in v4(const char *address, int port) {
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port)};
    inet_pton(AF_INET, address, &a.sin_addr);
    return a;
}

static struct sockaddr_in6 v6(const char *address, int port) {
    struct sockaddr_in6 a = {.sin6_family = AF_INET6, .sin6_port = htons(port)};
    inet_pton(AF_INET6, address, &a.sin6_addr);
    return a;
}

// Whether an ephemeral port's parity is shown: TCP chooses it, UDP does not.
static int parity = 1;

// Prints an address; an ephemeral port shows only as such, with its parity for TCP.
static void show(const char *what, struct sockaddr *a, socklen_t length) {
    char text[64] = "?";
    int port = 0;
    if (a->sa_family == AF_INET) {
        struct sockaddr_in *in = (void *)a;
        inet_ntop(AF_INET, &in->sin_addr, text, sizeof text);
        port = ntohs(in->sin_port);
    } else if (a->sa_family == AF_INET6) {
        struct sockaddr_in6 *in = (void *)a;
        inet_ntop(AF_INET6, &in->sin6_addr, text, sizeof text);
        port = ntohs(in->sin6_port);
    }
    if (port >= 32768 && port <= 60999)
        printf("%s: family %d %s ephemeral %s, length %d\n", what, a->sa_family, text,
               parity ? (port % 2 ? "odd" : "even") : "any", length);
    else
        printf("%s: family %d %s port %d, length %d\n", what, a->sa_family, text, port, length);
}

static void name(const char *what, int fd, int peer) {
    struct sockaddr_storage a;
    memset(&a, 0xee, sizeof a);
    socklen_t length = sizeof a;
    int r = peer ? getpeername(fd, (void *)&a, &length) : getsockname(fd, (void *)&a, &length);
    if (r < 0)
        result(what, r);
    else
        show(what, (void *)&a, length);
}

static void creating(void) {
    result("stream tcp", socket(AF_INET, SOCK_STREAM, IPPROTO_TCP) > 0);
    result("stream udp", socket(AF_INET, SOCK_STREAM, IPPROTO_UDP));
    result("dgram tcp", socket(AF_INET, SOCK_DGRAM, IPPROTO_TCP));
    result("protocol too large", socket(AF_INET, SOCK_STREAM, 300));
    result("kind too large", socket(AF_INET, 12, 0));
    result("rdm", socket(AF_INET, SOCK_RDM, 0));
    result("v6 stream udp", socket(AF_INET6, SOCK_STREAM, IPPROTO_UDP));
    int sv[2];
    result("pair", socketpair(AF_INET, SOCK_STREAM, 0, sv));
    result("pair v6 udp", socketpair(AF_INET6, SOCK_DGRAM, 0, sv));
}

static void options(void) {
    int t = socket(AF_INET, SOCK_STREAM, 0), u = socket(AF_INET, SOCK_DGRAM, 0);
    int t6 = socket(AF_INET6, SOCK_STREAM, 0);
    printf("tcp type %d protocol %d domain %d sndbuf %d rcvbuf %d\n", opt(t, SOL_SOCKET, SO_TYPE),
           opt(t, SOL_SOCKET, SO_PROTOCOL), opt(t, SOL_SOCKET, SO_DOMAIN),
           opt(t, SOL_SOCKET, SO_SNDBUF), opt(t, SOL_SOCKET, SO_RCVBUF));
    printf("udp type %d protocol %d sndbuf %d rcvbuf %d\n", opt(u, SOL_SOCKET, SO_TYPE),
           opt(u, SOL_SOCKET, SO_PROTOCOL), opt(u, SOL_SOCKET, SO_SNDBUF),
           opt(u, SOL_SOCKET, SO_RCVBUF));
    printf("tcp maxseg %d nodelay %d keepidle %d keepintvl %d keepcnt %d syncnt %d linger2 %d\n",
           opt(t, IPPROTO_TCP, TCP_MAXSEG), opt(t, IPPROTO_TCP, TCP_NODELAY),
           opt(t, IPPROTO_TCP, TCP_KEEPIDLE), opt(t, IPPROTO_TCP, TCP_KEEPINTVL),
           opt(t, IPPROTO_TCP, TCP_KEEPCNT), opt(t, IPPROTO_TCP, TCP_SYNCNT),
           opt(t, IPPROTO_TCP, TCP_LINGER2));
    printf("ip ttl %d tos %d mtu %d pmtu %d multicast ttl %d loop %d\n", opt(t, IPPROTO_IP, IP_TTL),
           opt(t, IPPROTO_IP, IP_TOS), opt(t, IPPROTO_IP, IP_MTU),
           opt(t, IPPROTO_IP, IP_MTU_DISCOVER), opt(u, IPPROTO_IP, IP_MULTICAST_TTL),
           opt(u, IPPROTO_IP, IP_MULTICAST_LOOP));
    printf("v6 only %d hops %d tclass %d; on v4 %d\n", opt(t6, IPPROTO_IPV6, IPV6_V6ONLY),
           opt(t6, IPPROTO_IPV6, IPV6_UNICAST_HOPS), opt(t6, IPPROTO_IPV6, IPV6_TCLASS),
           opt(t, IPPROTO_IPV6, IPV6_V6ONLY));
    printf("tcp option on udp %d, udp error %d, acceptconn %d\n", opt(u, IPPROTO_TCP, TCP_NODELAY),
           opt(u, SOL_SOCKET, SO_ERROR), opt(t, SOL_SOCKET, SO_ACCEPTCONN));
    char congestion[32] = {0};
    socklen_t length = sizeof congestion;
    getsockopt(t, IPPROTO_TCP, TCP_CONGESTION, congestion, &length);
    printf("congestion %s length %d\n", congestion, length);
    int one = 1, big = 100000, zero = 0, bad = 0x7fff0;
    result("set nodelay", setsockopt(t, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one));
    result("set nodelay short", setsockopt(t, IPPROTO_TCP, TCP_NODELAY, &one, 2));
    result("set keepidle 0", setsockopt(t, IPPROTO_TCP, TCP_KEEPIDLE, &zero, sizeof zero));
    result("set keepcnt big", setsockopt(t, IPPROTO_TCP, TCP_KEEPCNT, &big, sizeof big));
    result("set maxseg small", setsockopt(t, IPPROTO_TCP, TCP_MAXSEG, &one, sizeof one));
    result("set ttl 0", setsockopt(t, IPPROTO_IP, IP_TTL, &zero, sizeof zero));
    result("set ttl 9 one byte", setsockopt(t, IPPROTO_IP, IP_TTL, "\x09", 1));
    int tos = 0x2f;
    result("set tos", setsockopt(t, IPPROTO_IP, IP_TOS, &tos, sizeof tos));
    result("set udp tos", setsockopt(u, IPPROTO_IP, IP_TOS, &tos, sizeof tos));
    result("set sndbuf", setsockopt(t, SOL_SOCKET, SO_SNDBUF, &bad, sizeof bad));
    printf("now nodelay %d ttl %d tos %d udp tos %d sndbuf %d\n", opt(t, IPPROTO_TCP, TCP_NODELAY),
           opt(t, IPPROTO_IP, IP_TTL), opt(t, IPPROTO_IP, IP_TOS), opt(u, IPPROTO_IP, IP_TOS),
           opt(t, SOL_SOCKET, SO_SNDBUF));
    unsigned char small = 0xaa;
    length = 1;
    getsockopt(t, IPPROTO_IP, IP_TTL, &small, &length);
    printf("ttl in one byte %d length %d\n", small, length);
    result("set congestion", setsockopt(t, IPPROTO_TCP, TCP_CONGESTION, "nope", 4));
    result("set v6only on v4", setsockopt(t, IPPROTO_IPV6, IPV6_V6ONLY, &one, sizeof one));
    result("set unknown level", setsockopt(t, 999, 1, &one, sizeof one));
    result("get unknown level", opt(t, 999, 1));
    result("bind device", setsockopt(t, SOL_SOCKET, SO_BINDTODEVICE, "nope", 5));
    result("bind device lo", setsockopt(t, SOL_SOCKET, SO_BINDTODEVICE, "lo", 3));
    struct ucred cred;
    socklen_t credlength = sizeof cred;
    result("peercred", getsockopt(t, SOL_SOCKET, SO_PEERCRED, &cred, &credlength));
    printf("peercred %d %d %d\n", cred.pid, (int)cred.uid, (int)cred.gid);
    close(t);
    close(u);
    close(t6);
}

static void binding(void) {
    int s = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = v4("127.0.0.1", 5000);
    name("unbound", s, 0);
    name("unbound peer", s, 1);
    result("bind short", bind(s, (void *)&a, sizeof a - 1));
    struct sockaddr_in6 six = v6("::1", 5000);
    result("bind v6 address", bind(s, (void *)&six, sizeof six));
    result("bind elsewhere", bind(s, (void *)&(struct sockaddr_in){.sin_family = AF_INET,
                                                             .sin_addr.s_addr = htonl(0x08080808)},
                                  sizeof a));
    result("bind", bind(s, (void *)&a, sizeof a));
    name("bound", s, 0);
    result("bind again", bind(s, (void *)&a, sizeof a));
    int t = socket(AF_INET, SOCK_STREAM, 0);
    result("bind taken", bind(t, (void *)&a, sizeof a));
    struct sockaddr_in any = v4("0.0.0.0", 5000);
    result("bind any over taken", bind(t, (void *)&any, sizeof any));
    struct sockaddr_in other = v4("127.0.0.2", 5000);
    result("bind other loopback", bind(t, (void *)&other, sizeof other));
    close(t);
    // Both with SO_REUSEADDR share a port while neither listens.
    int one = 1;
    int r1 = socket(AF_INET, SOCK_STREAM, 0), r2 = socket(AF_INET, SOCK_STREAM, 0);
    setsockopt(r1, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    setsockopt(r2, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_in b = v4("127.0.0.1", 5001);
    result("reuse first", bind(r1, (void *)&b, sizeof b));
    result("reuse second", bind(r2, (void *)&b, sizeof b));
    result("reuse listen first", listen(r1, 4));
    result("reuse listen second", listen(r2, 4));
    close(r1);
    close(r2);
    // An unspecified family is taken for the any address only.
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in unspec = {.sin_family = AF_UNSPEC, .sin_port = htons(5002)};
    result("bind unspec any", bind(u, (void *)&unspec, sizeof unspec));
    name("bound unspec", u, 0);
    int u2 = socket(AF_INET, SOCK_DGRAM, 0);
    unspec.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    result("bind unspec loopback", bind(u2, (void *)&unspec, sizeof unspec));
    // IPv6 any covers IPv4 unless it is IPv6 only.
    int d6 = socket(AF_INET6, SOCK_DGRAM, 0);
    struct sockaddr_in6 any6 = v6("::", 5002);
    result("v6 any over v4 any", bind(d6, (void *)&any6, sizeof any6));
    int d6only = socket(AF_INET6, SOCK_DGRAM, 0);
    setsockopt(d6only, IPPROTO_IPV6, IPV6_V6ONLY, &one, sizeof one);
    result("v6 only any beside v4 any", bind(d6only, (void *)&any6, sizeof any6));
    name("v6 only bound", d6only, 0);
    result("v6only after bind", setsockopt(d6only, IPPROTO_IPV6, IPV6_V6ONLY, &one, sizeof one));
    struct sockaddr_in6 v4mapped = v6("::ffff:127.0.0.1", 5003);
    int m0 = socket(AF_INET6, SOCK_DGRAM, 0);
    result("v6 to mapped", bind(m0, (void *)&v4mapped, sizeof v4mapped));
    name("mapped bound", m0, 0);
    close(m0);
    int m = socket(AF_INET6, SOCK_DGRAM, 0);
    setsockopt(m, IPPROTO_IPV6, IPV6_V6ONLY, &one, sizeof one);
    result("v6 only to mapped", bind(m, (void *)&v4mapped, sizeof v4mapped));
    result("v6 short", bind(socket(AF_INET6, SOCK_DGRAM, 0), (void *)&any6, 23));
    close(s);
    close(u);
    close(u2);
    close(d6);
    close(d6only);
    close(m);
}

static void connecting(void) {
    signal(SIGPIPE, SIG_IGN);
    int c = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in nothing = v4("127.0.0.1", 5010);
    result("connect refused", connect(c, (void *)&nothing, sizeof nothing));
    name("after refused", c, 0);
    printf("after refused poll %#x error %d\n", revents(c), opt(c, SOL_SOCKET, SO_ERROR));
    int n = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0);
    result("nonblocking refused", connect(n, (void *)&nothing, sizeof nothing));
    printf("nonblocking refused poll %#x error %d then %d\n", revents(n),
           opt(n, SOL_SOCKET, SO_ERROR), opt(n, SOL_SOCKET, SO_ERROR));
    result("send unconnected", send(c, "x", 1, 0));
    char buf[256];
    result("recv unconnected", recv(c, buf, 1, 0));
    result("shutdown unconnected", shutdown(c, SHUT_WR));
    printf("poll after shutdown unconnected %#x\n", revents(c));
    close(c);
    close(n);
    int l = socket(AF_INET, SOCK_STREAM, 0);
    result("listen unbound", listen(l, 5));
    name("listener", l, 0);
    printf("listener poll %#x acceptconn %d\n", revents(l), opt(l, SOL_SOCKET, SO_ACCEPTCONN));
    result("connect listener", connect(l, (void *)&nothing, sizeof nothing));
    struct sockaddr_in at;
    socklen_t atlength = sizeof at;
    getsockname(l, (void *)&at, &atlength);
    at.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    c = socket(AF_INET, SOCK_STREAM, 0);
    result("connect", connect(c, (void *)&at, sizeof at));
    result("connect again", connect(c, (void *)&at, sizeof at));
    name("client", c, 0);
    name("client peer", c, 1);
    printf("listener poll with one %#x\n", revents(l));
    struct sockaddr_in from;
    socklen_t fromlength = 3;
    memset(&from, 0xee, sizeof from);
    int a = accept4(l, (void *)&from, &fromlength, SOCK_CLOEXEC);
    result("accept short room", a > 0);
    printf("short room length %d family %d\n", fromlength, from.sin_family);
    name("accepted", a, 0);
    name("accepted peer", a, 1);
    printf("accepted cloexec %d nonblock %d\n", fcntl(a, F_GETFD), fcntl(a, F_GETFL) & O_NONBLOCK);
    printf("client poll %#x accepted poll %#x\n", revents(c), revents(a));
    printf("connected sndbuf %d rcvbuf %d maxseg %d mtu %d\n", opt(c, SOL_SOCKET, SO_SNDBUF),
           opt(c, SOL_SOCKET, SO_RCVBUF), opt(c, IPPROTO_TCP, TCP_MAXSEG),
           opt(c, IPPROTO_IP, IP_MTU));
    // Bytes, peeking, waiting for all and throwing away.
    result("write", write(c, "hello world", 11));
    printf("accepted poll with data %#x\n", revents(a));
    result("peek", recv(a, buf, 5, MSG_PEEK));
    result("trunc", recv(a, buf, 6, MSG_TRUNC));
    result("read rest", read(a, buf, sizeof buf));
    printf("rest %.5s\n", buf);
    result("recvfrom", recvfrom(a, buf, sizeof buf, MSG_DONTWAIT, NULL, NULL));
    struct sockaddr_in sender;
    socklen_t senderlength = sizeof sender;
    write(c, "ab", 2);
    result("recvfrom with address", recvfrom(a, buf, sizeof buf, 0, (void *)&sender, &senderlength));
    printf("recvfrom address length %d\n", senderlength);
    result("sendto with address", sendto(c, "cd", 2, 0, (void *)&nothing, sizeof nothing));
    result("read it", read(a, buf, sizeof buf));
    if (fork() == 0) {
        usleep(20000);
        write(c, "abc", 3);
        usleep(20000);
        write(c, "defgh", 5);
        _exit(0);
    }
    result("waitall", recv(a, buf, 8, MSG_WAITALL));
    wait(NULL);
    // Ends: a FIN, then writes after the other end closed.
    result("shutdown write", shutdown(c, SHUT_WR));
    printf("after shutdown: client %#x accepted %#x\n", revents(c), revents(a));
    result("read end", read(a, buf, sizeof buf));
    result("write after shutdown", write(c, "x", 1));
    result("write back", write(a, "back", 4));
    result("read back", read(c, buf, sizeof buf));
    close(a);
    printf("after close: client %#x\n", revents(c));
    result("read after both", read(c, buf, sizeof buf));
    close(c);
    // A close with unread bytes resets.
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&at, sizeof at);
    a = accept(l, NULL, NULL);
    write(c, "unread", 6);
    close(a);
    printf("reset poll %#x\n", revents(c));
    result("read after reset", read(c, buf, sizeof buf));
    result("read again", read(c, buf, sizeof buf));
    result("write after reset", write(c, "x", 1));
    close(c);
    // Writes to an end that closed: the first goes, the reset ends it.
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&at, sizeof at);
    a = accept(l, NULL, NULL);
    close(a);
    printf("peer closed poll %#x\n", revents(c));
    result("write to closed", write(c, "one", 3));
    printf("then poll %#x\n", revents(c));
    result("write again", write(c, "two", 3));
    result("write third", write(c, "three", 5));
    result("read", read(c, buf, sizeof buf));
    close(c);
    // Closing a listener resets what waits in its backlog.
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&at, sizeof at);
    close(l);
    printf("listener gone poll %#x\n", revents(c));
    result("read after listener gone", read(c, buf, sizeof buf));
    close(c);
}

static void ports_held(void) {
    int one = 1;
    struct sockaddr_in a = v4("127.0.0.1", 5020);
    int l = socket(AF_INET, SOCK_STREAM, 0);
    result("bind server", bind(l, (void *)&a, sizeof a));
    listen(l, 1);
    int c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&a, sizeof a);
    int s = accept(l, NULL, NULL);
    // The server's end closes first and holds the port.
    close(s);
    char buf[8];
    read(c, buf, sizeof buf);
    close(c);
    close(l);
    l = socket(AF_INET, SOCK_STREAM, 0);
    result("bind while held", bind(l, (void *)&a, sizeof a));
    setsockopt(l, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    result("bind held with reuse", bind(l, (void *)&a, sizeof a));
    close(l);
    // The client's end closing first holds only its own port.
    struct sockaddr_in b = v4("127.0.0.1", 5021);
    l = socket(AF_INET, SOCK_STREAM, 0);
    bind(l, (void *)&b, sizeof b);
    listen(l, 1);
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (void *)&b, sizeof b);
    s = accept(l, NULL, NULL);
    close(c);
    read(s, buf, sizeof buf);
    close(s);
    close(l);
    l = socket(AF_INET, SOCK_STREAM, 0);
    result("bind after client closed first", bind(l, (void *)&b, sizeof b));
    close(l);
}

static void filling(void) {
    struct sockaddr_in a = v4("127.0.0.1", 5030);
    int l = socket(AF_INET, SOCK_STREAM, 0);
    bind(l, (void *)&a, sizeof a);
    listen(l, 1);
    int c = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0);
    result("nonblocking connect", connect(c, (void *)&a, sizeof a));
    struct pollfd p = {.fd = c, .events = POLLOUT};
    result("poll connected", poll(&p, 1, 5000));
    printf("connected error %d\n", opt(c, SOL_SOCKET, SO_ERROR));
    result("connect after", connect(c, (void *)&a, sizeof a));
    int s = accept(l, NULL, NULL);
    // No Nagle: each small write goes at once, whatever is unacknowledged.
    int one = 1;
    setsockopt(c, IPPROTO_TCP, TCP_NODELAY, &one, sizeof one);
    static char buf[1 << 16];
    long total = 0, n;
    while ((n = write(c, buf, sizeof buf)) > 0)
        total += n;
    printf("filled over a megabyte %d, %s, poll %#x\n", total > (1 << 20), strerror(errno),
           revents(c));
    long got = 0;
    while (got < total && (n = read(s, buf, sizeof buf)) > 0)
        got += n;
    printf("read all %d, poll %#x\n", got == total, revents(c));
    // epoll sees each change once with EPOLLET.
    int e = epoll_create1(0);
    struct epoll_event ev = {.events = EPOLLIN | EPOLLET, .data.fd = s};
    epoll_ctl(e, EPOLL_CTL_ADD, s, &ev);
    struct epoll_event got_events[4];
    result("epoll empty", epoll_wait(e, got_events, 4, 0));
    write(c, "x", 1);
    result("epoll data", epoll_wait(e, got_events, 4, 0));
    result("epoll again", epoll_wait(e, got_events, 4, 0));
    write(c, "y", 1);
    result("epoll more", epoll_wait(e, got_events, 4, 0));
    close(c);
    result("epoll end", epoll_wait(e, got_events, 4, 0));
    printf("end events %#x\n", got_events[0].events);
    close(e);
    close(s);
    close(l);
}

static void datagrams(void) {
    parity = 0;
    int r = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = v4("127.0.0.1", 5040);
    result("bind", bind(r, (void *)&a, sizeof a));
    int s = socket(AF_INET, SOCK_DGRAM, 0);
    result("send no destination", send(s, "x", 1, 0));
    result("sendto port 0", sendto(s, "x", 1, 0, (void *)&(struct sockaddr_in){.sin_family = AF_INET,
                                                                  .sin_addr.s_addr = htonl(INADDR_LOOPBACK)},
                                   sizeof a));
    result("sendto short", sendto(s, "x", 1, 0, (void *)&a, 8));
    result("sendto", sendto(s, "hello", 5, 0, (void *)&a, sizeof a));
    name("sender autobound", s, 0);
    result("sendto empty", sendto(s, "", 0, 0, (void *)&a, sizeof a));
    printf("receiver poll %#x\n", revents(r));
    char buf[64];
    struct sockaddr_in from;
    socklen_t length = sizeof from;
    result("recvfrom", recvfrom(r, buf, sizeof buf, 0, (void *)&from, &length));
    show("from", (void *)&from, length);
    result("recv empty", recv(r, buf, sizeof buf, 0));
    sendto(s, "truncated", 9, 0, (void *)&a, sizeof a);
    struct iovec v = {.iov_base = buf, .iov_len = 4};
    struct msghdr m = {.msg_name = &from, .msg_namelen = sizeof from, .msg_iov = &v, .msg_iovlen = 1};
    result("recvmsg short", recvmsg(r, &m, 0));
    printf("flags %#x namelen %d\n", m.msg_flags, m.msg_namelen);
    sendto(s, "whole", 5, 0, (void *)&a, sizeof a);
    result("recv trunc", recv(r, buf, 2, MSG_TRUNC));
    result("recv nothing", recv(r, buf, sizeof buf, MSG_DONTWAIT));
    static char big[70000];
    result("too large", sendto(s, big, 65508, 0, (void *)&a, sizeof a));
    result("largest", sendto(s, big, 65507, 0, (void *)&a, sizeof a));
    result("recv largest", recv(r, big, sizeof big, 0));
    // A connected socket takes only from its peer, and hears that a port is closed.
    int c = socket(AF_INET, SOCK_DGRAM, 0);
    result("connect", connect(c, (void *)&a, sizeof a));
    name("connected", c, 0);
    name("connected peer", c, 1);
    result("send connected", send(c, "via", 3, 0));
    result("recv via", recv(r, buf, sizeof buf, 0));
    struct sockaddr_in back;
    socklen_t backlength = sizeof back;
    getsockname(c, (void *)&back, &backlength);
    result("send to connected from elsewhere", sendto(s, "stranger", 8, 0, (void *)&back, sizeof back));
    result("send to connected from peer", sendto(r, "peer", 4, 0, (void *)&back, sizeof back));
    result("recv only peer", recv(c, buf, sizeof buf, MSG_DONTWAIT));
    result("recv none", recv(c, buf, sizeof buf, MSG_DONTWAIT));
    close(r);
    result("send to closed port", send(c, "x", 1, 0));
    printf("poll %#x\n", revents(c));
    result("send after refusal", send(c, "x", 1, 0));
    result("send next", send(c, "x", 1, 0));
    result("recv after refusal", recv(c, buf, sizeof buf, MSG_DONTWAIT));
    result("disconnect", connect(c, (void *)&(struct sockaddr){.sa_family = AF_UNSPEC}, sizeof(struct sockaddr)));
    name("disconnected", c, 0);
    name("disconnected peer", c, 1);
    result("shutdown", shutdown(c, SHUT_RDWR));
    printf("after shutdown poll %#x\n", revents(c));
    result("send after shutdown", send(c, "x", 1, 0));
    result("listen udp", listen(c, 1));
    result("accept udp", accept(c, NULL, NULL));
    // An IPv6 socket on the any address takes IPv4 senders, seen at mapped addresses.
    int six = socket(AF_INET6, SOCK_DGRAM, 0);
    struct sockaddr_in6 any6 = v6("::", 5041);
    result("bind v6", bind(six, (void *)&any6, sizeof any6));
    struct sockaddr_in to6 = v4("127.0.0.1", 5041);
    result("v4 to v6", sendto(s, "four", 4, 0, (void *)&to6, sizeof to6));
    struct sockaddr_in6 from6;
    socklen_t length6 = sizeof from6;
    result("recv on v6", recvfrom(six, buf, sizeof buf, 0, (void *)&from6, &length6));
    show("v6 from", (void *)&from6, length6);
    struct sockaddr_in6 loop6 = v6("::1", 5041);
    int s6 = socket(AF_INET6, SOCK_DGRAM, 0);
    result("v6 to v6", sendto(s6, "six", 3, 0, (void *)&loop6, sizeof loop6));
    length6 = sizeof from6;
    result("recv v6", recvfrom(six, buf, sizeof buf, 0, (void *)&from6, &length6));
    show("v6 from v6", (void *)&from6, length6);
    result("v6 too large", sendto(s6, big, 65528, 0, (void *)&loop6, sizeof loop6));
    close(s);
    close(c);
    close(six);
    close(s6);
    parity = 1;
}

static void stream_v6(void) {
    int l = socket(AF_INET6, SOCK_STREAM, 0);
    struct sockaddr_in6 any = v6("::", 5050);
    result("bind v6 stream", bind(l, (void *)&any, sizeof any));
    listen(l, 2);
    int c4 = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in to = v4("127.0.0.1", 5050);
    result("v4 to v6 listener", connect(c4, (void *)&to, sizeof to));
    int a = accept(l, NULL, NULL);
    name("v6 accepted", a, 0);
    name("v6 accepted peer", a, 1);
    int c6 = socket(AF_INET6, SOCK_STREAM, 0);
    struct sockaddr_in6 to6 = v6("::1", 5050);
    result("v6 to v6 listener", connect(c6, (void *)&to6, sizeof to6));
    name("v6 client", c6, 0);
    struct sockaddr_in6 far = v6("2001:db8::1", 80);
    int f = socket(AF_INET6, SOCK_STREAM, 0);
    result("v6 elsewhere", connect(f, (void *)&far, sizeof far));
    struct sockaddr_in6 any_to = v6("::", 5050);
    int z = socket(AF_INET6, SOCK_STREAM, 0);
    result("v6 to any", connect(z, (void *)&any_to, sizeof any_to));
    name("v6 to any peer", z, 1);
    struct sockaddr_in zero4 = v4("0.0.0.0", 5050);
    int z4 = socket(AF_INET, SOCK_STREAM, 0);
    result("v4 to any", connect(z4, (void *)&zero4, sizeof zero4));
    name("v4 to any peer", z4, 1);
    int b = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in bcast = v4("255.255.255.255", 80);
    result("tcp to broadcast", connect(b, (void *)&bcast, sizeof bcast));
    close(l);
    close(c4);
    close(a);
    close(c6);
    close(f);
    close(z);
    close(z4);
    close(b);
}

static void multiple(void) {
    int r = socket(AF_INET, SOCK_DGRAM, 0), s = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = v4("127.0.0.1", 5060);
    bind(r, (void *)&a, sizeof a);
    connect(s, (void *)&a, sizeof a);
    char *words[3] = {"one", "three", "seven"};
    struct mmsghdr out[3];
    struct iovec outv[3];
    memset(out, 0, sizeof out);
    for (int i = 0; i < 3; i++) {
        outv[i] = (struct iovec){.iov_base = words[i], .iov_len = strlen(words[i])};
        out[i].msg_hdr.msg_iov = &outv[i];
        out[i].msg_hdr.msg_iovlen = 1;
    }
    result("sendmmsg", sendmmsg(s, out, 3, 0));
    printf("lengths %u %u %u\n", out[0].msg_len, out[1].msg_len, out[2].msg_len);
    char buf[3][16];
    struct mmsghdr in[4];
    struct iovec inv[4];
    struct sockaddr_in from[4];
    memset(in, 0, sizeof in);
    for (int i = 0; i < 4; i++) {
        inv[i] = (struct iovec){.iov_base = buf[i % 3], .iov_len = 4};
        in[i].msg_hdr.msg_iov = &inv[i];
        in[i].msg_hdr.msg_iovlen = 1;
        in[i].msg_hdr.msg_name = &from[i];
        in[i].msg_hdr.msg_namelen = sizeof from[i];
    }
    result("recvmmsg", recvmmsg(r, in, 4, MSG_DONTWAIT, NULL));
    printf("got %u %u %u flags %#x %#x %#x names %u\n", in[0].msg_len, in[1].msg_len, in[2].msg_len,
           in[0].msg_hdr.msg_flags, in[1].msg_hdr.msg_flags, in[2].msg_hdr.msg_flags,
           in[0].msg_hdr.msg_namelen);
    result("recvmmsg empty", recvmmsg(r, in, 4, MSG_DONTWAIT, NULL));
    send(s, "x", 1, 0);
    result("recvmmsg wait for one", recvmmsg(r, in, 4, MSG_WAITFORONE, NULL));
    struct timespec bad = {.tv_nsec = -1};
    result("recvmmsg bad timeout", recvmmsg(r, in, 4, 0, &bad));
    result("sendmmsg none", sendmmsg(s, out, 0, 0));
    result("sendmmsg not a socket", sendmmsg(0, out, 1, 0));
    close(r);
    close(s);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    creating();
    options();
    binding();
    connecting();
    ports_held();
    filling();
    datagrams();
    stream_v6();
    multiple();
    return 0;
}
