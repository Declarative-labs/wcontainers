// Servers inside the container that clients outside connect to through published ports: an IPv4
// listener on the any address (port 8080), an IPv6 one taking IPv4 too (8081), and one on loopback
// (8082) that the outside never reaches. After listening it gives way many times (where a save
// may fall), then tells the test's server outside to start the clients. Arguments: that server's
// name and port.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <ifaddrs.h>
#include <netdb.h>
#include <netinet/in.h>
#include <poll.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
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

static int listener(int family, const char *address, int port) {
    int s = socket(family, SOCK_STREAM, 0);
    int one = 1;
    setsockopt(s, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_storage a = {0};
    socklen_t length;
    if (family == AF_INET) {
        struct sockaddr_in *v4 = (void *)&a;
        v4->sin_family = AF_INET;
        v4->sin_port = htons(port);
        inet_pton(AF_INET, address, &v4->sin_addr);
        length = sizeof *v4;
    } else {
        struct sockaddr_in6 *v6 = (void *)&a;
        v6->sin6_family = AF_INET6;
        v6->sin6_port = htons(port);
        inet_pton(AF_INET6, address, &v6->sin6_addr);
        length = sizeof *v6;
    }
    result("bind", bind(s, (void *)&a, length));
    result("listen", listen(s, 4));
    return s;
}

// The address and port of an IPv4 or IPv6 socket address, IPv4-mapped ones shown as IPv4.
static void show_address(const char *what, struct sockaddr_storage *a, socklen_t length) {
    char text[64] = "";
    int port = 0;
    if (a->ss_family == AF_INET) {
        struct sockaddr_in *v4 = (void *)a;
        inet_ntop(AF_INET, &v4->sin_addr, text, sizeof text);
        port = ntohs(v4->sin_port);
    } else if (a->ss_family == AF_INET6) {
        struct sockaddr_in6 *v6 = (void *)a;
        inet_ntop(AF_INET6, &v6->sin6_addr, text, sizeof text);
        port = ntohs(v6->sin6_port);
    }
    printf("%s family %d length %u %s port nonzero %d\n", what, a->ss_family, length, text,
           port != 0);
}

// Whether a local address is eth0's, whose number the reference's Docker chooses.
static int is_eth0(struct sockaddr_storage *a) {
    struct ifaddrs *all;
    if (getifaddrs(&all))
        return -1;
    int found = 0;
    for (struct ifaddrs *i = all; i; i = i->ifa_next) {
        if (!i->ifa_addr || i->ifa_addr->sa_family != AF_INET || strcmp(i->ifa_name, "eth0"))
            continue;
        struct in_addr own = ((struct sockaddr_in *)i->ifa_addr)->sin_addr;
        if (a->ss_family == AF_INET)
            found = ((struct sockaddr_in *)a)->sin_addr.s_addr == own.s_addr;
        else
            found = !memcmp(&((struct sockaddr_in6 *)a)->sin6_addr.s6_addr[12], &own, 4);
    }
    freeifaddrs(all);
    return found;
}

static void serve(const char *what, int s) {
    struct pollfd p = {.fd = s, .events = POLLIN | POLLOUT};
    printf("%s ", what);
    result("poll", poll(&p, 1, -1));
    printf("revents %#x\n", p.revents);
    struct sockaddr_storage peer;
    socklen_t length = sizeof peer;
    int c = accept(s, (void *)&peer, &length);
    result("accept", c < 0 ? -1 : 0);
    if (c < 0)
        return;
    show_address("peer", &peer, length);
    struct sockaddr_storage local;
    length = sizeof local;
    getsockname(c, (void *)&local, &length);
    printf("local is eth0's %d\n", is_eth0(&local));
    printf("local family %d length %u port %d\n", local.ss_family, length,
           ntohs(((struct sockaddr_in *)&local)->sin_port));
    char got[256];
    size_t held = 0;
    long n;
    while ((n = read(c, got + held, sizeof got - 1 - held)) > 0)
        held += n;
    result("last read", n);
    got[held] = 0;
    printf("got %s\n", got);
    char answer[300];
    int size = snprintf(answer, sizeof answer, "echo: %s", got);
    result("write", write(c, answer, size));
    result("close", close(c));
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    signal(SIGPIPE, SIG_IGN);
    if (argc < 3)
        return 2;
    int v4 = listener(AF_INET, "0.0.0.0", 8080);
    int v6 = listener(AF_INET6, "::", 8081);
    int lo = listener(AF_INET, "127.0.0.1", 8082);
    for (int i = 0; i < 400; i++)
        sched_yield();
    struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
    if (getaddrinfo(argv[1], argv[2], &hints, &found))
        return 3;
    int go = socket(AF_INET, SOCK_STREAM, 0);
    result("go connect", connect(go, found->ai_addr, found->ai_addrlen));
    result("go write", write(go, "go", 2));
    close(go);
    serve("ipv4", v4);
    serve("ipv6", v6);
    // Nothing reaches the loopback listener from outside.
    int flags = 1;
    result("loopback nonblocking", ioctl(lo, FIONBIO, &flags));
    result("loopback accept", accept(lo, 0, 0));
    return 0;
}
