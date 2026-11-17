// The control messages a UDP socket asks for about each datagram (IP_PKTINFO, IP_RECVTTL,
// IP_RECVTOS, IPV6_RECVPKTINFO, IPV6_RECVHOPLIMIT, IPV6_RECVTCLASS), over IPv4, IPv6 and IPv4 to
// an IPv6 socket, with too little room for them, and the options read back.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <ifaddrs.h>
#include <net/if.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static struct in_addr eth0;

// An interface's number by its name, which Docker chooses for eth0.
static const char *interface(int index) {
    static char name[IF_NAMESIZE];
    return if_indextoname(index, name) ? name : "none";
}

static void show(const char *what, int s, size_t room) {
    char data[64];
    char control[512];
    struct sockaddr_storage from;
    struct iovec iov = {data, sizeof data};
    struct msghdr m = {.msg_name = &from,
                       .msg_namelen = sizeof from,
                       .msg_iov = &iov,
                       .msg_iovlen = 1,
                       .msg_control = control,
                       .msg_controllen = room};
    long n = recvmsg(s, &m, MSG_DONTWAIT);
    printf("%s: %ld flags %#x controllen %zu\n", what, n, m.msg_flags, (size_t)m.msg_controllen);
    if (n < 0)
        return;
    for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
        printf(" level %d type %d len %zu:", c->cmsg_level, c->cmsg_type, (size_t)c->cmsg_len);
        unsigned char *d = CMSG_DATA(c);
        size_t length = c->cmsg_len - CMSG_LEN(0);
        if (c->cmsg_level == IPPROTO_IP && c->cmsg_type == IP_PKTINFO &&
            length >= sizeof(struct in_pktinfo)) {
            struct in_pktinfo *p = (void *)d;
            printf(" ifindex %s spec_dst %s", interface(p->ipi_ifindex),
                   p->ipi_spec_dst.s_addr == eth0.s_addr ? "eth0" : inet_ntoa(p->ipi_spec_dst));
            printf(" addr %s\n", p->ipi_addr.s_addr == eth0.s_addr ? "eth0" : inet_ntoa(p->ipi_addr));
            continue;
        }
        if (c->cmsg_level == IPPROTO_IPV6 && c->cmsg_type == IPV6_PKTINFO) {
            struct in6_pktinfo *p = (void *)d;
            char text[64];
            inet_ntop(AF_INET6, &p->ipi6_addr, text, sizeof text);
            int mapped_eth0 = !memcmp(&p->ipi6_addr.s6_addr[12], &eth0, 4);
            printf(" addr %s ifindex %s\n", mapped_eth0 ? "mapped eth0" : text,
                   interface(p->ipi6_ifindex));
            continue;
        }
        if (c->cmsg_level == IPPROTO_IP && c->cmsg_type == IP_PKTINFO) {
            // Cut short: the interface (whose number Docker chooses) and what is left.
            printf(" ifindex %s", interface(*(int *)d));
            for (size_t i = 4; i < length; i++)
                printf(" %02x", d[i]);
            printf("\n");
            continue;
        }
        for (size_t i = 0; i < length; i++)
            printf(" %02x", d[i]);
        printf("\n");
    }
}

static void option(const char *what, int s, int level, int name) {
    int value = -1;
    socklen_t length = sizeof value;
    long r = getsockopt(s, level, name, &value, &length);
    printf("%s: %ld value %d length %u\n", what, r, value, length);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    struct ifaddrs *all;
    getifaddrs(&all);
    for (struct ifaddrs *i = all; i; i = i->ifa_next)
        if (i->ifa_addr && i->ifa_addr->sa_family == AF_INET && !strcmp(i->ifa_name, "eth0"))
            eth0 = ((struct sockaddr_in *)i->ifa_addr)->sin_addr;
    int one = 1;
    int r4 = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(9100)};
    bind(r4, (void *)&a, sizeof a);
    option("pktinfo before", r4, IPPROTO_IP, IP_PKTINFO);
    printf("pktinfo %d\n", setsockopt(r4, IPPROTO_IP, IP_PKTINFO, &one, sizeof one));
    printf("recvttl %d\n", setsockopt(r4, IPPROTO_IP, IP_RECVTTL, &one, sizeof one));
    printf("recvtos %d\n", setsockopt(r4, IPPROTO_IP, IP_RECVTOS, &one, sizeof one));
    option("pktinfo after", r4, IPPROTO_IP, IP_PKTINFO);
    option("recvttl after", r4, IPPROTO_IP, IP_RECVTTL);
    option("recvtos after", r4, IPPROTO_IP, IP_RECVTOS);
    int s = socket(AF_INET, SOCK_DGRAM, 0);
    int tos = 0x28, ttl = 9;
    setsockopt(s, IPPROTO_IP, IP_TOS, &tos, sizeof tos);
    setsockopt(s, IPPROTO_IP, IP_TTL, &ttl, sizeof ttl);
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    sendto(s, "one", 3, 0, (void *)&a, sizeof a);
    a.sin_addr = eth0;
    sendto(s, "two", 3, 0, (void *)&a, sizeof a);
    a.sin_addr.s_addr = htonl(0x7f000005);
    sendto(s, "three", 5, 0, (void *)&a, sizeof a);
    show("loopback", r4, 512);
    show("eth0", r4, 512);
    show("small room", r4, 24);
    int plain = socket(AF_INET, SOCK_DGRAM, 0);
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    sendto(plain, "four", 4, 0, (void *)&a, sizeof a);
    show("default ttl", r4, 512);
    one = 0;
    setsockopt(r4, IPPROTO_IP, IP_RECVTTL, &one, sizeof one);
    sendto(plain, "five", 4, 0, (void *)&a, sizeof a);
    show("ttl off", r4, 512);

    int r6 = socket(AF_INET6, SOCK_DGRAM, 0);
    struct sockaddr_in6 b = {.sin6_family = AF_INET6, .sin6_port = htons(9101)};
    bind(r6, (void *)&b, sizeof b);
    one = 1;
    printf("recvpktinfo %d\n", setsockopt(r6, IPPROTO_IPV6, IPV6_RECVPKTINFO, &one, sizeof one));
    printf("recvhoplimit %d\n", setsockopt(r6, IPPROTO_IPV6, IPV6_RECVHOPLIMIT, &one, sizeof one));
    printf("recvtclass %d\n", setsockopt(r6, IPPROTO_IPV6, IPV6_RECVTCLASS, &one, sizeof one));
    option("recvpktinfo after", r6, IPPROTO_IPV6, IPV6_RECVPKTINFO);
    int s6 = socket(AF_INET6, SOCK_DGRAM, 0);
    int hops = 5, tclass = 0x40;
    setsockopt(s6, IPPROTO_IPV6, IPV6_UNICAST_HOPS, &hops, sizeof hops);
    setsockopt(s6, IPPROTO_IPV6, IPV6_TCLASS, &tclass, sizeof tclass);
    b.sin6_addr = in6addr_loopback;
    sendto(s6, "six", 3, 0, (void *)&b, sizeof b);
    show("ipv6 loopback", r6, 512);
    struct sockaddr_in to6 = {.sin_family = AF_INET, .sin_port = htons(9101)};
    to6.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    sendto(s, "seven", 5, 0, (void *)&to6, sizeof to6);
    show("ipv4 to ipv6", r6, 512);
    printf("v4 pktinfo on v6 %d\n", setsockopt(r6, IPPROTO_IP, IP_PKTINFO, &one, sizeof one));
    sendto(s, "eight", 5, 0, (void *)&to6, sizeof to6);
    show("ipv4 to ipv6 with ip options", r6, 512);
    // Packet information is kept for a datagram only if it was asked for when the datagram came.
    one = 0;
    setsockopt(r4, IPPROTO_IP, IP_PKTINFO, &one, sizeof one);
    setsockopt(r6, IPPROTO_IP, IP_PKTINFO, &one, sizeof one);
    setsockopt(r6, IPPROTO_IPV6, IPV6_RECVPKTINFO, &one, sizeof one);
    a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    sendto(plain, "late", 4, 0, (void *)&a, sizeof a);
    sendto(s, "late6", 5, 0, (void *)&to6, sizeof to6);
    sendto(s6, "late66", 6, 0, (void *)&b, sizeof b);
    one = 1;
    setsockopt(r4, IPPROTO_IP, IP_PKTINFO, &one, sizeof one);
    setsockopt(r6, IPPROTO_IPV6, IPV6_RECVPKTINFO, &one, sizeof one);
    show("asked late", r4, 512);
    show("asked late ipv4 to ipv6", r6, 512);
    show("asked late ipv6", r6, 512);
    return 0;
}
