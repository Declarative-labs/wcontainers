// I/O signals from internet and netlink sockets with O_ASYNC.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile int io, chosen, chosen_code;
static volatile long chosen_band;
static volatile int chosen_fd;

static void on_io(int signal) { (void)signal, io++; }
static void on_chosen(int signal, siginfo_t *info, void *context) {
    (void)signal, (void)context;
    chosen++;
    chosen_code = info->si_code;
    chosen_band = info->si_band;
    chosen_fd = info->si_fd;
}

static void settle(void) { usleep(20000); }

static void watch(int fd, int how) {
    fcntl(fd, F_SETOWN, getpid());
    fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_ASYNC | how);
}

static void chosen_on(int fd) {
    fcntl(fd, F_SETOWN, getpid());
    fcntl(fd, F_SETSIG, SIGRTMIN + 3);
    fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_ASYNC);
}

static void report(const char *what, int before) {
    printf("%s: io %d\n", what, io - before);
}

static void report_chosen(const char *what, int before, int fd) {
    printf("%s: %d code %d band %#lx fd %d\n", what, chosen - before, chosen_code, chosen_band,
           chosen_fd == fd);
}

static int listener_at(int port) {
    int l = socket(AF_INET, SOCK_STREAM, 0);
    int one = 1;
    setsockopt(l, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port),
                            .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    if (bind(l, (struct sockaddr *)&a, sizeof a) < 0 || listen(l, 4) < 0)
        perror("listener");
    return l;
}

static struct sockaddr_in loopback(int port) {
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port),
                            .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    return a;
}

static void tcp(void) {
    int l = listener_at(5301);
    watch(l, 0);
    int c = socket(AF_INET, SOCK_STREAM, 0);
    watch(c, O_NONBLOCK);
    int before = io;
    struct sockaddr_in a = loopback(5301);
    long r = connect(c, (struct sockaddr *)&a, sizeof a);
    printf("connect %ld %s\n", r, r < 0 ? strerror(errno) : "");
    settle();
    report("tcp connect and listener", before);
    int s = accept(l, NULL, NULL);
    watch(s, O_NONBLOCK);
    before = io;
    send(c, "abc", 3, 0);
    settle();
    report("tcp data", before);
    before = io;
    send(c, "d", 1, 0);
    settle();
    report("tcp more data", before);
    char buf[65536];
    recv(s, buf, sizeof buf, 0);
    before = io;
    recv(s, buf, sizeof buf, 0);
    report("tcp read empty", before);
    // Room coming back after a send found none.
    before = io;
    long sent = 0;
    while (send(c, buf, sizeof buf, MSG_DONTWAIT) > 0)
        sent++;
    settle();
    report("tcp filled", before);
    before = io;
    while (recv(s, buf, sizeof buf, MSG_DONTWAIT) > 0)
        settle();
    settle();
    printf("tcp drained: io %d\n", io - before > 0);
    // The other end shutting and closing.
    before = io;
    shutdown(c, SHUT_WR);
    settle();
    report("tcp peer shut", before);
    before = io;
    shutdown(s, SHUT_WR);
    settle();
    report("tcp own shut after peer", before);
    before = io;
    close(c);
    settle();
    report("tcp peer closed", before);
    close(s);
    // A reset from a close with unread bytes.
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (struct sockaddr *)&a, sizeof a);
    s = accept(l, NULL, NULL);
    watch(c, 0);
    send(c, "x", 1, 0);
    settle();
    before = io;
    close(s);
    settle();
    report("tcp reset", before);
    close(c);
    // A chosen signal carries the band and the descriptor.
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (struct sockaddr *)&a, sizeof a);
    s = accept(l, NULL, NULL);
    chosen_on(s);
    before = chosen;
    send(c, "e", 1, 0);
    settle();
    report_chosen("tcp chosen data", before, s);
    before = chosen;
    shutdown(c, SHUT_WR);
    settle();
    report_chosen("tcp chosen shut", before, s);
    close(c);
    close(s);
    close(l);
    // A refused connect.
    c = socket(AF_INET, SOCK_STREAM, 0);
    watch(c, O_NONBLOCK);
    a = loopback(5302);
    before = io;
    connect(c, (struct sockaddr *)&a, sizeof a);
    settle();
    report("tcp refused", before);
    close(c);
    // A reader waiting for data.
    l = listener_at(5303);
    a = loopback(5303);
    c = socket(AF_INET, SOCK_STREAM, 0);
    connect(c, (struct sockaddr *)&a, sizeof a);
    s = accept(l, NULL, NULL);
    watch(s, 0);
    before = io;
    pid_t parent = getpid();
    if (fork() == 0) {
        char path[64], stat[256];
        snprintf(path, sizeof path, "/proc/%d/stat", parent);
        for (;;) {
            int fd = open(path, O_RDONLY);
            long n = read(fd, stat, sizeof stat - 1);
            close(fd);
            stat[n > 0 ? n : 0] = 0;
            char *end = strrchr(stat, ')');
            if (end && end[2] == 'S')
                break;
            usleep(1000);
        }
        send(c, "h", 1, 0);
        _exit(0);
    }
    r = recv(s, buf, 1, 0);
    printf("tcp waited read %ld: io %d\n", r, io - before);
    wait(NULL);
    close(c);
    close(s);
    close(l);
}

static void udp(void) {
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in a = loopback(5311);
    bind(u, (struct sockaddr *)&a, sizeof a);
    watch(u, O_NONBLOCK);
    int before = io;
    sendto(u, "a", 1, 0, (struct sockaddr *)&a, sizeof a);
    settle();
    report("udp datagram", before);
    char buf[64];
    recv(u, buf, sizeof buf, 0);
    // A connected socket hears of an unreachable port.
    int v = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in closed = loopback(5312);
    connect(v, (struct sockaddr *)&closed, sizeof closed);
    watch(v, O_NONBLOCK);
    before = io;
    send(v, "b", 1, 0);
    settle();
    report("udp unreachable", before);
    long r = recv(v, buf, sizeof buf, 0);
    printf("udp after %ld %s\n", r, r < 0 ? strerror(errno) : "");
    close(v);
    // A chosen signal on a datagram.
    chosen_on(u);
    before = chosen;
    sendto(u, "c", 1, 0, (struct sockaddr *)&a, sizeof a);
    settle();
    report_chosen("udp chosen", before, u);
    close(u);
}

static void netlink(void) {
    int n = socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
    watch(n, O_NONBLOCK);
    struct {
        struct nlmsghdr h;
        struct ifinfomsg i;
    } q = {.h = {.nlmsg_len = sizeof q, .nlmsg_type = RTM_GETLINK,
                 .nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP, .nlmsg_seq = 1}};
    int before = io;
    send(n, &q, sizeof q, 0);
    settle();
    report("netlink dump asked", before);
    // Each part of the dump comes as the one before is read, and is told.
    char buf[32768];
    int parts = 0, told = 0;
    for (;;) {
        before = io;
        long r = recv(n, buf, sizeof buf, 0);
        if (r <= 0)
            break;
        parts++;
        told += io - before;
    }
    printf("netlink each part told: %d\n", told == parts - 1);
    // A second dump while one runs is refused.
    send(n, &q, sizeof q, 0);
    q.h.nlmsg_seq = 2;
    send(n, &q, sizeof q, 0);
    int busy = 0, done = 0;
    long r;
    while ((r = recv(n, buf, sizeof buf, 0)) > 0) {
        for (struct nlmsghdr *h = (void *)buf; NLMSG_OK(h, r); h = NLMSG_NEXT(h, r)) {
            if (h->nlmsg_type == NLMSG_ERROR)
                busy = h->nlmsg_seq * 1000 - ((struct nlmsgerr *)NLMSG_DATA(h))->error;
            if (h->nlmsg_type == NLMSG_DONE)
                done = h->nlmsg_seq;
        }
    }
    printf("netlink second dump: error %d done %d\n", busy, done);
    // A read that faults lets its message go.
    send(n, &q, sizeof q, 0);
    r = recv(n, (void *)8, 4096, 0);
    printf("netlink fault %ld %s\n", r, r < 0 ? strerror(errno) : "");
    parts = 0;
    while (recv(n, buf, sizeof buf, 0) > 0)
        parts++;
    printf("netlink after fault: %d parts\n", parts > 0);
    close(n);
}

int main(void) {
    signal(SIGIO, on_io);
    struct sigaction action = {.sa_sigaction = on_chosen, .sa_flags = SA_SIGINFO};
    sigaction(SIGRTMIN + 3, &action, NULL);
    tcp();
    udp();
    netlink();
    return 0;
}
