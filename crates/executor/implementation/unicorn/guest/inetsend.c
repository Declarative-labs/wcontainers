// Control messages of internet sends (packet information, hop limits, classes, marks,
// priorities, transmit times) and what receivers are told of them; marks, priorities and
// transmit times as socket options, as root and as a user.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <linux/net_tstamp.h>
#include <net/if.h>
#include <netinet/in.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifndef IP_PROTOCOL
#define IP_PROTOCOL 52
#endif
#ifndef IPV6_FLOWINFO
#define IPV6_FLOWINFO 11
#endif
#ifndef IPV6_HOPOPTS
#define IPV6_HOPOPTS 54
#endif
#ifndef SO_RCVMARK
#define SO_RCVMARK 75
#endif
#ifndef SO_RCVPRIORITY
#define SO_RCVPRIORITY 82
#endif

static int r4, r6, eth_index;
static struct in_addr eth_address;

static const char *index_name(int index) {
    static char text[16];
    if (index == 1)
        return "lo";
    if (index == eth_index)
        return "eth0";
    snprintf(text, sizeof text, "%d", index);
    return text;
}

static const char *v4_name(struct in_addr a) {
    static char text[2][20];
    static int which;
    which ^= 1;
    if (a.s_addr == eth_address.s_addr)
        return "eth0's";
    inet_ntop(AF_INET, &a, text[which], sizeof text[which]);
    return text[which];
}

// Reads what came to a receiver, if anything, with its source and control messages.
static void drain(const char *who, int fd) {
    char buf[64], ctl[512], name[64];
    struct iovec v = {buf, sizeof buf};
    struct msghdr m = {.msg_name = name, .msg_namelen = sizeof name, .msg_iov = &v,
                       .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
    int n = recvmsg(fd, &m, MSG_DONTWAIT);
    if (n < 0)
        return;
    printf("  %s got %d from ", who, n);
    if (((struct sockaddr *)name)->sa_family == AF_INET) {
        printf("%s", v4_name(((struct sockaddr_in *)name)->sin_addr));
    } else {
        char a[64];
        inet_ntop(AF_INET6, &((struct sockaddr_in6 *)name)->sin6_addr, a, sizeof a);
        printf("%s", a);
    }
    for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
        unsigned char *d = CMSG_DATA(c);
        int value;
        memcpy(&value, d, 4);
        if (c->cmsg_level == IPPROTO_IP && c->cmsg_type == IP_PKTINFO) {
            struct in_pktinfo p;
            memcpy(&p, d, sizeof p);
            printf(" pktinfo %s spec %s to %s", index_name(p.ipi_ifindex), v4_name(p.ipi_spec_dst),
                   v4_name(p.ipi_addr));
        } else if (c->cmsg_level == IPPROTO_IPV6 && c->cmsg_type == IPV6_PKTINFO) {
            struct in6_pktinfo p;
            char a[64];
            memcpy(&p, d, sizeof p);
            inet_ntop(AF_INET6, &p.ipi6_addr, a, sizeof a);
            printf(" pktinfo %s to %s", index_name(p.ipi6_ifindex), a);
        } else if (c->cmsg_level == IPPROTO_IP && c->cmsg_type == IP_TOS) {
            printf(" tos %#x", d[0]);
        } else {
            printf(" %d/%d %d", c->cmsg_level, c->cmsg_type, value);
        }
    }
    printf("\n");
}

static void send_with(const char *what, int fd, void *to, socklen_t tolen, int level, int type,
                      const void *data, int len) {
    char ctl[CMSG_SPACE(64)] = {0};
    struct iovec v = {"hi", 2};
    struct msghdr m = {.msg_name = to, .msg_namelen = tolen, .msg_iov = &v, .msg_iovlen = 1};
    if (len >= 0) {
        m.msg_control = ctl;
        m.msg_controllen = CMSG_SPACE(len);
        struct cmsghdr *c = CMSG_FIRSTHDR(&m);
        c->cmsg_level = level;
        c->cmsg_type = type;
        c->cmsg_len = CMSG_LEN(len);
        memcpy(CMSG_DATA(c), data, len);
    }
    int r = sendmsg(fd, &m, 0);
    printf("%s: %d %s\n", what, r, r < 0 ? strerror(errno) : "");
    drain("v4", r4);
    drain("v6", r6);
}

static void option(const char *what, int fd, int name, int value) {
    int r = setsockopt(fd, SOL_SOCKET, name, &value, sizeof value);
    int got = -9;
    socklen_t len = sizeof got;
    getsockopt(fd, SOL_SOCKET, name, &got, &len);
    printf("%s %d: %d %s, now %d\n", what, value, r, r < 0 ? strerror(errno) : "", got);
}

static void options(const char *who) {
    printf("options as %s\n", who);
    int u = socket(AF_INET, SOCK_DGRAM, 0), x = socket(AF_UNIX, SOCK_DGRAM, 0);
    int t = socket(AF_INET, SOCK_STREAM, 0);
    option("udp mark", u, SO_MARK, 5);
    option("local mark", x, SO_MARK, 6);
    option("udp priority", u, SO_PRIORITY, 7);
    option("udp priority", u, SO_PRIORITY, 3);
    option("local priority", x, SO_PRIORITY, -1);
    option("tcp priority", t, SO_PRIORITY, 9);
    option("udp receive marks", u, SO_RCVMARK, 4);
    option("udp receive priorities", u, SO_RCVPRIORITY, 1);
    struct sock_txtime st = {.clockid = CLOCK_MONOTONIC, .flags = 0};
    struct sock_txtime got = {.clockid = -5, .flags = 9};
    socklen_t len = sizeof got;
    int g = getsockopt(u, SOL_SOCKET, SO_TXTIME, &got, &len);
    printf("txtime before: %d length %u clock %d flags %u\n", g, len, got.clockid, got.flags);
    struct {
        const char *what;
        int clock;
        unsigned flags;
        int size;
    } tries[] = {
        {"monotonic", CLOCK_MONOTONIC, SOF_TXTIME_REPORT_ERRORS, sizeof st},
        {"bad flags", CLOCK_MONOTONIC, 4, sizeof st},
        {"realtime", CLOCK_REALTIME, 0, sizeof st},
        {"tai", CLOCK_TAI, 0, sizeof st},
        {"boottime", CLOCK_BOOTTIME, 0, sizeof st},
        {"short", CLOCK_MONOTONIC, 0, 4},
        {"long", CLOCK_MONOTONIC, 0, 12},
    };
    for (unsigned i = 0; i < sizeof tries / sizeof tries[0]; i++) {
        char raw[12] = {0};
        st.clockid = tries[i].clock;
        st.flags = tries[i].flags;
        memcpy(raw, &st, sizeof st);
        int r = setsockopt(u, SOL_SOCKET, SO_TXTIME, raw, tries[i].size);
        printf("txtime %s: %d %s\n", tries[i].what, r, r < 0 ? strerror(errno) : "");
    }
    len = sizeof got;
    g = getsockopt(u, SOL_SOCKET, SO_TXTIME, &got, &len);
    printf("txtime after: %d length %u clock %d flags %u\n", g, len, got.clockid, got.flags);
    int r = setsockopt(x, SOL_SOCKET, SO_TXTIME, &st, sizeof st);
    printf("txtime local: %d\n", r);
    close(u);
    close(x);
    close(t);
}

int main(void) {
    int on = 1, v;
    r4 = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in any4 = {.sin_family = AF_INET, .sin_port = htons(7300)};
    bind(r4, (void *)&any4, sizeof any4);
    setsockopt(r4, IPPROTO_IP, IP_PKTINFO, &on, 4);
    setsockopt(r4, IPPROTO_IP, IP_RECVTTL, &on, 4);
    setsockopt(r4, IPPROTO_IP, IP_RECVTOS, &on, 4);
    setsockopt(r4, SOL_SOCKET, SO_RCVMARK, &on, 4);
    setsockopt(r4, SOL_SOCKET, SO_RCVPRIORITY, &on, 4);
    r6 = socket(AF_INET6, SOCK_DGRAM, 0);
    setsockopt(r6, IPPROTO_IPV6, IPV6_V6ONLY, &on, 4);
    struct sockaddr_in6 any6 = {.sin6_family = AF_INET6, .sin6_port = htons(7300)};
    bind(r6, (void *)&any6, sizeof any6);
    setsockopt(r6, IPPROTO_IPV6, IPV6_RECVPKTINFO, &on, 4);
    setsockopt(r6, IPPROTO_IPV6, IPV6_RECVHOPLIMIT, &on, 4);
    setsockopt(r6, IPPROTO_IPV6, IPV6_RECVTCLASS, &on, 4);

    int s = socket(AF_INET, SOCK_DGRAM, 0);
    eth_index = if_nametoindex("eth0");
    struct ifreq ifr = {0};
    strcpy(ifr.ifr_name, "eth0");
    ioctl(s, SIOCGIFADDR, &ifr);
    eth_address = ((struct sockaddr_in *)&ifr.ifr_addr)->sin_addr;

    struct sockaddr_in lo = {.sin_family = AF_INET, .sin_port = htons(7300)};
    lo.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    struct sockaddr_in lo5 = lo, eth = lo, gateway = lo, other = lo;
    inet_pton(AF_INET, "127.0.0.5", &lo5.sin_addr);
    eth.sin_addr = eth_address;
    inet_pton(AF_INET, "172.17.0.1", &gateway.sin_addr);
    inet_pton(AF_INET, "10.9.9.9", &other.sin_addr);

    // IPv4 packet information: sources and interfaces.
    struct in_pktinfo pi = {0};
    send_with("plain", s, &lo, sizeof lo, 0, 0, 0, -1);
    send_with("pktinfo zero", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    pi.ipi_spec_dst = lo5.sin_addr;
    send_with("source 127.0.0.5", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    pi.ipi_spec_dst = eth_address;
    send_with("source eth0's to lo", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    send_with("source eth0's to eth0's", s, &eth, sizeof eth, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    pi.ipi_spec_dst = lo.sin_addr;
    send_with("source lo to eth0's", s, &eth, sizeof eth, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    send_with("source lo to the gateway", s, &gateway, sizeof gateway, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    send_with("source lo to 127.0.0.5", s, &lo5, sizeof lo5, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    pi.ipi_spec_dst = other.sin_addr;
    send_with("source foreign", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    inet_pton(AF_INET, "224.0.0.1", &pi.ipi_spec_dst);
    send_with("source multicast", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    inet_pton(AF_INET, "255.255.255.255", &pi.ipi_spec_dst);
    send_with("source broadcast", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    memset(&pi, 0, sizeof pi);
    pi.ipi_ifindex = 99;
    send_with("interface 99", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    pi.ipi_ifindex = 1;
    send_with("through lo to lo", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    send_with("through lo to eth0's", s, &eth, sizeof eth, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    send_with("through lo to elsewhere", s, &other, sizeof other, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    pi.ipi_ifindex = eth_index;
    send_with("through eth0 to eth0's", s, &eth, sizeof eth, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    send_with("through eth0 to lo", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);
    pi.ipi_spec_dst = lo.sin_addr;
    send_with("through eth0 from lo to eth0's", s, &eth, sizeof eth, IPPROTO_IP, IP_PKTINFO, &pi,
              sizeof pi);
    send_with("pktinfo short", s, &lo, sizeof lo, IPPROTO_IP, IP_PKTINFO, &pi, 8);
    int c = socket(AF_INET, SOCK_DGRAM, 0);
    connect(c, (void *)&lo, sizeof lo);
    memset(&pi, 0, sizeof pi);
    pi.ipi_spec_dst = lo5.sin_addr;
    send_with("connected, source 127.0.0.5", c, 0, 0, IPPROTO_IP, IP_PKTINFO, &pi, sizeof pi);

    // Other IPv4 and socket-level messages.
    v = 7;
    send_with("protocol 7", s, &lo, sizeof lo, IPPROTO_IP, IP_PROTOCOL, &v, 4);
    v = 0;
    send_with("protocol 0", s, &lo, sizeof lo, IPPROTO_IP, IP_PROTOCOL, &v, 4);
    v = 0x10;
    send_with("tos 0x10", s, &lo, sizeof lo, IPPROTO_IP, IP_TOS, &v, 4);
    v = 9;
    send_with("mark 9", s, &lo, sizeof lo, SOL_SOCKET, SO_MARK, &v, 4);
    send_with("mark short", s, &lo, sizeof lo, SOL_SOCKET, SO_MARK, &v, 2);
    v = 12;
    send_with("priority 12", s, &lo, sizeof lo, SOL_SOCKET, SO_PRIORITY, &v, 4);
    v = 0x10;
    setsockopt(s, IPPROTO_IP, IP_TOS, &v, 4);
    int prio = -1;
    socklen_t len = sizeof prio;
    getsockopt(s, SOL_SOCKET, SO_PRIORITY, &prio, &len);
    printf("priority after tos 0x10: %d\n", prio);
    v = 3;
    setsockopt(s, SOL_SOCKET, SO_PRIORITY, &v, 4);
    v = 0x10;
    setsockopt(s, IPPROTO_IP, IP_TOS, &v, 4);
    getsockopt(s, SOL_SOCKET, SO_PRIORITY, &prio, &len);
    printf("priority after the same tos again: %d\n", prio);
    v = 77;
    setsockopt(s, SOL_SOCKET, SO_MARK, &v, 4);
    send_with("mark 77 priority 3", s, &lo, sizeof lo, 0, 0, 0, -1);
    v = 0x08;
    send_with("then tos 0x08", s, &lo, sizeof lo, IPPROTO_IP, IP_TOS, &v, 4);
    uint64_t when = 1;
    send_with("txtime unset", s, &lo, sizeof lo, SOL_SOCKET, SCM_TXTIME, &when, 8);
    struct sock_txtime st = {.clockid = CLOCK_MONOTONIC};
    setsockopt(s, SOL_SOCKET, SO_TXTIME, &st, sizeof st);
    send_with("txtime", s, &lo, sizeof lo, SOL_SOCKET, SCM_TXTIME, &when, 8);
    send_with("txtime short", s, &lo, sizeof lo, SOL_SOCKET, SCM_TXTIME, &when, 4);
    send_with("ipv6 level on ipv4", s, &lo, sizeof lo, IPPROTO_IPV6, IPV6_TCLASS, &v, 4);
    send_with("level 99", s, &lo, sizeof lo, 99, 1, &v, 4);
    send_with("socket level unknown", s, &lo, sizeof lo, SOL_SOCKET, 99, &v, 4);

    // IPv6.
    int s6 = socket(AF_INET6, SOCK_DGRAM, 0);
    struct sockaddr_in6 lo6 = {.sin6_family = AF_INET6, .sin6_port = htons(7300)};
    lo6.sin6_addr = in6addr_loopback;
    struct sockaddr_in6 m4 = lo6;
    inet_pton(AF_INET6, "::ffff:127.0.0.1", &m4.sin6_addr);
    struct in6_pktinfo p6 = {0};
    send_with("v6 pktinfo zero", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6, sizeof p6);
    p6.ipi6_addr = in6addr_loopback;
    send_with("v6 source ::1", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6, sizeof p6);
    inet_pton(AF_INET6, "fd00::9", &p6.ipi6_addr);
    send_with("v6 source foreign", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6,
              sizeof p6);
    inet_pton(AF_INET6, "fe80::1", &p6.ipi6_addr);
    send_with("v6 source link-local", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6,
              sizeof p6);
    memset(&p6, 0, sizeof p6);
    p6.ipi6_ifindex = 99;
    send_with("v6 interface 99", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6, sizeof p6);
    p6.ipi6_ifindex = 1;
    send_with("v6 through lo", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6, sizeof p6);
    p6.ipi6_ifindex = eth_index;
    send_with("v6 through eth0", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6, sizeof p6);
    send_with("v6 pktinfo short", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_PKTINFO, &p6, 8);
    memset(&p6, 0, sizeof p6);
    inet_pton(AF_INET6, "::ffff:127.0.0.5", &p6.ipi6_addr);
    send_with("v6 to v4, mapped source", s6, &m4, sizeof m4, IPPROTO_IPV6, IPV6_PKTINFO, &p6,
              sizeof p6);
    send_with("v6 to v4, mapped source short", s6, &m4, sizeof m4, IPPROTO_IPV6, IPV6_PKTINFO,
              &p6, 16);
    p6.ipi6_ifindex = 99;
    send_with("v6 to v4, interface 99", s6, &m4, sizeof m4, IPPROTO_IPV6, IPV6_PKTINFO, &p6,
              sizeof p6);
    p6.ipi6_addr = in6addr_loopback;
    p6.ipi6_ifindex = 0;
    send_with("v6 to v4, source ::1", s6, &m4, sizeof m4, IPPROTO_IPV6, IPV6_PKTINFO, &p6,
              sizeof p6);
    struct in_pktinfo spec = {0};
    spec.ipi_spec_dst = lo5.sin_addr;
    send_with("v6 to v4, ipv4 pktinfo", s6, &m4, sizeof m4, IPPROTO_IP, IP_PKTINFO, &spec,
              sizeof spec);
    int values[] = {9, -1, 256};
    for (int i = 0; i < 3; i++) {
        char what[40];
        snprintf(what, sizeof what, "v6 hop limit %d", values[i]);
        send_with(what, s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_HOPLIMIT, &values[i], 4);
    }
    send_with("v6 hop limit short", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_HOPLIMIT, &v, 2);
    int classes[] = {0x28, -1, -2};
    for (int i = 0; i < 3; i++) {
        char what[40];
        snprintf(what, sizeof what, "v6 class %d", classes[i]);
        send_with(what, s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_TCLASS, &classes[i], 4);
    }
    int frags[] = {1, -1, 2};
    for (int i = 0; i < 3; i++) {
        char what[40];
        snprintf(what, sizeof what, "v6 dontfrag %d", frags[i]);
        send_with(what, s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_DONTFRAG, &frags[i], 4);
    }
    uint32_t flow = htonl(0x12345);
    send_with("v6 flow", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_FLOWINFO, &flow, 4);
    send_with("v6 flow short", s6, &lo6, sizeof lo6, IPPROTO_IPV6, IPV6_FLOWINFO, &flow, 2);
    send_with("v6 type 99", s6, &lo6, sizeof lo6, IPPROTO_IPV6, 99, &v, 4);
    v = 5;
    send_with("v6 ip ttl", s6, &lo6, sizeof lo6, IPPROTO_IP, IP_TTL, &v, 4);
    send_with("v6 to v4 ip ttl", s6, &m4, sizeof m4, IPPROTO_IP, IP_TTL, &v, 4);
    send_with("v6 to v4 hop limit", s6, &m4, sizeof m4, IPPROTO_IPV6, IPV6_HOPLIMIT, &v, 4);
    v = 0x10;
    setsockopt(s6, IPPROTO_IP, IP_TOS, &v, 4);
    v = 5;
    setsockopt(s6, SOL_SOCKET, SO_MARK, &v, 4);
    send_with("v6 to v4 after ip tos and mark", s6, &m4, sizeof m4, 0, 0, 0, -1);

    // TCP reads only the socket level, and any refusal is EINVAL.
    int l = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in tl = lo;
    tl.sin_port = htons(7301);
    bind(l, (void *)&tl, sizeof tl);
    listen(l, 1);
    int t = socket(AF_INET, SOCK_STREAM, 0);
    connect(t, (void *)&tl, sizeof tl);
    v = 5;
    send_with("tcp mark", t, 0, 0, SOL_SOCKET, SO_MARK, &v, 4);
    send_with("tcp mark short", t, 0, 0, SOL_SOCKET, SO_MARK, &v, 2);
    send_with("tcp pktinfo", t, 0, 0, IPPROTO_IP, IP_PKTINFO, &pi, 3);

    options("root");
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        if (setuid(65534) < 0)
            return 1;
        options("a user");
        int u = socket(AF_INET, SOCK_DGRAM, 0);
        v = 9;
        send_with("user mark", u, &lo, sizeof lo, SOL_SOCKET, SO_MARK, &v, 4);
        send_with("user priority 9", u, &lo, sizeof lo, SOL_SOCKET, SO_PRIORITY, &v, 4);
        v = 6;
        send_with("user priority 6", u, &lo, sizeof lo, SOL_SOCKET, SO_PRIORITY, &v, 4);
        return 0;
    }
    int status;
    waitpid(child, &status, 0);
    printf("child %d\n", WEXITSTATUS(status));
    return 0;
}
