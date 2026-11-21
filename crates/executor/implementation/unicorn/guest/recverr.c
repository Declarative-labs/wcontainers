// UDP's error queue: port unreachable errors under IP_RECVERR and IPV6_RECVERR, read with
// MSG_ERRQUEUE, and what the socket reports without them.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <time.h>
#include <linux/errqueue.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static short revents(int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLPRI};
    poll(&p, 1, 0);
    return p.revents;
}

static void address(const struct sockaddr *a) {
    char text[64] = "";
    if (a->sa_family == AF_INET) {
        const struct sockaddr_in *in = (const void *)a;
        inet_ntop(AF_INET, &in->sin_addr, text, sizeof text);
        printf(" %s:%d", text, ntohs(in->sin_port));
    } else if (a->sa_family == AF_INET6) {
        const struct sockaddr_in6 *in = (const void *)a;
        inet_ntop(AF_INET6, &in->sin6_addr, text, sizeof text);
        printf(" [%s]:%d", text, ntohs(in->sin6_port));
    } else {
        printf(" family %d", a->sa_family);
    }
}

// One read of the error queue: the bytes, the name, the flags and each control message.
static void read_errors(const char *what, int fd, size_t room, size_t control_room, int flags) {
    static char data[4096];
    memset(data, 0, sizeof data);
    struct sockaddr_storage name;
    memset(&name, 0xee, sizeof name);
    char control[256];
    memset(control, 0, sizeof control);
    struct iovec iov = {.iov_base = data, .iov_len = room};
    struct msghdr msg = {.msg_name = &name, .msg_namelen = sizeof name, .msg_iov = &iov,
                         .msg_iovlen = 1, .msg_control = control,
                         .msg_controllen = control_room};
    long r = recvmsg(fd, &msg, MSG_ERRQUEUE | MSG_DONTWAIT | flags);
    if (r < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
        return;
    }
    printf("%s: %ld \"%.*s\" flags %#x namelen %d", what, r, (int)(r < 16 ? r : 16), data,
           msg.msg_flags, msg.msg_namelen);
    if (msg.msg_namelen)
        address((struct sockaddr *)&name);
    printf(" controllen %zu\n", (size_t)msg.msg_controllen);
    for (struct cmsghdr *c = CMSG_FIRSTHDR(&msg); c; c = CMSG_NXTHDR(&msg, c)) {
        printf("  cmsg level %d type %d len %zu", c->cmsg_level, c->cmsg_type, (size_t)c->cmsg_len);
        if ((c->cmsg_level == SOL_IP && c->cmsg_type == IP_RECVERR) ||
            (c->cmsg_level == SOL_IPV6 && c->cmsg_type == IPV6_RECVERR)) {
            struct sock_extended_err *e = (void *)CMSG_DATA(c);
            printf(" errno %u origin %u type %u code %u pad %u info %u data %u", e->ee_errno,
                   e->ee_origin, e->ee_type, e->ee_code, e->ee_pad, e->ee_info, e->ee_data);
            if (c->cmsg_len >= CMSG_LEN(sizeof *e + sizeof(struct sockaddr_in)))
                address(SO_EE_OFFENDER(e));
        } else if (c->cmsg_level == SOL_IP && c->cmsg_type == IP_PKTINFO) {
            struct in_pktinfo *p = (void *)CMSG_DATA(c);
            char a[32], b[32];
            inet_ntop(AF_INET, &p->ipi_spec_dst, a, sizeof a);
            inet_ntop(AF_INET, &p->ipi_addr, b, sizeof b);
            printf(" index %d %s %s", p->ipi_ifindex, a, b);
        } else if (c->cmsg_level == SOL_IPV6 && c->cmsg_type == IPV6_PKTINFO) {
            struct in6_pktinfo *p = (void *)CMSG_DATA(c);
            char a[64];
            inet_ntop(AF_INET6, &p->ipi6_addr, a, sizeof a);
            printf(" index %d %s", p->ipi6_ifindex, a);
        } else {
            for (size_t i = 0; i < c->cmsg_len - CMSG_LEN(0); i++)
                printf(" %02x", CMSG_DATA(c)[i]);
        }
        printf("\n");
    }
}

static int udp(int family, int port) {
    int s = socket(family, SOCK_DGRAM, 0);
    if (family == AF_INET) {
        struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port),
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
        bind(s, (struct sockaddr *)&a, sizeof a);
    } else {
        struct sockaddr_in6 a = {.sin6_family = AF_INET6, .sin6_port = htons(port),
                                 .sin6_addr = IN6ADDR_LOOPBACK_INIT};
        bind(s, (struct sockaddr *)&a, sizeof a);
    }
    return s;
}

static void send_to(int s, int family, int port, const char *bytes, size_t length) {
    long r;
    if (family == AF_INET) {
        struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port),
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
        r = sendto(s, bytes, length, 0, (struct sockaddr *)&a, sizeof a);
    } else {
        struct sockaddr_in6 a = {.sin6_family = AF_INET6, .sin6_port = htons(port),
                                 .sin6_addr = IN6ADDR_LOOPBACK_INIT};
        r = sendto(s, bytes, length, 0, (struct sockaddr *)&a, sizeof a);
    }
    if (r < 0)
        printf("send: -1 %s\n", strerror(errno));
}

static void family_errors(int family) {
    int level = family == AF_INET ? SOL_IP : SOL_IPV6;
    int option = family == AF_INET ? IP_RECVERR : IPV6_RECVERR;
    printf("family %d\n", family);
    // Without the option an unconnected socket hears nothing.
    int s = udp(family, 5401);
    send_to(s, family, 5402, "lost", 4);
    printf("plain poll %#x\n", revents(s));
    read_errors("plain queue", s, 64, 256, 0);
    int one = 1;
    result("set", setsockopt(s, level, option, &one, sizeof one));
    send_to(s, family, 5402, "first", 5);
    printf("poll %#x\n", revents(s));
    int error = -1;
    socklen_t length = sizeof error;
    getsockopt(s, SOL_SOCKET, SO_ERROR, &error, &length);
    printf("so_error %d\n", error);
    printf("poll after so_error %#x\n", revents(s));
    int queued = -1;
    ioctl(s, FIONREAD, &queued);
    printf("fionread %d\n", queued);
    read_errors("peek", s, 64, 256, MSG_PEEK);
    read_errors("read", s, 64, 256, 0);
    printf("poll after read %#x\n", revents(s));
    read_errors("empty", s, 64, 256, 0);
    // Two errors, then the socket's error, a short buffer and no room for the message.
    send_to(s, family, 5402, "second", 6);
    send_to(s, family, 5403, "third", 5);
    char buf[16];
    long r = recv(s, buf, sizeof buf, MSG_DONTWAIT);
    printf("recv %ld %s\n", r, r < 0 ? strerror(errno) : "");
    r = recv(s, buf, sizeof buf, MSG_DONTWAIT);
    printf("recv again %ld %s\n", r, r < 0 ? strerror(errno) : "");
    printf("poll %#x\n", revents(s));
    read_errors("short", s, 3, 256, 0);
    printf("poll between %#x\n", revents(s));
    send_to(s, family, 5402, "fourth", 6);
    read_errors("no room", s, 64, 16, 0);
    send_to(s, family, 5402, "fifth", 5);
    read_errors("half room", s, 64, 40, 0);
    printf("poll after both %#x\n", revents(s));
    // With packet information asked for too.
    if (family == AF_INET) {
        setsockopt(s, SOL_IP, IP_PKTINFO, &one, sizeof one);
        setsockopt(s, SOL_IP, IP_RECVTTL, &one, sizeof one);
        setsockopt(s, SOL_IP, IP_RECVTOS, &one, sizeof one);
        send_to(s, family, 5402, "info", 4);
        read_errors("with pktinfo", s, 64, 256, 0);
        int zero = 0;
        setsockopt(s, SOL_IP, IP_PKTINFO, &zero, sizeof zero);
        setsockopt(s, SOL_IP, IP_RECVTTL, &zero, sizeof zero);
        setsockopt(s, SOL_IP, IP_RECVTOS, &zero, sizeof zero);
    } else {
        setsockopt(s, SOL_IPV6, IPV6_RECVPKTINFO, &one, sizeof one);
        setsockopt(s, SOL_IPV6, IPV6_RECVHOPLIMIT, &one, sizeof one);
        setsockopt(s, SOL_IPV6, IPV6_RECVTCLASS, &one, sizeof one);
        send_to(s, family, 5402, "info", 4);
        read_errors("with pktinfo", s, 64, 256, 0);
        int zero = 0;
        setsockopt(s, SOL_IPV6, IPV6_RECVPKTINFO, &zero, sizeof zero);
        setsockopt(s, SOL_IPV6, IPV6_RECVHOPLIMIT, &zero, sizeof zero);
        setsockopt(s, SOL_IPV6, IPV6_RECVTCLASS, &zero, sizeof zero);
    }
    // A connected socket hears of it at its next call either way.
    int c = udp(family, 5404);
    setsockopt(c, level, option, &one, sizeof one);
    if (family == AF_INET) {
        struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(5405),
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
        connect(c, (struct sockaddr *)&a, sizeof a);
    } else {
        struct sockaddr_in6 a = {.sin6_family = AF_INET6, .sin6_port = htons(5405),
                                 .sin6_addr = IN6ADDR_LOOPBACK_INIT};
        connect(c, (struct sockaddr *)&a, sizeof a);
    }
    result("connected send", send(c, "x", 1, 0));
    result("connected send again", send(c, "y", 1, 0));
    result("connected send third", send(c, "z", 1, 0));
    read_errors("connected queue", c, 64, 256, 0);
    error = -1;
    getsockopt(c, SOL_SOCKET, SO_ERROR, &error, &length);
    printf("so_error between %d\n", error);
    send(c, "w", 1, 0);
    read_errors("connected queue next", c, 64, 256, 0);
    printf("connected poll mid %#x\n", revents(c));
    read_errors("connected queue last", c, 64, 256, 0);
    printf("connected poll %#x\n", revents(c));
    // Turning the option off empties the queue.
    send_to(s, family, 5402, "gone", 4);
    int zero = 0;
    setsockopt(s, level, option, &zero, sizeof zero);
    printf("off poll %#x\n", revents(s));
    read_errors("off queue", s, 64, 256, 0);
    close(c);
    close(s);
}

int main(void) {
    family_errors(AF_INET);
    family_errors(AF_INET6);
    // A large datagram's error carries only what the error message held of it.
    int s = udp(AF_INET, 5411);
    int one = 1;
    setsockopt(s, SOL_IP, IP_RECVERR, &one, sizeof one);
    static char big[2000];
    memset(big, 'b', sizeof big);
    send_to(s, AF_INET, 5412, big, sizeof big);
    read_errors("big", s, 64, 256, MSG_TRUNC);
    send_to(s, AF_INET, 5412, big, sizeof big);
    read_errors("big whole", s, 4096, 256, 0);
    int s6 = udp(AF_INET6, 5413);
    setsockopt(s6, SOL_IPV6, IPV6_RECVERR, &one, sizeof one);
    send_to(s6, AF_INET6, 5414, big, sizeof big);
    read_errors("big6 whole", s6, 4096, 256, 0);
    // An IPv6 socket sending to an IPv4 address goes by IPv4's option.
    int d = socket(AF_INET6, SOCK_DGRAM, 0);
    struct sockaddr_in6 mapped = {.sin6_family = AF_INET6, .sin6_port = htons(5416)};
    inet_pton(AF_INET6, "::ffff:127.0.0.1", &mapped.sin6_addr);
    setsockopt(d, SOL_IPV6, IPV6_RECVERR, &one, sizeof one);
    sendto(d, "six", 3, 0, (struct sockaddr *)&mapped, sizeof mapped);
    printf("mapped v6 option poll %#x\n", revents(d));
    read_errors("mapped v6 option", d, 64, 256, 0);
    setsockopt(d, SOL_IP, IP_RECVERR, &one, sizeof one);
    setsockopt(d, SOL_IP, IP_RECVTTL, &one, sizeof one);
    setsockopt(d, SOL_IPV6, IPV6_RECVPKTINFO, &one, sizeof one);
    sendto(d, "four", 4, 0, (struct sockaddr *)&mapped, sizeof mapped);
    printf("mapped v4 option poll %#x\n", revents(d));
    read_errors("mapped v4 option", d, 64, 256, 0);
    return 0;
}
