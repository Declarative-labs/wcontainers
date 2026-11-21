// UDP segmentation (UDP_SEGMENT as an option and a control message) and receive offload
// (UDP_GRO): segments to most receivers, the datagram whole with its segment size to others.
#define _GNU_SOURCE
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <stdint.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netinet/udp.h>
#include <arpa/inet.h>
#ifndef SO_RCVMARK
#define SO_RCVMARK 75
#endif
#ifndef UDP_SEGMENT
#define UDP_SEGMENT 103
#endif
#ifndef UDP_GRO
#define UDP_GRO 104
#endif
// Reads everything a receiver holds, a run of datagrams alike as one line.
static void drain(const char *who, int fd) {
    char last[300] = "";
    int run = 0;
    for (;;) {
        char buf[70000], ctl[256], line[300];
        struct iovec v = {buf, sizeof buf};
        struct msghdr m = {.msg_iov = &v, .msg_iovlen = 1, .msg_control = ctl,
                           .msg_controllen = sizeof ctl};
        int n = recvmsg(fd, &m, MSG_DONTWAIT);
        if (n >= 0) {
            int at = snprintf(line, sizeof line, "%s %d flags %#x:", who, n, m.msg_flags);
            for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
                int x;
                memcpy(&x, CMSG_DATA(c), 4);
                at += snprintf(line + at, sizeof line - at, " [%d/%d len %lu %d]", c->cmsg_level,
                               c->cmsg_type, (unsigned long)c->cmsg_len, x);
            }
            if (run > 0 && strcmp(line, last) == 0) {
                run++;
                continue;
            }
        }
        if (run > 0)
            printf("  %d x %s\n", run, last);
        if (n < 0)
            break;
        strcpy(last, line);
        run = 1;
    }
}
static int A, B, A6;
static char data[70000];
static void sendseg(const char *what, int s, void *to, socklen_t tl, int len, int seg, int seglen, int flags) {
    char ctl[CMSG_SPACE(8)] = {0};
    struct iovec v = {data, len};
    struct msghdr m = {.msg_name = to, .msg_namelen = tl, .msg_iov = &v, .msg_iovlen = 1};
    if (seglen >= 0) {
        m.msg_control = ctl; m.msg_controllen = CMSG_SPACE(seglen);
        struct cmsghdr *c = CMSG_FIRSTHDR(&m); c->cmsg_level = SOL_UDP; c->cmsg_type = UDP_SEGMENT; c->cmsg_len = CMSG_LEN(seglen);
        uint16_t g = seg; memcpy(CMSG_DATA(c), &g, 2);
    }
    int r = sendmsg(s, &m, flags);
    printf("%s: %d %s\n", what, r, r < 0 ? strerror(errno) : "");
    drain("A", A); drain("B", B); drain("A6", A6);
}
static void opt(int fd, int name, int v, int len) {
    int r = setsockopt(fd, SOL_UDP, name, &v, len);
    int g = -9; socklen_t gl = 4; getsockopt(fd, SOL_UDP, name, &g, &gl);
    printf("set %d to %d (len %d): %d %s, now %d\n", name, v, len, r, r ? strerror(errno) : "", g);
}
int main(void) {
    int on = 1;
    memset(data, 'z', sizeof data);
    A = socket(AF_INET, SOCK_DGRAM, 0); B = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(7400)}; a.sin_addr.s_addr = htonl(0x7f000001);
    struct sockaddr_in b = a; b.sin_port = htons(7401);
    struct sockaddr_in none = a; none.sin_port = htons(7402);
    bind(A, (void *)&a, sizeof a); bind(B, (void *)&b, sizeof b);
    setsockopt(A, IPPROTO_IP, IP_RECVTTL, &on, 4);
    setsockopt(B, IPPROTO_IP, IP_RECVTTL, &on, 4);
    setsockopt(B, SOL_SOCKET, SO_RCVMARK, &on, 4);
    opt(B, UDP_GRO, 1, 4);
    opt(B, UDP_GRO, 5, 4);
    A6 = socket(AF_INET6, SOCK_DGRAM, 0);
    struct sockaddr_in6 a6 = {.sin6_family = AF_INET6, .sin6_port = htons(7403)}; a6.sin6_addr = in6addr_loopback;
    bind(A6, (void *)&a6, sizeof a6);
    int s = socket(AF_INET, SOCK_DGRAM, 0);
    opt(s, UDP_SEGMENT, -1, 4); opt(s, UDP_SEGMENT, 70000, 4); opt(s, UDP_SEGMENT, 65535, 4); opt(s, UDP_SEGMENT, 100, 2); opt(s, UDP_SEGMENT, 100, 4);
    sendseg("250 by 100 to A", s, &a, sizeof a, 250, 0, -1, 0);
    sendseg("250 by 100 to B", s, &b, sizeof b, 250, 0, -1, 0);
    sendseg("80 by 100 to B", s, &b, sizeof b, 80, 0, -1, 0);
    sendseg("100 by 100 to B", s, &b, sizeof b, 100, 0, -1, 0);
    sendseg("0 by 100 to B", s, &b, sizeof b, 0, 0, -1, 0);
    sendseg("12800 by 100 to A", s, &a, sizeof a, 12800, 0, -1, 0);
    sendseg("12801 by 100 to A", s, &a, sizeof a, 12801, 0, -1, 0);
    sendseg("cmsg 60, 130 to A", s, &a, sizeof a, 130, 60, 2, 0);
    sendseg("cmsg 0, 130 to A", s, &a, sizeof a, 130, 0, 2, 0);
    sendseg("cmsg short", s, &a, sizeof a, 130, 60, 1, 0);
    sendseg("cmsg long", s, &a, sizeof a, 130, 60, 4, 0);
    sendseg("to nobody", s, &none, sizeof none, 250, 0, -1, 0);
    setsockopt(s, IPPROTO_IP, IP_RECVERR, &on, 4);
    sendseg("to nobody with recverr", s, &none, sizeof none, 250, 0, -1, 0);
    { char buf[1000], ctl[512]; struct iovec v = {buf, sizeof buf}; struct msghdr m = {.msg_iov = &v, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
      int n = recvmsg(s, &m, MSG_ERRQUEUE | MSG_DONTWAIT); printf("  error read %d flags %#x\n", n, m.msg_flags);
      n = recvmsg(s, &m, MSG_ERRQUEUE | MSG_DONTWAIT); printf("  error read %d\n", n); }
    sendseg("more 50 cmsg 20", s, &a, sizeof a, 50, 20, 2, MSG_MORE);
    sendseg("then 50", s, &a, sizeof a, 50, 0, -1, 0);
    sendseg("65000 by 100 to B", s, &b, sizeof b, 6500, 0, -1, 0);
    opt(s, UDP_SEGMENT, 1, 4);
    sendseg("128 by 1 to A", s, &a, sizeof a, 128, 0, -1, 0);
    sendseg("129 by 1 to A", s, &a, sizeof a, 129, 0, -1, 0);
    opt(s, UDP_SEGMENT, 65507, 4);
    sendseg("65507 by 65507", s, &a, sizeof a, 65507, 0, -1, 0);
    opt(s, UDP_SEGMENT, 0, 4);
    int s6 = socket(AF_INET6, SOCK_DGRAM, 0);
    sendseg("v6 cmsg 40, 100", s6, &a6, sizeof a6, 100, 40, 2, 0);
    struct sockaddr_in6 m4 = {.sin6_family = AF_INET6, .sin6_port = htons(7400)}; inet_pton(AF_INET6, "::ffff:127.0.0.1", &m4.sin6_addr);
    sendseg("v6 to v4 cmsg 40, 100", s6, &m4, sizeof m4, 100, 40, 2, 0);
    int t = socket(AF_INET, SOCK_STREAM, 0);
    int v = 100; printf("tcp udp_segment %d %s\n", setsockopt(t, SOL_UDP, UDP_SEGMENT, &v, 4), strerror(errno));
    { char ctl[CMSG_SPACE(4)] = {0}; struct iovec io = {data, 10}; struct msghdr m = {.msg_name = &a, .msg_namelen = sizeof a, .msg_iov = &io, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
      struct cmsghdr *c = CMSG_FIRSTHDR(&m); c->cmsg_level = SOL_UDP; c->cmsg_type = 99; c->cmsg_len = CMSG_LEN(4);
      printf("udp type 99: %d %s\n", (int)sendmsg(s, &m, 0), strerror(errno)); drain("A", A);
      c->cmsg_level = IPPROTO_IP; c->cmsg_type = 99;
      struct cmsghdr *d = (void *)ctl; (void)d;
      printf("ip type 99: %d %s\n", (int)sendmsg(s, &m, 0), strerror(errno)); drain("A", A);
    }
    /* a bad IP message before a bad UDP one: which error wins */
    { char ctl[2 * CMSG_SPACE(4)] = {0}; struct iovec io = {data, 10}; struct msghdr m = {.msg_name = &a, .msg_namelen = sizeof a, .msg_iov = &io, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
      struct cmsghdr *c = CMSG_FIRSTHDR(&m); c->cmsg_level = SOL_SOCKET; c->cmsg_type = SO_MARK; c->cmsg_len = CMSG_LEN(2);
      c = CMSG_NXTHDR(&m, c); c->cmsg_level = SOL_UDP; c->cmsg_type = UDP_SEGMENT; c->cmsg_len = CMSG_LEN(4);
      printf("bad mark then bad segment: %d %s\n", (int)sendmsg(s, &m, 0), strerror(errno)); drain("A", A);
    }
    return 0;
}
