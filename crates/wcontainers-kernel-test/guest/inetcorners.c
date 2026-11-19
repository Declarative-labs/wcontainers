// Socket options read back as Linux keeps them (busy polling, the incoming CPU, TCP_LINGER2,
// the window clamp set at the handshake, the bound device by name and by index, quick
// acknowledgements after an exchange, IPV6_ADDRFORM), a TCP socket connected to itself, and
// the neighbour table's dump.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <net/if.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <sys/ioctl.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static void get(const char *what, int s, int level, int name) {
    int v = -7;
    socklen_t len = sizeof v;
    errno = 0;
    int r = getsockopt(s, level, name, &v, &len);
    if (r < 0)
        printf("%s get: -1 %s\n", what, strerror(errno));
    else
        printf("%s get: len %u %d\n", what, len, v);
}

static void set(const char *what, int s, int level, int name, int v) {
    errno = 0;
    int r = setsockopt(s, level, name, &v, sizeof v);
    printf("%s set %d: %d %s\n", what, v, r, r < 0 ? strerror(errno) : "");
}

static void both(const char *what, int s, int level, int name, int v) {
    set(what, s, level, name, v);
    get(what, s, level, name);
}

// eth0's number differs between hosts (Docker Desktop's kernel makes tunnel devices first), so
// it is shown as a name.
static void device(const char *what, int s, socklen_t room) {
    char name[32];
    memset(name, 'x', sizeof name);
    socklen_t len = room;
    errno = 0;
    int r = getsockopt(s, SOL_SOCKET, SO_BINDTODEVICE, name, &len);
    if (r < 0)
        printf("%s device (room %u): -1 %s\n", what, room, strerror(errno));
    else
        printf("%s device (room %u): len %u name %.*s last %d\n", what, room, len, (int)len, name,
               len ? name[len - 1] : -1);
    int v = -7;
    socklen_t size = sizeof v;
    getsockopt(s, SOL_SOCKET, SO_BINDTOIFINDEX, &v, &size);
    if (v && v == (int)if_nametoindex("eth0"))
        printf("%s index: eth0's\n", what);
    else
        printf("%s index: %d\n", what, v);
}

static void bind_device(const char *what, int s, const char *name, socklen_t len) {
    errno = 0;
    int r = setsockopt(s, SOL_SOCKET, SO_BINDTODEVICE, name, len);
    printf("%s bind %s (%u): %d %s\n", what, name, len, r, r < 0 ? strerror(errno) : "");
}

static struct sockaddr_in at(int port) {
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port), .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    return a;
}

static void pair(int port, int *c, int *s, int rcvbuf, int clamp) {
    int l = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = at(port);
    if (clamp) setsockopt(l, IPPROTO_TCP, TCP_WINDOW_CLAMP, &clamp, sizeof clamp);
    bind(l, (void *)&a, sizeof a);
    listen(l, 4);
    *c = socket(AF_INET, SOCK_STREAM, 0);
    if (rcvbuf) setsockopt(*c, SOL_SOCKET, SO_RCVBUF, &rcvbuf, sizeof rcvbuf);
    if (connect(*c, (void *)&a, sizeof a)) printf("connect %d: %s\n", port, strerror(errno));
    *s = accept(l, NULL, NULL);
    close(l);
}

static void neighbours(int family) {
    int nl = socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE);
    struct {
        struct nlmsghdr h;
        struct ndmsg n;
    } req = {.h = {.nlmsg_len = sizeof req, .nlmsg_type = RTM_GETNEIGH, .nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP, .nlmsg_seq = 3},
             .n = {.ndm_family = family}};
    send(nl, &req, sizeof req, 0);
    char buf[8192];
    long n = recv(nl, buf, sizeof buf, 0);
    struct nlmsghdr *h = (void *)buf;
    printf("neighbours of family %d: %ld bytes, type %d seq %u", family, n, h->nlmsg_type, h->nlmsg_seq);
    if (h->nlmsg_type == NLMSG_ERROR || h->nlmsg_type == NLMSG_DONE)
        printf(" value %d", *(int *)NLMSG_DATA(h));
    printf(" flags %x\n", h->nlmsg_flags);
    close(nl);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int t = socket(AF_INET, SOCK_STREAM, 0);
    both("busy poll", t, SOL_SOCKET, SO_BUSY_POLL, 5);
    both("busy poll", t, SOL_SOCKET, SO_BUSY_POLL, 0);
    both("busy poll", t, SOL_SOCKET, SO_BUSY_POLL, -1);
    both("busy poll", t, SOL_SOCKET, SO_BUSY_POLL, 2147483647);
    get("incoming cpu fresh", t, SOL_SOCKET, SO_INCOMING_CPU);
    both("incoming cpu", t, SOL_SOCKET, SO_INCOMING_CPU, 3);
    both("incoming cpu", t, SOL_SOCKET, SO_INCOMING_CPU, -5);
    both("incoming cpu", t, SOL_SOCKET, SO_INCOMING_CPU, 1000);
    get("linger2 fresh", t, IPPROTO_TCP, TCP_LINGER2);
    both("linger2", t, IPPROTO_TCP, TCP_LINGER2, 0);
    both("linger2", t, IPPROTO_TCP, TCP_LINGER2, -5);
    both("linger2", t, IPPROTO_TCP, TCP_LINGER2, 30);
    both("linger2", t, IPPROTO_TCP, TCP_LINGER2, 500);
    both("linger2", t, IPPROTO_TCP, TCP_LINGER2, 0);
    get("window clamp fresh", t, IPPROTO_TCP, TCP_WINDOW_CLAMP);
    both("window clamp", t, IPPROTO_TCP, TCP_WINDOW_CLAMP, 0);
    both("window clamp", t, IPPROTO_TCP, TCP_WINDOW_CLAMP, 100);
    both("window clamp", t, IPPROTO_TCP, TCP_WINDOW_CLAMP, 0);

    // The bound device, by name and by index, on internet and local sockets.
    device("fresh", t, 32);
    bind_device("tcp", t, "lo", 3);
    device("tcp", t, 32);
    device("tcp", t, 16);
    device("tcp", t, 15);
    bind_device("tcp", t, "eth0", 4);
    device("tcp", t, 32);
    bind_device("tcp", t, "nope", 5);
    device("tcp", t, 32);
    bind_device("tcp", t, "lo-and-a-very-long-name", 23);
    bind_device("tcp", t, "", 1);
    device("tcp", t, 32);
    bind_device("tcp", t, "lo", 0);
    device("tcp", t, 32);
    {
        int eth0 = (int)if_nametoindex("eth0");
        printf("tcp eth0's index: %d\n", setsockopt(t, SOL_SOCKET, SO_BINDTOIFINDEX, &eth0, sizeof eth0));
    }
    device("tcp", t, 32);
    set("tcp index", t, SOL_SOCKET, SO_BINDTOIFINDEX, -1);
    device("tcp", t, 32);
    both("tcp index", t, SOL_SOCKET, SO_BINDTOIFINDEX, 99);
    device("tcp", t, 32);
    both("tcp index", t, SOL_SOCKET, SO_BINDTOIFINDEX, 0);
    device("tcp", t, 32);
    int u = socket(AF_UNIX, SOCK_STREAM, 0);
    bind_device("local", u, "lo", 3);
    device("local", u, 32);
    both("local index", u, SOL_SOCKET, SO_BINDTOIFINDEX, 0);
    close(u);
    close(t);

    // The window clamp a connection gets at its handshake.
    int c, s;
    pair(7201, &c, &s, 0, 0);
    get("connected window clamp", c, IPPROTO_TCP, TCP_WINDOW_CLAMP);
    get("accepted window clamp", s, IPPROTO_TCP, TCP_WINDOW_CLAMP);
    both("connected window clamp", c, IPPROTO_TCP, TCP_WINDOW_CLAMP, 0);
    both("connected window clamp", c, IPPROTO_TCP, TCP_WINDOW_CLAMP, 100);
    both("connected window clamp", c, IPPROTO_TCP, TCP_WINDOW_CLAMP, 100000);
    get("connected linger2", c, IPPROTO_TCP, TCP_LINGER2);
    get("connected quickack", c, IPPROTO_TCP, TCP_QUICKACK);
    // An exchange: the end that answers what it got goes into pingpong mode.
    char buf[8];
    write(c, "abc", 3);
    read(s, buf, 3);
    write(s, "xyz", 3);
    read(c, buf, 3);
    usleep(100000);
    get("after data quickack c", c, IPPROTO_TCP, TCP_QUICKACK);
    get("after data quickack s", s, IPPROTO_TCP, TCP_QUICKACK);
    both("quickack s", s, IPPROTO_TCP, TCP_QUICKACK, 1);
    both("quickack c", c, IPPROTO_TCP, TCP_QUICKACK, 0);
    shutdown(c, SHUT_RDWR);
    usleep(100000);
    both("after shutdown window clamp", c, IPPROTO_TCP, TCP_WINDOW_CLAMP, 0);
    close(c), close(s);
    pair(7202, &c, &s, 4096, 5000);
    get("small buffer window clamp", c, IPPROTO_TCP, TCP_WINDOW_CLAMP);
    get("clamped listener's accepted window clamp", s, IPPROTO_TCP, TCP_WINDOW_CLAMP);
    close(c), close(s);
    {
        int l = socket(AF_INET, SOCK_STREAM, 0);
        struct sockaddr_in a = at(7203);
        bind(l, (void *)&a, sizeof a);
        listen(l, 4);
        get("listener window clamp", l, IPPROTO_TCP, TCP_WINDOW_CLAMP);
        both("listener window clamp", l, IPPROTO_TCP, TCP_WINDOW_CLAMP, 0);
        close(l);
    }

    // A connect to its own address and port opens simultaneously with itself.
    {
        int me = socket(AF_INET, SOCK_STREAM, 0);
        struct sockaddr_in a = at(7204), name;
        bind(me, (void *)&a, sizeof a);
        errno = 0;
        printf("connect self: %d %s\n", connect(me, (void *)&a, sizeof a), strerror(errno));
        printf("write self: %d\n", (int)write(me, "me", 2));
        printf("read self: %d\n", (int)recv(me, buf, sizeof buf, MSG_DONTWAIT));
        socklen_t len = sizeof name;
        getpeername(me, (void *)&name, &len);
        printf("peer port %d\n", ntohs(name.sin_port));
        struct pollfd p = {me, POLLIN | POLLOUT | POLLRDHUP, 0};
        poll(&p, 1, 0);
        printf("self poll %x\n", p.revents);
        write(me, "again", 5);
        int queued = -1;
        ioctl(me, FIONREAD, &queued);
        printf("self queued %d\n", queued);
        printf("self shutdown: %d\n", shutdown(me, SHUT_WR));
        p.revents = 0;
        poll(&p, 1, 0);
        printf("self poll after shutdown %x\n", p.revents);
        printf("self reads %d then %d\n", (int)read(me, buf, sizeof buf), (int)read(me, buf, sizeof buf));
        errno = 0;
        printf("self write after shutdown %d %s\n", (int)write(me, "x", 1), strerror(errno));
        close(me);
        // Closed with bytes unread: its reset is its own.
        me = socket(AF_INET, SOCK_STREAM, 0);
        a = at(7207);
        bind(me, (void *)&a, sizeof a);
        printf("connect self again: %d\n", connect(me, (void *)&a, sizeof a));
        write(me, "left", 4);
        close(me);
    }

    // IPV6_ADDRFORM: a connected IPv6 socket to an IPv4-mapped address becomes IPv4.
    {
        int u6 = socket(AF_INET6, SOCK_DGRAM, 0);
        get("unconnected addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM);
        set("unconnected addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM, AF_INET);
        set("unconnected addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM, AF_INET6);
        struct sockaddr_in6 to = {.sin6_family = AF_INET6, .sin6_port = htons(7205)};
        to.sin6_addr = in6addr_loopback;
        connect(u6, (void *)&to, sizeof to);
        get("v6 peer addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM);
        set("v6 peer addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM, AF_INET);
        inet_pton(AF_INET6, "::ffff:127.0.0.1", &to.sin6_addr);
        connect(u6, (void *)&to, sizeof to);
        get("mapped peer addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM);
        set("mapped peer addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM, 5);
        set("mapped peer addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM, AF_INET);
        get("after addrform domain", u6, SOL_SOCKET, SO_DOMAIN);
        get("after addrform addrform", u6, IPPROTO_IPV6, IPV6_ADDRFORM);
        get("after addrform ttl", u6, IPPROTO_IP, IP_TTL);
        struct sockaddr_storage name;
        socklen_t len = sizeof name;
        getsockname(u6, (void *)&name, &len);
        printf("after addrform name family %d len %u\n", name.ss_family, len);
        len = sizeof name;
        getpeername(u6, (void *)&name, &len);
        printf("after addrform peer family %d len %u port %d\n", name.ss_family, len,
               ntohs(((struct sockaddr_in *)&name)->sin_port));
        close(u6);
        int six = socket(AF_INET6, SOCK_STREAM, 0);
        set("v6only tcp", six, IPPROTO_IPV6, IPV6_V6ONLY, 1);
        close(six);
        int l = socket(AF_INET, SOCK_STREAM, 0);
        struct sockaddr_in a = at(7206);
        bind(l, (void *)&a, sizeof a);
        listen(l, 4);
        int t6 = socket(AF_INET6, SOCK_STREAM, 0);
        to.sin6_port = htons(7206);
        printf("tcp6 connect mapped: %d\n", connect(t6, (void *)&to, sizeof to));
        int a4 = accept(l, NULL, NULL);
        set("tcp6 addrform", t6, IPPROTO_IPV6, IPV6_ADDRFORM, AF_INET);
        get("tcp6 after addrform domain", t6, SOL_SOCKET, SO_DOMAIN);
        printf("tcp6 after addrform write %d\n", (int)write(t6, "ok", 2));
        printf("tcp6 after addrform read %d\n", (int)read(a4, buf, sizeof buf));
        len = sizeof name;
        getsockname(t6, (void *)&name, &len);
        printf("tcp6 after addrform name family %d len %u\n", name.ss_family, len);
        close(t6), close(a4), close(l);
    }

    neighbours(AF_UNSPEC);
    neighbours(AF_INET);
    neighbours(AF_INET6);
    return 0;
}
