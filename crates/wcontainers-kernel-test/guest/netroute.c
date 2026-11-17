// The container's interfaces, addresses and routes as programs ask for them: getifaddrs and
// if_nameindex, netlink's route dumps and requests, and the interface ioctls. Only lo and eth0
// are shown (the reference's kernel adds tunnel devices), and the parts Docker chooses per
// container (eth0's number, the last byte of its address, its hardware address) are hidden.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <ifaddrs.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include <net/if.h>
#include <netinet/in.h>
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

static int shown(const char *name) { return !strcmp(name, "lo") || !strcmp(name, "eth0"); }

// An address with the container's own last byte hidden.
static const char *text(int family, const void *address) {
    static char out[64];
    inet_ntop(family, address, out, sizeof out);
    if (!strncmp(out, "172.17.0.", 9) && strcmp(out, "172.17.0.1") != 0)
        strcpy(out, "172.17.0.x");
    return out;
}

static const char *interface(int index) {
    static char name[IF_NAMESIZE];
    if (!if_indextoname(index, name))
        return "?";
    return name;
}

static void addresses(void) {
    struct ifaddrs *list;
    result("getifaddrs", getifaddrs(&list));
    for (struct ifaddrs *a = list; a; a = a->ifa_next) {
        if (!shown(a->ifa_name))
            continue;
        int family = a->ifa_addr ? a->ifa_addr->sa_family : -1;
        printf("ifaddr %s flags %#x family %d", a->ifa_name, a->ifa_flags, family);
        if (family == AF_INET) {
            printf(" %s", text(AF_INET, &((struct sockaddr_in *)a->ifa_addr)->sin_addr));
            printf(" mask %s", text(AF_INET, &((struct sockaddr_in *)a->ifa_netmask)->sin_addr));
            if (a->ifa_broadaddr && (a->ifa_flags & IFF_BROADCAST))
                printf(" broadcast %s",
                       text(AF_INET, &((struct sockaddr_in *)a->ifa_broadaddr)->sin_addr));
        } else if (family == AF_INET6) {
            printf(" %s", text(AF_INET6, &((struct sockaddr_in6 *)a->ifa_addr)->sin6_addr));
        }
        printf("\n");
    }
    freeifaddrs(list);
    struct if_nameindex *names = if_nameindex();
    for (struct if_nameindex *n = names; n && n->if_index; n++)
        if (shown(n->if_name))
            printf("nameindex %s %s\n", n->if_name, !strcmp(n->if_name, "lo") ? "1" : "?");
    if_freenameindex(names);
    printf("lo index %u, nothing %u\n", if_nametoindex("lo"), if_nametoindex("nothing"));
}

static int request(int fd, int type, int flags, int family, int seq) {
    struct {
        struct nlmsghdr h;
        struct ifinfomsg i;
    } req = {.h = {.nlmsg_len = sizeof req, .nlmsg_type = type, .nlmsg_flags = flags, .nlmsg_seq = seq},
             .i = {.ifi_family = family}};
    return send(fd, &req, sizeof req, 0);
}

// Reads one answer; prints each message, and returns 0 at NLMSG_DONE or an error.
static int answers(int fd, int type) {
    static char buf[65536];
    int n = recv(fd, buf, sizeof buf, 0);
    if (n < 0) {
        result("recv", n);
        return 0;
    }
    for (struct nlmsghdr *h = (void *)buf; NLMSG_OK(h, n); h = NLMSG_NEXT(h, n)) {
        if (h->nlmsg_type == RTM_NEWLINK) {
            struct ifinfomsg *i = NLMSG_DATA(h);
            struct rtattr *b = IFLA_RTA(i);
            int left = IFLA_PAYLOAD(h);
            int show = 0;
            for (; RTA_OK(b, left); b = RTA_NEXT(b, left))
                if (b->rta_type == IFLA_IFNAME)
                    show = shown(RTA_DATA(b));
            if (!show)
                continue;
        }
        printf(" message type %u flags %#x seq %u for us %d\n", h->nlmsg_type, h->nlmsg_flags,
               h->nlmsg_seq, h->nlmsg_pid == (unsigned)getpid());
        if (h->nlmsg_type == NLMSG_DONE)
            return 0;
        if (h->nlmsg_type == NLMSG_ERROR) {
            struct nlmsgerr *e = NLMSG_DATA(h);
            printf("  error %d of type %u, length %u\n", e->error, e->msg.nlmsg_type, h->nlmsg_len);
            return 0;
        }
        if (type == RTM_GETLINK) {
            struct ifinfomsg *i = NLMSG_DATA(h);
            struct rtattr *a = IFLA_RTA(i);
            int left = IFLA_PAYLOAD(h);
            char name[32] = "";
            for (struct rtattr *b = a; RTA_OK(b, left); b = RTA_NEXT(b, left))
                if (b->rta_type == IFLA_IFNAME)
                    strcpy(name, RTA_DATA(b));
            if (!shown(name))
                continue;
            printf("  link %s type %u flags %#x\n", name, i->ifi_type, i->ifi_flags);
            left = IFLA_PAYLOAD(h);
            for (; RTA_OK(a, left); a = RTA_NEXT(a, left)) {
                switch (a->rta_type) {
                case IFLA_MTU:
                case IFLA_TXQLEN:
                case IFLA_MIN_MTU:
                case IFLA_MAX_MTU:
                case IFLA_GROUP:
                case IFLA_PROMISCUITY:
                    printf("   attribute %u: %u\n", a->rta_type, *(unsigned *)RTA_DATA(a));
                    break;
                case IFLA_OPERSTATE:
                case IFLA_LINKMODE:
                case IFLA_CARRIER:
                    printf("   attribute %u: %u\n", a->rta_type, *(unsigned char *)RTA_DATA(a));
                    break;
                case IFLA_QDISC:
                    printf("   attribute %u: %s\n", a->rta_type, (char *)RTA_DATA(a));
                    break;
                case IFLA_ADDRESS:
                case IFLA_BROADCAST:
                    printf("   attribute %u: %d bytes\n", a->rta_type, RTA_PAYLOAD(a));
                    break;
                }
            }
        } else if (type == RTM_GETADDR) {
            struct ifaddrmsg *i = NLMSG_DATA(h);
            printf("  address family %u prefix %u flags %#x scope %u on %s attributes", i->ifa_family,
                   i->ifa_prefixlen, i->ifa_flags, i->ifa_scope, interface(i->ifa_index));
            struct rtattr *a = IFA_RTA(i);
            int left = IFA_PAYLOAD(h);
            for (; RTA_OK(a, left); a = RTA_NEXT(a, left)) {
                printf(" %u", a->rta_type);
                if (a->rta_type == IFA_ADDRESS || a->rta_type == IFA_BROADCAST)
                    printf("=%s", text(i->ifa_family, RTA_DATA(a)));
                if (a->rta_type == IFA_LABEL)
                    printf("=%s", (char *)RTA_DATA(a));
            }
            printf("\n");
        } else {
            struct rtmsg *r = NLMSG_DATA(h);
            printf("  route family %u to /%u table %u protocol %u scope %u type %u attributes",
                   r->rtm_family, r->rtm_dst_len, r->rtm_table, r->rtm_protocol, r->rtm_scope,
                   r->rtm_type);
            struct rtattr *a = RTM_RTA(r);
            int left = RTM_PAYLOAD(h);
            for (; RTA_OK(a, left); a = RTA_NEXT(a, left)) {
                printf(" %u", a->rta_type);
                if (a->rta_type == RTA_DST || a->rta_type == RTA_GATEWAY || a->rta_type == RTA_PREFSRC)
                    printf("=%s", text(r->rtm_family, RTA_DATA(a)));
                if (a->rta_type == RTA_OIF)
                    printf("=%s", interface(*(int *)RTA_DATA(a)));
            }
            printf("\n");
        }
    }
    return 1;
}

static void dump(int fd, int type, int family) {
    printf("dump %d family %d\n", type, family);
    result(" send", request(fd, type, NLM_F_REQUEST | NLM_F_DUMP, family, 42) > 0);
    while (answers(fd, type))
        ;
}

static void netlink(void) {
    int fd = socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
    struct sockaddr_nl a = {.nl_family = AF_NETLINK};
    socklen_t length = sizeof a;
    getsockname(fd, (void *)&a, &length);
    printf("unbound pid %u length %d\n", a.nl_pid, length);
    result("bind", bind(fd, (void *)&a, sizeof a));
    length = sizeof a;
    getsockname(fd, (void *)&a, &length);
    printf("bound to own pid %d\n", a.nl_pid == (unsigned)getpid());
    int other = socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE);
    struct sockaddr_nl same = {.nl_family = AF_NETLINK, .nl_pid = getpid()};
    result("bind taken", bind(other, (void *)&same, sizeof same));
    result("bind short", bind(other, (void *)&same, 8));
    result("listen", listen(fd, 1));
    result("other protocol", socket(AF_NETLINK, SOCK_RAW, 33));
    result("stream", socket(AF_NETLINK, SOCK_STREAM, NETLINK_ROUTE));
    dump(fd, RTM_GETLINK, AF_UNSPEC);
    dump(fd, RTM_GETADDR, AF_UNSPEC);
    dump(fd, RTM_GETADDR, AF_INET6);
    dump(fd, RTM_GETROUTE, AF_INET);
    dump(fd, RTM_GETROUTE, AF_INET6);
    printf("one link with ack\n");
    struct {
        struct nlmsghdr h;
        struct ifinfomsg i;
    } one = {.h = {.nlmsg_len = sizeof one, .nlmsg_type = RTM_GETLINK,
                   .nlmsg_flags = NLM_F_REQUEST | NLM_F_ACK, .nlmsg_seq = 5},
             .i = {.ifi_index = 1}};
    send(fd, &one, sizeof one, 0);
    answers(fd, RTM_GETLINK);
    answers(fd, RTM_GETLINK);
    printf("no such link\n");
    one.i.ifi_index = 999;
    send(fd, &one, sizeof one, 0);
    answers(fd, RTM_GETLINK);
    printf("a change\n");
    request(fd, RTM_NEWADDR, NLM_F_REQUEST | NLM_F_ACK, AF_INET, 6);
    answers(fd, RTM_GETADDR);
    printf("no request flag\n");
    request(fd, RTM_GETLINK, NLM_F_ACK, AF_UNSPEC, 8);
    answers(fd, RTM_GETLINK);
    int one_flag = 1;
    result("cap ack", setsockopt(fd, SOL_NETLINK, NETLINK_CAP_ACK, &one_flag, sizeof one_flag));
    request(fd, RTM_NEWLINK, NLM_F_REQUEST, AF_UNSPEC, 9);
    answers(fd, RTM_GETLINK);
    int got = -1;
    socklen_t gotlength = sizeof got;
    getsockopt(fd, SOL_NETLINK, NETLINK_CAP_ACK, &got, &gotlength);
    printf("cap ack now %d\n", got);
    result("nothing waiting", recv(fd, &got, sizeof got, MSG_DONTWAIT));
    close(fd);
    close(other);
}

static void ioctls(void) {
    int fd = socket(AF_INET, SOCK_DGRAM, 0);
    struct ifreq list[8];
    struct ifconf c = {.ifc_len = 0, .ifc_buf = NULL};
    result("ifconf size", ioctl(fd, SIOCGIFCONF, &c));
    c.ifc_len = sizeof list;
    c.ifc_req = list;
    result("ifconf", ioctl(fd, SIOCGIFCONF, &c));
    for (int i = 0; i < c.ifc_len / (int)sizeof(struct ifreq); i++)
        if (shown(list[i].ifr_name))
            printf("ifconf %s %s\n", list[i].ifr_name,
                   text(AF_INET, &((struct sockaddr_in *)&list[i].ifr_addr)->sin_addr));
    const char *names[] = {"lo", "eth0", "nothing", "lo:1"};
    for (int i = 0; i < 4; i++) {
        struct ifreq r = {0};
        strcpy(r.ifr_name, names[i]);
        printf("%s:", names[i]);
        if (ioctl(fd, SIOCGIFFLAGS, &r) < 0)
            printf(" flags %s", strerror(errno));
        else
            printf(" flags %#x", r.ifr_flags & 0xffff);
        if (ioctl(fd, SIOCGIFMTU, &r) == 0)
            printf(" mtu %d", r.ifr_mtu);
        if (ioctl(fd, SIOCGIFHWADDR, &r) == 0)
            printf(" hardware family %d", r.ifr_hwaddr.sa_family);
        if (ioctl(fd, SIOCGIFADDR, &r) < 0)
            printf(" address %s", strerror(errno));
        else
            printf(" address %s", text(AF_INET, &((struct sockaddr_in *)&r.ifr_addr)->sin_addr));
        if (ioctl(fd, SIOCGIFNETMASK, &r) == 0)
            printf(" mask %s", text(AF_INET, &((struct sockaddr_in *)&r.ifr_netmask)->sin_addr));
        if (ioctl(fd, SIOCGIFBRDADDR, &r) == 0)
            printf(" broadcast %s",
                   text(AF_INET, &((struct sockaddr_in *)&r.ifr_broadaddr)->sin_addr));
        if (ioctl(fd, SIOCGIFTXQLEN, &r) == 0)
            printf(" queue %d", r.ifr_qlen);
        printf("\n");
    }
    struct ifreq r = {.ifr_ifindex = 1};
    result("name of 1", ioctl(fd, SIOCGIFNAME, &r));
    printf("name %s\n", r.ifr_name);
    r.ifr_ifindex = 999;
    result("name of 999", ioctl(fd, SIOCGIFNAME, &r));
    strcpy(r.ifr_name, "lo");
    result("index", ioctl(fd, SIOCGIFINDEX, &r));
    printf("index %d\n", r.ifr_ifindex);
    r.ifr_flags = 0;
    result("set flags", ioctl(fd, SIOCSIFFLAGS, &r));
    int unix_socket = socket(AF_UNIX, SOCK_STREAM, 0);
    result("flags on a local socket", ioctl(unix_socket, SIOCGIFFLAGS, &r));
    result("address on a local socket", ioctl(unix_socket, SIOCGIFADDR, &r));
    int waiting = -1;
    result("inq", ioctl(fd, FIONREAD, &waiting));
    printf("waiting %d\n", waiting);
    result("not a request", ioctl(fd, 0x1234, &waiting));
    close(fd);
    close(unix_socket);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    addresses();
    netlink();
    ioctls();
    return 0;
}
