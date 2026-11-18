// Socket corners: TCP sendmsg's control messages by state, SO_RCVLOWAT's caps and poll,
// recvmmsg interrupted and with SO_RCVTIMEO, autobind of a path-bound socket, socket mappings
// under RLIMIT_DATA and RLIMIT_AS, UDP's coverage and encapsulation options, and a datagram to
// the all-hosts group.
#define _GNU_SOURCE
#include <errno.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <signal.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#include <sys/epoll.h>
#include <sys/ioctl.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static void tcp_pair(int port, int *c, int *s) {
    int l = socket(AF_INET, SOCK_STREAM, 0);
    int one = 1;
    setsockopt(l, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(l, (void *)&a, sizeof a);
    listen(l, 4);
    *c = socket(AF_INET, SOCK_STREAM, 0);
    connect(*c, (void *)&a, sizeof a);
    *s = accept(l, NULL, NULL);
    close(l);
}

static long bad_cmsg_send(int fd) {
    char ctl[24] = {0};
    struct cmsghdr *c = (struct cmsghdr *)ctl;
    c->cmsg_len = 100;
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    char byte = 'x';
    struct iovec io = {&byte, 1};
    struct msghdr m = {.msg_iov = &io, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
    return sendmsg(fd, &m, MSG_NOSIGNAL | MSG_DONTWAIT);
}

static void tcp_states(void) {
    int c, s;
    // Shut for writing: FIN_WAIT, so sk_stream_wait_connect says EPIPE first.
    tcp_pair(7501, &c, &s);
    shutdown(c, SHUT_WR);
    result("shut wr bad control", bad_cmsg_send(c));
    // The peer closed: CLOSE_WAIT, control messages are read.
    result("close wait bad control", bad_cmsg_send(s));
    close(c);
    close(s);
    // A listener.
    int l = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7502), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(l, (void *)&a, sizeof a);
    listen(l, 1);
    result("listener bad control", bad_cmsg_send(l));
    close(l);
    // A reset connection: its error comes first.
    tcp_pair(7503, &c, &s);
    struct linger lg = {1, 0};
    setsockopt(s, SOL_SOCKET, SO_LINGER, &lg, sizeof lg);
    close(s);
    usleep(50000);
    result("reset bad control", bad_cmsg_send(c));
    result("reset bad control again", bad_cmsg_send(c));
    close(c);
    // Both shut.
    tcp_pair(7504, &c, &s);
    shutdown(c, SHUT_RDWR);
    result("shut rdwr bad control", bad_cmsg_send(c));
    close(c);
    close(s);
}

static void lowat(void) {
    int c, s;
    tcp_pair(7505, &c, &s);
    int rcv = 2048;
    setsockopt(s, SOL_SOCKET, SO_RCVBUF, &rcv, sizeof rcv);
    int big = 100000;
    result("set lowat big", setsockopt(s, SOL_SOCKET, SO_RCVLOWAT, &big, sizeof big));
    int got = 0;
    socklen_t gl = sizeof got;
    getsockopt(s, SOL_SOCKET, SO_RCVLOWAT, &got, &gl);
    printf("lowat read back %d\n", got);
    getsockopt(s, SOL_SOCKET, SO_RCVBUF, &got, &gl);
    printf("rcvbuf read back %d\n", got);
    static char buf[3000];
    write(c, buf, 3000);
    usleep(50000);
    struct pollfd p = {.fd = s, .events = POLLIN};
    result("poll 3000 with big lowat", poll(&p, 1, 0));
    fd_set r;
    FD_ZERO(&r);
    FD_SET(s, &r);
    struct timeval tv = {0, 0};
    result("select 3000 with big lowat", select(s + 1, &r, NULL, NULL, &tv));
    int ep = epoll_create1(0);
    struct epoll_event ev = {.events = EPOLLIN, .data.fd = s}, out;
    epoll_ctl(ep, EPOLL_CTL_ADD, s, &ev);
    result("epoll 3000 with big lowat", epoll_wait(ep, &out, 1, 0));
    int n = 0;
    ioctl(s, FIONREAD, &n);
    printf("fionread %d\n", n);
    close(c);
    close(s);
    // A small lowat; then the peer shuts: POLLIN at the end whatever the lowat.
    tcp_pair(7506, &c, &s);
    int ten = 10;
    setsockopt(s, SOL_SOCKET, SO_RCVLOWAT, &ten, sizeof ten);
    write(c, "ab", 2);
    shutdown(c, SHUT_WR);
    usleep(50000);
    p.fd = s;
    p.revents = 0;
    result("poll 2 bytes and fin with lowat 10", poll(&p, 1, 0));
    printf("revents %x\n", p.revents);
    close(c);
    close(s);
    // UDP ignores lowat in poll.
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7507), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(u, (void *)&a, sizeof a);
    setsockopt(u, SOL_SOCKET, SO_RCVLOWAT, &ten, sizeof ten);
    sendto(u, "x", 1, 0, (void *)&a, sizeof a);
    p.fd = u;
    p.revents = 0;
    result("udp poll with lowat 10", poll(&p, 1, 0));
    close(u);
    // Local streams: lowat and poll.
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    result("unix set lowat", setsockopt(sv[1], SOL_SOCKET, SO_RCVLOWAT, &ten, sizeof ten));
    write(sv[0], "abc", 3);
    p.fd = sv[1];
    p.revents = 0;
    result("unix poll 3 with lowat 10", poll(&p, 1, 0));
    close(sv[0]);
    close(sv[1]);
    // Negative marks are the most there is; TCP's is capped by an unset buffer, which grows.
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    int minus = -5;
    setsockopt(sv[0], SOL_SOCKET, SO_RCVLOWAT, &minus, sizeof minus);
    getsockopt(sv[0], SOL_SOCKET, SO_RCVLOWAT, &got, &gl);
    printf("unix negative lowat read back %d\n", got);
    close(sv[0]);
    close(sv[1]);
    tcp_pair(7509, &c, &s);
    int huge = 1 << 30;
    setsockopt(s, SOL_SOCKET, SO_RCVLOWAT, &huge, sizeof huge);
    getsockopt(s, SOL_SOCKET, SO_RCVLOWAT, &got, &gl);
    printf("unlocked lowat read back %d\n", got);
    getsockopt(s, SOL_SOCKET, SO_RCVBUF, &got, &gl);
    printf("unlocked rcvbuf after %d\n", got);
    getsockopt(s, IPPROTO_TCP, TCP_WINDOW_CLAMP, &got, &gl);
    printf("unlocked window clamp after %d\n", got);
    setsockopt(c, SOL_SOCKET, SO_RCVLOWAT, &minus, sizeof minus);
    getsockopt(c, SOL_SOCKET, SO_RCVLOWAT, &got, &gl);
    printf("tcp negative lowat read back %d\n", got);
    int small = 1000;
    setsockopt(c, SOL_SOCKET, SO_RCVLOWAT, &small, sizeof small);
    getsockopt(c, SOL_SOCKET, SO_RCVBUF, &got, &gl);
    printf("small lowat leaves rcvbuf %d\n", got);
    // A mark lowered under what is queued makes the socket readable.
    write(s, buf, 500);
    usleep(50000);
    p.fd = c;
    p.revents = 0;
    result("poll 500 with lowat 1000", poll(&p, 1, 0));
    int four = 400;
    setsockopt(c, SOL_SOCKET, SO_RCVLOWAT, &four, sizeof four);
    result("poll 500 with lowat 400", poll(&p, 1, 0));
    close(c);
    close(s);
}

static void udp_options(void) {
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    static const int names[] = {10, 11, 100};
    for (int i = 0; i < 3; i++) {
        int v = -1;
        socklen_t l = sizeof v;
        long r = getsockopt(u, IPPROTO_UDP, names[i], &v, &l);
        printf("udp option %d read: %ld %s value %d length %d\n", names[i], r, r < 0 ? strerror(errno) : "", v, (int)l);
        v = 8;
        r = setsockopt(u, IPPROTO_UDP, names[i], &v, sizeof v);
        if (names[i] != 100)
            result("udp option set", r);
    }
    close(u);
}

// A peer closed with unread messages leaves its error first in line for each kind.
static void closed_with_unread(void) {
    static const int kinds[] = {SOCK_STREAM, SOCK_SEQPACKET, SOCK_DGRAM};
    static const char *names[] = {"stream", "seqpacket", "datagram"};
    for (int i = 0; i < 3; i++) {
        for (int oob = 0; oob < 2; oob++) {
            int sv[2];
            socketpair(AF_UNIX, kinds[i], 0, sv);
            send(sv[0], "unread", 6, 0);
            close(sv[1]);
            char what[64];
            snprintf(what, sizeof what, "%s%s send after the peer closed", names[i], oob ? " urgent" : "");
            result(what, send(sv[0], "x", 1, MSG_NOSIGNAL | MSG_DONTWAIT | (oob ? MSG_OOB : 0)));
            result("and again", send(sv[0], "x", 1, MSG_NOSIGNAL | MSG_DONTWAIT));
            close(sv[0]);
        }
    }
}

static void all_hosts(void) {
    int r = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7510)};
    bind(r, (void *)&a, sizeof a);
    int s = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in to = {.sin_family = AF_INET, .sin_port = htons(7510), .sin_addr.s_addr = htonl(0xe0000001)};
    result("send to all hosts", sendto(s, "all", 3, 0, (void *)&to, sizeof to));
    to.sin_addr.s_addr = htonl(0xe0010203);
    result("send to another group", sendto(s, "one", 3, 0, (void *)&to, sizeof to));
    usleep(50000);
    char b[8];
    long n;
    while ((n = recv(r, b, sizeof b, MSG_DONTWAIT)) > 0)
        printf("received %.*s\n", (int)n, b);
    close(r);
    close(s);
}

static void on_alarm(int sig) { (void)sig; }

static void batches(void) {
    struct sigaction sa = {.sa_handler = on_alarm};
    sigaction(SIGALRM, &sa, NULL);
    int r = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7508), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(r, (void *)&a, sizeof a);
    sendto(r, "first", 5, 0, (void *)&a, sizeof a);
    char b1[16], b2[16];
    struct iovec i1 = {b1, sizeof b1}, i2 = {b2, sizeof b2};
    struct mmsghdr v[2] = {{.msg_hdr = {.msg_iov = &i1, .msg_iovlen = 1}}, {.msg_hdr = {.msg_iov = &i2, .msg_iovlen = 1}}};
    struct itimerval it = {.it_value = {0, 200000}};
    setitimer(ITIMER_REAL, &it, NULL);
    errno = 0;
    int n = recvmmsg(r, v, 2, 0, NULL);
    printf("recvmmsg interrupted: %d errno %d lengths %u %u\n", n, n < 0 ? errno : 0, v[0].msg_len, v[1].msg_len);
    int err = 0;
    socklen_t el = sizeof err;
    getsockopt(r, SOL_SOCKET, SO_ERROR, &err, &el);
    printf("so_error after: %d\n", err);
    // With SO_RCVTIMEO the second message's wait ends.
    struct timeval tv = {0, 100000};
    setsockopt(r, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
    sendto(r, "again", 5, 0, (void *)&a, sizeof a);
    v[0].msg_len = v[1].msg_len = 0;
    n = recvmmsg(r, v, 2, 0, NULL);
    printf("recvmmsg rcvtimeo: %d lengths %u %u\n", n, v[0].msg_len, v[1].msg_len);
    getsockopt(r, SOL_SOCKET, SO_ERROR, &err, &el);
    printf("so_error after: %d\n", err);
    // With SA_RESTART.
    sa.sa_flags = SA_RESTART;
    sigaction(SIGALRM, &sa, NULL);
    tv.tv_usec = 0;
    setsockopt(r, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
    sendto(r, "third", 5, 0, (void *)&a, sizeof a);
    setitimer(ITIMER_REAL, &it, NULL);
    pid_t child = fork();
    if (child == 0) {
        usleep(400000);
        sendto(r, "fourth!", 7, 0, (void *)&a, sizeof a);
        _exit(0);
    }
    v[0].msg_len = v[1].msg_len = 0;
    n = recvmmsg(r, v, 2, 0, NULL);
    printf("recvmmsg restart: %d errno %d lengths %u %u\n", n, n < 0 ? errno : 0, v[0].msg_len, v[1].msg_len);
    waitpid(child, NULL, 0);
    getsockopt(r, SOL_SOCKET, SO_ERROR, &err, &el);
    printf("so_error after: %d\n", err);
    close(r);
}

static void binds(void) {
    int x = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un named = {.sun_family = AF_UNIX, .sun_path = "/tmp/sockedges.sock"};
    unlink(named.sun_path);
    result("bind path", bind(x, (void *)&named, sizeof named));
    struct sockaddr_un un = {.sun_family = AF_UNIX};
    result("autobind of path-bound", bind(x, (void *)&un, sizeof(sa_family_t)));
    struct sockaddr_un got;
    socklen_t gl = sizeof got;
    getsockname(x, (void *)&got, &gl);
    printf("name after: %d %s\n", (int)gl, got.sun_path);
    struct sockaddr_un wrong = {.sun_family = AF_INET};
    int y = socket(AF_UNIX, SOCK_DGRAM, 0);
    result("family-only bind with wrong family", bind(y, (void *)&wrong, sizeof(sa_family_t)));
    result("bind length 1", bind(y, (void *)&un, 1));
    close(x);
    close(y);
    unlink(named.sun_path);
}

static void socket_mmap(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    errno = 0;
    void *p = mmap(NULL, 65536, PROT_READ | PROT_WRITE, MAP_PRIVATE, sv[0], 0);
    printf("mmap unix socket: %s\n", p == MAP_FAILED ? strerror(errno) : "mapped");
    struct rlimit rl = {4096, 4096};
    pid_t child = fork();
    if (child == 0) {
        setrlimit(RLIMIT_DATA, &rl);
        errno = 0;
        p = mmap(NULL, 65536, PROT_READ | PROT_WRITE, MAP_PRIVATE, sv[0], 0);
        printf("mmap unix socket under data limit: %s\n", p == MAP_FAILED ? strerror(errno) : "mapped");
        p = mmap(NULL, 65536, PROT_READ, MAP_PRIVATE, sv[0], 0);
        printf("mmap unix socket read only under data limit: %s\n", p == MAP_FAILED ? strerror(errno) : "mapped");
        p = mmap(NULL, 65536, PROT_READ | PROT_WRITE, MAP_SHARED, sv[0], 0);
        printf("mmap unix socket shared under data limit: %s\n", p == MAP_FAILED ? strerror(errno) : "mapped");
        rl.rlim_cur = rl.rlim_max = 1 << 20;
        struct rlimit as = {1 << 20, 1 << 20};
        setrlimit(RLIMIT_AS, &as);
        p = mmap(NULL, 64 << 20, PROT_READ, MAP_SHARED, sv[0], 0);
        printf("mmap unix socket over address limit: %s\n", p == MAP_FAILED ? strerror(errno) : "mapped");
        _exit(0);
    }
    waitpid(child, NULL, 0);
    // A refused mapping over a fixed range leaves the range unmapped.
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    int t = socket(AF_INET, SOCK_STREAM, 0);
    for (int i = 0; i < 2; i++) {
        char *area = mmap(NULL, 65536, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        area[0] = 1;
        errno = 0;
        p = mmap(area, 65536, i ? PROT_READ | PROT_WRITE : PROT_READ, MAP_SHARED | MAP_FIXED, i ? t : u, 0);
        unsigned char resident;
        long r = mincore(area, 4096, &resident);
        printf("fixed mapping of %s socket: %s; range mapped after: %s\n", i ? "a tcp" : "a udp",
               p == MAP_FAILED ? strerror(errno) : "mapped", r == 0 ? "yes" : strerror(errno));
    }
    close(u);
    close(t);
}

// Options a TCP connection takes and keeps but whose effect on the wire is not built.
static void unacted(void) {
    int c, s, one = 1;
    tcp_pair(7512, &c, &s);
    result("cork", setsockopt(c, IPPROTO_TCP, TCP_CORK, &one, sizeof one));
    result("fast open connect when connected", setsockopt(c, IPPROTO_TCP, TCP_FASTOPEN_CONNECT, &one, sizeof one));
    int fresh = socket(AF_INET, SOCK_STREAM, 0);
    int two = 2;
    result("fast open connect 2", setsockopt(fresh, IPPROTO_TCP, TCP_FASTOPEN_CONNECT, &two, sizeof two));
    result("fast open connect", setsockopt(fresh, IPPROTO_TCP, TCP_FASTOPEN_CONNECT, &one, sizeof one));
    close(fresh);
    result("save syn", setsockopt(c, IPPROTO_TCP, TCP_SAVE_SYN, &one, sizeof one));
    int delay = 1000;
    result("transmit delay", setsockopt(c, IPPROTO_TCP, TCP_TX_DELAY, &delay, sizeof delay));
    unsigned rate = 1000000;
    result("pacing rate", setsockopt(c, SOL_SOCKET, SO_MAX_PACING_RATE, &rate, sizeof rate));
    result("send more", send(c, "ab", 2, MSG_MORE));
    close(c);
    close(s);
}

// A local datagram socket that finds its peer gone, or connects to another, loses what it held;
// a live peer connected back is told ECONNRESET.
static void dgram_disconnects(void) {
    int p[2];
    socketpair(AF_UNIX, SOCK_DGRAM | SOCK_NONBLOCK, 0, p);
    send(p[0], "", 0, 0);
    send(p[0], "ab", 2, 0);
    close(p[0]);
    result("send to the gone peer", send(p[1], "x", 1, MSG_DONTWAIT));
    char buf[8];
    result("receive after", recv(p[1], buf, sizeof buf, MSG_DONTWAIT));
    result("send again", send(p[1], "x", 1, MSG_DONTWAIT));
    int a = socket(AF_UNIX, SOCK_DGRAM | SOCK_NONBLOCK, 0), b = socket(AF_UNIX, SOCK_DGRAM | SOCK_NONBLOCK, 0),
        c = socket(AF_UNIX, SOCK_DGRAM | SOCK_NONBLOCK, 0);
    struct sockaddr_un na = {AF_UNIX, "\0dga"}, nb = {AF_UNIX, "\0dgb"}, nc = {AF_UNIX, "\0dgc"};
    socklen_t length = offsetof(struct sockaddr_un, sun_path) + 4;
    bind(a, (void *)&na, length);
    bind(b, (void *)&nb, length);
    bind(c, (void *)&nc, length);
    connect(a, (void *)&nb, length);
    connect(b, (void *)&na, length);
    send(b, "to a", 4, 0);
    result("reconnect", connect(a, (void *)&nc, length));
    result("a receives", recv(a, buf, sizeof buf, MSG_DONTWAIT));
    int error = 0;
    socklen_t size = sizeof error;
    getsockopt(b, SOL_SOCKET, SO_ERROR, &error, &size);
    printf("b's error %d\n", error);
    close(a);
    close(b);
    close(c);
}

// Reads of no bytes from a local stream, with and without an urgent byte queued after others:
// a peek goes on past what it wants to the urgent byte.
static void zero_reads(void) {
    static char data[4096], buf[8];
    for (int oob = 0; oob < 2; oob++)
        for (int peek = 0; peek < 2; peek++)
            for (int length = 1; length <= 4096; length *= 4096) {
                int p[2];
                socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, p);
                send(p[0], data, length, oob ? MSG_OOB : 0);
                printf("urgent %d peek %d length %d: ", oob, peek, length);
                result("no bytes", recv(p[1], buf, 0, (peek ? MSG_PEEK : 0) | MSG_DONTWAIT));
                result("then", recv(p[1], buf, sizeof buf, MSG_DONTWAIT));
                close(p[0]);
                close(p[1]);
            }
    // A peek of exactly the first buffer's bytes, with an urgent byte after.
    int p[2];
    socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, p);
    send(p[0], "abc", 3, 0);
    send(p[0], "de", 2, MSG_OOB);
    result("peek of the first", recv(p[1], buf, 3, MSG_PEEK | MSG_DONTWAIT));
    result("peek past it", recv(p[1], buf, 5, MSG_PEEK | MSG_DONTWAIT));
    close(p[0]);
    close(p[1]);
}

// listen on local sockets that are connected: refused unnamed, and a call that changes nothing
// when named, for socket pairs, connected and accepted sockets, also after the peer closed.
static void connected_listens(void) {
    char buf[4];
    int types[] = {SOCK_STREAM, SOCK_SEQPACKET, SOCK_DGRAM};
    for (int t = 0; t < 3; t++) {
        for (int named = 0; named < 2; named++) {
            int p[2];
            socketpair(AF_UNIX, types[t] | SOCK_NONBLOCK, 0, p);
            printf("pair of type %d named %d: ", types[t], named);
            struct sockaddr_un automatic = {AF_UNIX};
            if (named)
                bind(p[0], (void *)&automatic, sizeof(sa_family_t));
            result("listen", listen(p[0], -1));
            int accepting = 0;
            socklen_t size = sizeof accepting;
            getsockopt(p[0], SOL_SOCKET, SO_ACCEPTCONN, &accepting, &size);
            printf("accepting %d\n", accepting);
            send(p[1], "y", 1, MSG_DONTWAIT);
            result("receive", recv(p[0], buf, sizeof buf, MSG_DONTWAIT));
            result("accept", accept4(p[0], 0, 0, SOCK_NONBLOCK));
            close(p[0]);
            close(p[1]);
        }
    }
    for (int type = 1; type <= 5; type += 4) {
        struct sockaddr_un server = {AF_UNIX, "\0lsrv"}, client = {AF_UNIX, "\0lcli"};
        socklen_t length = offsetof(struct sockaddr_un, sun_path) + 5;
        int l = socket(AF_UNIX, type, 0);
        bind(l, (void *)&server, length);
        listen(l, 5);
        int c = socket(AF_UNIX, type | SOCK_NONBLOCK, 0);
        bind(c, (void *)&client, length);
        connect(c, (void *)&server, length);
        printf("type %d: ", type);
        result("listen on the client", listen(c, 3));
        int a = accept4(l, 0, 0, SOCK_NONBLOCK);
        result("listen on the accepted", listen(a, 3));
        int other = socket(AF_UNIX, type, 0);
        result("connect to the client", connect(other, (void *)&client, length));
        close(a);
        result("listen after the peer closed", listen(c, 3));
        result("receive", recv(c, buf, sizeof buf, MSG_DONTWAIT));
        close(other);
        close(c);
        close(l);
    }
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    signal(SIGPIPE, SIG_IGN);
    if (argc > 1 && !strcmp(argv[1], "unacted")) {
        unacted();
        return 0;
    }
    tcp_states();
    lowat();
    batches();
    binds();
    socket_mmap();
    udp_options();
    closed_with_unread();
    all_hosts();
    dgram_disconnects();
    zero_reads();
    connected_listens();
    return 0;
}
