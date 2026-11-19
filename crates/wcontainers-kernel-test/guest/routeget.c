// Questions about one route (`ip route get`) and the rule dump (`ip rule`), as netlink answers
// them: every message's header, the route or rule's fixed part and its attributes. eth0's own
// address, which Docker chooses per container, is shown as 172.17.0.x.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <linux/fib_rules.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include <net/if.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static unsigned char own[4];

static void address(int family, const unsigned char *bytes, int length) {
    char out[64];
    if (family == AF_INET && length == 4) {
        if (!memcmp(bytes, own, 4))
            printf("172.17.0.x");
        else
            printf("%s", inet_ntop(AF_INET, bytes, out, sizeof out));
    } else if (family == AF_INET6 && length == 16) {
        printf("%s", inet_ntop(AF_INET6, bytes, out, sizeof out));
    } else {
        for (int i = 0; i < length; i++)
            printf("%02x", bytes[i]);
    }
}

static void print(int family, struct rtattr *a, int left) {
    for (; RTA_OK(a, left); a = RTA_NEXT(a, left)) {
        int length = RTA_PAYLOAD(a);
        unsigned char *data = RTA_DATA(a);
        printf("   attribute %u length %d: ", a->rta_type, length);
        if (length == 4 && family == AF_INET && (a->rta_type == RTA_DST || a->rta_type == RTA_SRC ||
                                                 a->rta_type == RTA_PREFSRC || a->rta_type == RTA_GATEWAY))
            address(family, data, length);
        else if (length == 16 && family == AF_INET6)
            address(family, data, length);
        // Its first word is the route's reference count, which varies on Linux.
        else if (a->rta_type == RTA_CACHEINFO && length == 32)
            printf("(cache information)");
        else if (a->rta_type == RTA_OIF && length == 4 && family != 0) {
            char name[IF_NAMESIZE] = "?";
            if_indextoname(*(unsigned *)data, name);
            printf("%s", name);
        }
        else
            for (int i = 0; i < length; i++)
                printf("%02x", data[i]);
        printf("\n");
    }
}

// Reads answers until NLMSG_DONE, an error or a lone answer.
static void answers(int fd) {
    static char buf[65536];
    for (;;) {
        int n = recv(fd, buf, sizeof buf, 0);
        if (n < 0) {
            perror("recv");
            return;
        }
        printf(" a read\n");
        int more = 0;
        for (struct nlmsghdr *h = (void *)buf; NLMSG_OK(h, n); h = NLMSG_NEXT(h, n)) {
            printf(" message type %u flags %#x seq %u length %u\n", h->nlmsg_type, h->nlmsg_flags,
                   h->nlmsg_seq, h->nlmsg_len);
            if (h->nlmsg_type == NLMSG_DONE)
                return;
            if (h->nlmsg_type == NLMSG_ERROR) {
                struct nlmsgerr *e = NLMSG_DATA(h);
                printf("  error %d\n", e->error);
                return;
            }
            more |= h->nlmsg_flags & NLM_F_MULTI;
            struct rtmsg *r = NLMSG_DATA(h);
            printf("  family %u dst/%u src/%u tos %u table %u protocol %u scope %u type %u flags %#x\n",
                   r->rtm_family, r->rtm_dst_len, r->rtm_src_len, r->rtm_tos, r->rtm_table,
                   r->rtm_protocol, r->rtm_scope, r->rtm_type, r->rtm_flags);
            print(r->rtm_family, RTM_RTA(r), RTM_PAYLOAD(h));
        }
        if (!more)
            return;
    }
}

static void get(int fd, int family, const char *to, unsigned flags, int oif) {
    struct {
        struct nlmsghdr h;
        struct rtmsg r;
        char attributes[64];
    } req = {.h = {.nlmsg_type = RTM_GETROUTE, .nlmsg_flags = NLM_F_REQUEST, .nlmsg_seq = 7},
             .r = {.rtm_family = family, .rtm_flags = flags}};
    int length = NLMSG_LENGTH(sizeof(struct rtmsg));
    struct rtattr *a = (void *)((char *)&req + NLMSG_ALIGN(length));
    unsigned char bytes[16];
    int size = family == AF_INET ? 4 : 16;
    inet_pton(family, to, bytes);
    if (!strcmp(to, "own"))
        memcpy(bytes, own, 4);
    a->rta_type = RTA_DST;
    a->rta_len = RTA_LENGTH(size);
    memcpy(RTA_DATA(a), bytes, size);
    length = NLMSG_ALIGN(length) + RTA_ALIGN(a->rta_len);
    req.r.rtm_dst_len = size * 8;
    if (oif) {
        a = (void *)((char *)&req + length);
        a->rta_type = RTA_OIF;
        a->rta_len = RTA_LENGTH(4);
        memcpy(RTA_DATA(a), &oif, 4);
        length += RTA_ALIGN(a->rta_len);
    }
    req.h.nlmsg_len = length;
    printf("route to %s family %d flags %#x oif %d\n", strcmp(to, "own") ? to : "172.17.0.x", family, flags, oif);
    send(fd, &req, length, 0);
    answers(fd);
}

// A question with the header's other fields and one more attribute chosen, for the checks
// NETLINK_GET_STRICT_CHK turns on.
static void get_checked(int fd, int family, const char *to, int dst_len, int table, unsigned flags,
                        int extra) {
    struct {
        struct nlmsghdr h;
        struct rtmsg r;
        char attributes[64];
    } req = {.h = {.nlmsg_type = RTM_GETROUTE, .nlmsg_flags = NLM_F_REQUEST, .nlmsg_seq = 8},
             .r = {.rtm_family = family, .rtm_dst_len = dst_len, .rtm_table = table, .rtm_flags = flags}};
    int length = NLMSG_ALIGN(NLMSG_LENGTH(sizeof(struct rtmsg)));
    struct rtattr *a = (void *)((char *)&req + length);
    int size = family == AF_INET ? 4 : 16;
    a->rta_type = RTA_DST;
    a->rta_len = RTA_LENGTH(size);
    inet_pton(family, to, RTA_DATA(a));
    length += RTA_ALIGN(a->rta_len);
    if (extra) {
        a = (void *)((char *)&req + length);
        a->rta_type = extra;
        a->rta_len = RTA_LENGTH(4);
        memset(RTA_DATA(a), 0, 4);
        length += RTA_ALIGN(a->rta_len);
    }
    req.h.nlmsg_len = length;
    printf("checked route to %s dst/%d table %d flags %#x extra %d\n", to, dst_len, table, flags, extra);
    send(fd, &req, length, 0);
    answers(fd);
}

static void checks(int fd) {
    get_checked(fd, AF_INET, "127.0.0.1", 16, 0, 0, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 0, 0, 0, 0);
    int on = 1;
    printf("strict %d\n", setsockopt(fd, SOL_NETLINK, NETLINK_GET_STRICT_CHK, &on, sizeof on));
    get_checked(fd, AF_INET, "127.0.0.1", 32, 0, 0, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 16, 0, 0, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 0, 0, 0, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 32, 254, 0, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 32, 0, RTM_F_LOOKUP_TABLE, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 32, 0, RTM_F_PREFIX, 0);
    get_checked(fd, AF_INET, "127.0.0.1", 32, 0, 0, RTA_MARK);
    get_checked(fd, AF_INET, "127.0.0.1", 32, 0, 0, RTA_PRIORITY);
    get_checked(fd, AF_INET6, "::1", 128, 0, 0, 0);
    get_checked(fd, AF_INET6, "::1", 64, 0, 0, 0);
    get_checked(fd, AF_INET6, "::1", 0, 0, 0, 0);
    get_checked(fd, AF_INET6, "::1", 128, 0, RTM_F_PREFIX, 0);
    get_checked(fd, AF_INET6, "::1", 128, 0, 0, RTA_PRIORITY);
    get_checked(fd, AF_INET6, "::1", 128, 0, 0, RTA_MARK);
}

static void rules(int fd, int family) {
    struct {
        struct nlmsghdr h;
        struct fib_rule_hdr r;
    } req = {.h = {.nlmsg_len = sizeof req, .nlmsg_type = RTM_GETRULE, .nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP, .nlmsg_seq = 9},
             .r = {.family = family}};
    printf("rules family %d\n", family);
    send(fd, &req, sizeof req, 0);
    answers(fd);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    // eth0's own address, from a socket connected through it.
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in out = {.sin_family = AF_INET, .sin_port = htons(53)};
    inet_pton(AF_INET, "1.1.1.1", &out.sin_addr);
    connect(u, (void *)&out, sizeof out);
    socklen_t l = sizeof out;
    getsockname(u, (void *)&out, &l);
    memcpy(own, &out.sin_addr, 4);
    int fd = socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE);
    get(fd, AF_INET, "127.0.0.1", 0, 0);
    get(fd, AF_INET, "127.3.4.5", 0, 0);
    get(fd, AF_INET, "1.1.1.1", 0, 0);
    get(fd, AF_INET, "own", 0, 0);
    get(fd, AF_INET, "172.17.0.1", 0, 0);
    get(fd, AF_INET, "172.17.9.9", 0, 0);
    get(fd, AF_INET, "172.17.255.255", 0, 0);
    get(fd, AF_INET, "224.0.0.1", 0, 0);
    get(fd, AF_INET, "239.1.2.3", 0, 0);
    get(fd, AF_INET, "127.255.255.255", 0, 0);
    get(fd, AF_INET, "172.17.0.0", 0, 0);
    get(fd, AF_INET, "172.18.0.1", 0, 0);
    get(fd, AF_INET, "127.255.255.255", 0, 1);
    get(fd, AF_INET, "255.255.255.255", 0, 0);
    get(fd, AF_INET, "0.0.0.0", 0, 0);
    get(fd, AF_INET, "1.1.1.1", RTM_F_FIB_MATCH, 0);
    get(fd, AF_INET, "127.0.0.1", RTM_F_FIB_MATCH, 0);
    get(fd, AF_INET, "1.1.1.1", 0, 1);
    get(fd, AF_INET6, "::1", 0, 0);
    get(fd, AF_INET6, "2001:db8::1", 0, 0);
    rules(fd, AF_INET);
    rules(fd, AF_INET6);
    rules(fd, AF_UNSPEC);
    checks(fd);
    return 0;
}
