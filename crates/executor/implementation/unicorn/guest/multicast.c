// IPv4 multicast and broadcast between the container's own sockets: memberships, the interface
// multicast leaves by, looping back, IP_MULTICAST_ALL, and SO_BROADCAST.
#define _GNU_SOURCE
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <net/if.h>
#include <sys/ioctl.h>
static int A, B, C, D;
static struct in_addr eth;
static void drain(const char *who, int fd) {
    char buf[100], name[32]; socklen_t nl = sizeof name;
    for (;;) {
        char ctl[256];
        struct iovec v = {buf, sizeof buf};
        struct msghdr m = {.msg_name = name, .msg_namelen = nl, .msg_iov = &v, .msg_iovlen = 1, .msg_control = ctl, .msg_controllen = sizeof ctl};
        int n = recvmsg(fd, &m, MSG_DONTWAIT);
        if (n < 0) return;
        struct in_addr from = ((struct sockaddr_in *)name)->sin_addr;
        char a[20];
        inet_ntop(AF_INET, &from, a, 20);
        printf("  %s got %d from %s", who, n, from.s_addr == eth.s_addr ? "eth0's" : a);
        for (struct cmsghdr *c = CMSG_FIRSTHDR(&m); c; c = CMSG_NXTHDR(&m, c)) {
            if (c->cmsg_type == IP_TTL) { int t; memcpy(&t, CMSG_DATA(c), 4); printf(" ttl %d", t); }
            if (c->cmsg_type == IP_PKTINFO) { struct in_pktinfo p; memcpy(&p, CMSG_DATA(c), sizeof p); inet_ntop(AF_INET, &p.ipi_addr, a, 20); printf(" to %s on %s", a, p.ipi_ifindex == 1 ? "lo" : p.ipi_ifindex == (int)if_nametoindex("eth0") ? "eth0" : "?"); }
        }
        printf("\n");
    }
}
static void all(void) { drain("A any joined", A); drain("B any", B); drain("C group joined", C); drain("D any no mcall", D); }
static void to(const char *what, int s, const char *addr, int port) {
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port)}; inet_pton(AF_INET, addr, &a.sin_addr);
    int r = sendto(s, "hi", 2, 0, (void *)&a, sizeof a);
    printf("%s: %d %s\n", what, r, r < 0 ? strerror(errno) : ""); usleep(20000); all();
}
static int bound(const char *addr, int port) {
    int s = socket(AF_INET, SOCK_DGRAM, 0); int on = 1; setsockopt(s, SOL_SOCKET, SO_REUSEADDR, &on, 4);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port)}; inet_pton(AF_INET, addr, &a.sin_addr);
    if (bind(s, (void *)&a, sizeof a)) printf("bind %s: %s\n", addr, strerror(errno)); return s;
}
int main(void) {
    A = bound("0.0.0.0", 7700); B = bound("0.0.0.0", 7700); C = bound("224.1.1.1", 7700); D = bound("0.0.0.0", 7700);
    int off = 0; setsockopt(D, IPPROTO_IP, IP_MULTICAST_ALL, &off, 4);
    int yes = 1;
    setsockopt(A, IPPROTO_IP, IP_RECVTTL, &yes, 4);
    setsockopt(A, IPPROTO_IP, IP_PKTINFO, &yes, 4);
    { int t = socket(AF_INET, SOCK_DGRAM, 0); struct ifreq ifr = {0}; strcpy(ifr.ifr_name, "eth0"); ioctl(t, SIOCGIFADDR, &ifr); eth = ((struct sockaddr_in *)&ifr.ifr_addr)->sin_addr; close(t); }
    int s = socket(AF_INET, SOCK_DGRAM, 0);
    to("before joining", s, "224.1.1.1", 7700);
    struct ip_mreq m = {0}; inet_pton(AF_INET, "224.1.1.1", &m.imr_multiaddr);
    printf("join A: %d\n", setsockopt(A, IPPROTO_IP, IP_ADD_MEMBERSHIP, &m, sizeof m));
    printf("join A again: %d %s\n", setsockopt(A, IPPROTO_IP, IP_ADD_MEMBERSHIP, &m, sizeof m), strerror(errno));
    printf("join C: %d\n", setsockopt(C, IPPROTO_IP, IP_ADD_MEMBERSHIP, &m, sizeof m));
    to("after joining", s, "224.1.1.1", 7700);
    to("other group", s, "224.1.1.2", 7700);
    int loop = 0; setsockopt(s, IPPROTO_IP, IP_MULTICAST_LOOP, &loop, 4);
    to("no loop", s, "224.1.1.1", 7700);
    loop = 1; setsockopt(s, IPPROTO_IP, IP_MULTICAST_LOOP, &loop, 4);
    struct in_addr lo; inet_pton(AF_INET, "127.0.0.1", &lo);
    printf("multicast_if lo: %d\n", setsockopt(s, IPPROTO_IP, IP_MULTICAST_IF, &lo, sizeof lo));
    to("through lo", s, "224.1.1.1", 7700);
    struct ip_mreqn mn = {0}; mn.imr_multiaddr = m.imr_multiaddr; mn.imr_ifindex = 1;
    printf("join B on lo: %d\n", setsockopt(B, IPPROTO_IP, IP_ADD_MEMBERSHIP, &mn, sizeof mn));
    to("through lo, B joined on lo", s, "224.1.1.1", 7700);
    printf("drop A: %d\n", setsockopt(A, IPPROTO_IP, IP_DROP_MEMBERSHIP, &m, sizeof m));
    printf("drop A again: %d %s\n", setsockopt(A, IPPROTO_IP, IP_DROP_MEMBERSHIP, &m, sizeof m), strerror(errno));
    struct in_addr none = {0};
    setsockopt(s, IPPROTO_IP, IP_MULTICAST_IF, &none, sizeof none);
    to("after A dropped", s, "224.1.1.1", 7700);
    printf("join unicast: %d %s\n", (inet_pton(AF_INET, "10.0.0.1", &m.imr_multiaddr), setsockopt(A, IPPROTO_IP, IP_ADD_MEMBERSHIP, &m, sizeof m)), strerror(errno));
    printf("join short: %d %s\n", setsockopt(A, IPPROTO_IP, IP_ADD_MEMBERSHIP, &m, 4), strerror(errno));
    to("broadcast without so_broadcast", s, "255.255.255.255", 7700);
    int on = 1; setsockopt(s, SOL_SOCKET, SO_BROADCAST, &on, 4);
    to("broadcast", s, "255.255.255.255", 7700);
    to("subnet broadcast", s, "172.17.255.255", 7700);
    int ttl = 3; setsockopt(s, IPPROTO_IP, IP_MULTICAST_TTL, &ttl, 4);
    to("multicast ttl 3", s, "224.1.1.1", 7700);
    int t = socket(AF_INET, SOCK_STREAM, 0);
    printf("tcp join: %d %s\n", setsockopt(t, IPPROTO_IP, IP_ADD_MEMBERSHIP, &m, sizeof m), strerror(errno));
    printf("tcp multicast_if: %d %s\n", setsockopt(t, IPPROTO_IP, IP_MULTICAST_IF, &lo, sizeof lo), strerror(errno));
    int many = socket(AF_INET, SOCK_DGRAM, 0);
    for (int i = 0; i < 21; i++) {
        struct ip_mreq g2 = {0}; g2.imr_multiaddr.s_addr = htonl(0xe0020000 + i);
        int r = setsockopt(many, IPPROTO_IP, IP_ADD_MEMBERSHIP, &g2, sizeof g2);
        if (r) printf("join %d: %s\n", i, strerror(errno));
    }
    struct group_req gr = {.gr_interface = 0};
    struct sockaddr_in *ga = (void *)&gr.gr_group; ga->sin_family = AF_INET; inet_pton(AF_INET, "224.3.3.3", &ga->sin_addr);
    printf("mcast_join_group: %d %s\n", setsockopt(D, IPPROTO_IP, MCAST_JOIN_GROUP, &gr, sizeof gr), strerror(errno));
    to("to 224.3.3.3", s, "224.3.3.3", 7700);
    printf("mcast_leave_group: %d %s\n", setsockopt(D, IPPROTO_IP, MCAST_LEAVE_GROUP, &gr, sizeof gr), strerror(errno));
    to("to 224.3.3.3 again", s, "224.3.3.3", 7700);
    struct in_addr far; inet_pton(AF_INET, "10.1.1.1", &far);
    printf("multicast_if foreign: %d %s\n", setsockopt(s, IPPROTO_IP, IP_MULTICAST_IF, &far, sizeof far), strerror(errno));
    struct ip_mreqn bad = {0}; bad.imr_ifindex = 99;
    printf("multicast_if index 99: %d %s\n", setsockopt(s, IPPROTO_IP, IP_MULTICAST_IF, &bad, sizeof bad), strerror(errno));
    setsockopt(s, IPPROTO_IP, IP_MULTICAST_IF, &lo, sizeof lo);
    unsigned char g[8]; socklen_t gl = 8; memset(g, 0, 8);
    getsockopt(s, IPPROTO_IP, IP_MULTICAST_IF, g, &gl); printf("multicast_if get len %u %d.%d.%d.%d\n", gl, g[0], g[1], g[2], g[3]);
    // A connection's socket: the ip_mreq forms are refused, the group_req ones taken, and an
    // accepted socket does not inherit its listener's groups.
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    struct ip_mreq tcp_group = {0}; tcp_group.imr_multiaddr = ga->sin_addr;
    printf("tcp add_membership: %d %s\n", setsockopt(listener, IPPROTO_IP, IP_ADD_MEMBERSHIP, &tcp_group, sizeof tcp_group), strerror(errno));
    printf("tcp mcast_join_group: %d %s\n", setsockopt(listener, IPPROTO_IP, MCAST_JOIN_GROUP, &gr, sizeof gr), strerror(errno));
    struct sockaddr_in here = {.sin_family = AF_INET}; inet_pton(AF_INET, "127.0.0.1", &here.sin_addr);
    socklen_t hl = sizeof here;
    bind(listener, (void *)&here, sizeof here); getsockname(listener, (void *)&here, &hl); listen(listener, 1);
    int client = socket(AF_INET, SOCK_STREAM, 0);
    connect(client, (void *)&here, sizeof here);
    int accepted = accept(listener, NULL, NULL);
    errno = 0;
    printf("accepted mcast_leave_group: %d %s\n", setsockopt(accepted, IPPROTO_IP, MCAST_LEAVE_GROUP, &gr, sizeof gr), strerror(errno));
    errno = 0;
    printf("listener mcast_leave_group: %d %s\n", setsockopt(listener, IPPROTO_IP, MCAST_LEAVE_GROUP, &gr, sizeof gr), strerror(errno));
    fflush(stdout);
    return 0;
}
