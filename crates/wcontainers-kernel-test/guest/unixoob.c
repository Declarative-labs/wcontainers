// Urgent bytes on local stream sockets (MSG_OOB, SO_OOBINLINE, SIOCATMARK, POLLPRI and SIGURG) and
// I/O signals from local sockets with O_ASYNC.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>

static volatile int urgent, io, chosen, pipes;
static void on_pipe(int signal) { (void)signal, pipes++; }
static volatile int chosen_code, chosen_fd;
static volatile long chosen_band;

static void on_urgent(int signal) { (void)signal, urgent++; }
static void on_io(int signal) { (void)signal, io++; }
static void on_chosen(int signal, siginfo_t *info, void *context) {
    (void)signal, (void)context;
    chosen++;
    chosen_code = info->si_code;
    chosen_band = info->si_band;
    chosen_fd = info->si_fd;
}

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static short revents(int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLPRI | POLLRDHUP | POLLRDBAND};
    poll(&p, 1, 0);
    return p.revents;
}

static void state(const char *what, int fd) {
    int mark = -1, queued = -1;
    long a = ioctl(fd, SIOCATMARK, &mark);
    long b = ioctl(fd, FIONREAD, &queued);
    printf("%s: mark %ld/%d queued %ld/%d poll %#x\n", what, a, mark, b, queued, revents(fd));
}

static void receive(const char *what, int fd, size_t size, int flags) {
    char buf[64] = {0};
    struct iovec iov = {.iov_base = buf, .iov_len = size};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1};
    long r = recvmsg(fd, &msg, flags | MSG_DONTWAIT);
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld \"%.*s\" flags %#x\n", what, r, (int)r, buf, msg.msg_flags);
}

static void pair(int sv[2]) {
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, sv) < 0)
        perror("socketpair");
}

static void basics(void) {
    int sv[2];
    pair(sv);
    state("fresh", sv[1]);
    result("send none urgent", send(sv[0], "", 0, MSG_OOB));
    result("send urgent", send(sv[0], "abc", 3, MSG_OOB));
    state("after urgent", sv[1]);
    receive("receive urgent peeked", sv[1], 8, MSG_OOB | MSG_PEEK);
    receive("receive urgent", sv[1], 8, MSG_OOB);
    state("after urgent read", sv[1]);
    receive("receive urgent again", sv[1], 8, MSG_OOB);
    receive("receive rest", sv[1], 8, 0);
    state("after rest", sv[1]);
    receive("receive empty", sv[1], 8, 0);
    close(sv[0]);
    close(sv[1]);
}

static void mark(void) {
    int sv[2];
    pair(sv);
    send(sv[0], "ab", 2, 0);
    send(sv[0], "cd", 2, MSG_OOB);
    send(sv[0], "ef", 2, 0);
    state("queued around urgent", sv[1]);
    receive("peek across", sv[1], 16, MSG_PEEK);
    receive("read to mark", sv[1], 16, 0);
    state("at mark", sv[1]);
    receive("peek at mark", sv[1], 16, MSG_PEEK);
    receive("read past mark", sv[1], 16, 0);
    state("past mark", sv[1]);
    receive("urgent after skipping", sv[1], 8, MSG_OOB);
    // A second urgent byte takes over from the first.
    send(sv[0], "x", 1, MSG_OOB);
    send(sv[0], "y", 1, MSG_OOB);
    state("two urgent", sv[1]);
    receive("urgent of two", sv[1], 8, MSG_OOB);
    state("one left", sv[1]);
    receive("read after two", sv[1], 16, 0);
    receive("read after two again", sv[1], 16, 0);
    // An urgent byte read out of band leaves an empty buffer in the queue.
    send(sv[0], "12", 2, MSG_OOB);
    send(sv[0], "34", 2, 0);
    receive("urgent before data", sv[1], 8, MSG_OOB);
    state("hole", sv[1]);
    receive("peek over hole", sv[1], 16, MSG_PEEK);
    receive("read over hole", sv[1], 16, 0);
    state("after hole", sv[1]);
    // Partial reads and the queued count.
    send(sv[0], "0123456789", 10, 0);
    receive("read part", sv[1], 3, 0);
    state("after part", sv[1]);
    receive("read rest of part", sv[1], 16, 0);
    // Into no room.
    send(sv[0], "z", 1, MSG_OOB);
    receive("urgent into nothing", sv[1], 0, MSG_OOB);
    state("after nothing", sv[1]);
    receive("urgent after nothing", sv[1], 8, MSG_OOB);
    close(sv[0]);
    close(sv[1]);
}

static void inline_urgent(void) {
    int sv[2], on = 1, value = -1;
    socklen_t length = sizeof value;
    pair(sv);
    result("set inline", setsockopt(sv[1], SOL_SOCKET, SO_OOBINLINE, &on, sizeof on));
    getsockopt(sv[1], SOL_SOCKET, SO_OOBINLINE, &value, &length);
    printf("inline %d\n", value);
    send(sv[0], "pq", 2, 0);
    send(sv[0], "rs", 2, MSG_OOB);
    send(sv[0], "tu", 2, 0);
    state("inline queued", sv[1]);
    receive("inline urgent", sv[1], 8, MSG_OOB);
    receive("inline read", sv[1], 16, 0);
    state("inline at mark", sv[1]);
    receive("inline peek", sv[1], 16, MSG_PEEK);
    receive("inline read on", sv[1], 16, 0);
    state("inline after", sv[1]);
    close(sv[0]);
    close(sv[1]);
}

static void refused(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_SEQPACKET, 0, sv);
    result("packet urgent", send(sv[0], "a", 1, MSG_OOB));
    send(sv[0], "a", 1, 0);
    receive("packet receive urgent", sv[1], 8, MSG_OOB);
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    result("datagram urgent", send(sv[0], "a", 1, MSG_OOB));
    close(sv[0]);
    close(sv[1]);
    pair(sv);
    shutdown(sv[1], SHUT_RD);
    result("urgent to shut reader", send(sv[0], "ab", 2, MSG_OOB | MSG_NOSIGNAL));
    receive("shut reader", sv[1], 8, 0);
    close(sv[0]);
    close(sv[1]);
    // A lone urgent byte's broken pipe sends no SIGPIPE; with bytes before it, one does.
    signal(SIGPIPE, on_pipe);
    pair(sv);
    shutdown(sv[1], SHUT_RD);
    result("lone urgent to shut reader", send(sv[0], "a", 1, MSG_OOB));
    printf("pipe signals %d\n", pipes);
    result("urgent after bytes to shut reader", send(sv[0], "ab", 2, MSG_OOB));
    printf("pipe signals %d\n", pipes);
    shutdown(sv[0], SHUT_WR);
    result("lone urgent after own shutdown", send(sv[0], "a", 1, MSG_OOB));
    printf("pipe signals %d\n", pipes);
    close(sv[0]);
    close(sv[1]);
    signal(SIGPIPE, SIG_IGN);
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    receive("unconnected urgent", listener, 8, MSG_OOB);
    // A connection the listener never took is reset when the listener goes.
    struct sockaddr_un address = {.sun_family = AF_UNIX, .sun_path = "\0oobr"};
    bind(listener, (struct sockaddr *)&address, sizeof(sa_family_t) + 5);
    listen(listener, 4);
    int client = socket(AF_UNIX, SOCK_STREAM, 0);
    connect(client, (struct sockaddr *)&address, sizeof(sa_family_t) + 5);
    close(listener);
    receive("client of a gone listener", client, 8, 0);
    receive("client of a gone listener again", client, 8, 0);
    close(client);
}

static void passing(void) {
    int sv[2], other[2];
    pair(sv);
    pair(other);
    char control[CMSG_SPACE(sizeof(int))] = {0};
    struct iovec iov = {.iov_base = "fg", .iov_len = 2};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control,
                         .msg_controllen = sizeof control};
    struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    c->cmsg_len = CMSG_LEN(sizeof(int));
    memcpy(CMSG_DATA(c), &other[0], sizeof(int));
    result("urgent with descriptor", sendmsg(sv[0], &msg, MSG_OOB));
    for (int round = 0; round < 2; round++) {
        char buf[8] = {0}, got[CMSG_SPACE(sizeof(int))] = {0};
        struct iovec in = {.iov_base = buf, .iov_len = sizeof buf};
        struct msghdr m = {.msg_iov = &in, .msg_iovlen = 1, .msg_control = got,
                           .msg_controllen = sizeof got};
        long r = recvmsg(sv[1], &m, MSG_DONTWAIT | (round ? MSG_OOB : 0));
        struct cmsghdr *h = CMSG_FIRSTHDR(&m);
        printf("receive %d: %ld \"%.*s\" flags %#x control %s\n", round, r, r > 0 ? (int)r : 0,
               buf, m.msg_flags, h ? (h->cmsg_type == SCM_RIGHTS ? "rights" : "other") : "none");
        if (h && h->cmsg_type == SCM_RIGHTS) {
            int fd;
            memcpy(&fd, CMSG_DATA(h), sizeof fd);
            close(fd);
        }
    }
    close(other[0]);
    close(other[1]);
    close(sv[0]);
    close(sv[1]);
}

static void waiting(void) {
    int sv[2];
    pair(sv);
    int ep = epoll_create1(0);
    struct epoll_event event = {.events = EPOLLPRI | EPOLLET, .data.u32 = 7};
    epoll_ctl(ep, EPOLL_CTL_ADD, sv[1], &event);
    struct epoll_event out[2];
    result("epoll before", epoll_wait(ep, out, 2, 0));
    send(sv[0], "k", 1, MSG_OOB);
    long n = epoll_wait(ep, out, 2, 0);
    printf("epoll urgent: %ld %#x\n", n, n > 0 ? out[0].events : 0);
    result("epoll edge again", epoll_wait(ep, out, 2, 0));
    char buf[4];
    recv(sv[1], buf, 4, MSG_OOB);
    result("epoll after read", epoll_wait(ep, out, 2, 0));
    // A reader waiting in poll for urgent bytes only.
    if (fork() == 0) {
        usleep(20000);
        send(sv[0], "m", 1, MSG_OOB);
        _exit(0);
    }
    struct pollfd p = {.fd = sv[1], .events = POLLPRI};
    long r = poll(&p, 1, 5000);
    printf("poll waited: %ld %#x\n", r, p.revents);
    close(ep);
    close(sv[0]);
    close(sv[1]);
}

static void signals(void) {
    int sv[2];
    signal(SIGURG, on_urgent);
    signal(SIGIO, on_io);
    pair(sv);
    send(sv[0], "a", 1, MSG_OOB);
    printf("no owner: urgent %d io %d\n", urgent, io);
    result("own", fcntl(sv[1], F_SETOWN, getpid()));
    send(sv[0], "b", 1, MSG_OOB);
    printf("owner: urgent %d io %d\n", urgent, io);
    // O_ASYNC: data, room and the other end going.
    result("async", fcntl(sv[1], F_SETFL, O_ASYNC | O_NONBLOCK));
    printf("flags %#x\n", fcntl(sv[1], F_GETFL) & (O_ASYNC | O_NONBLOCK));
    send(sv[0], "c", 1, 0);
    printf("data: urgent %d io %d\n", urgent, io);
    send(sv[0], "d", 1, MSG_OOB);
    printf("urgent async: urgent %d io %d\n", urgent, io);
    char buf[64];
    recv(sv[1], buf, sizeof buf, MSG_DONTWAIT);
    printf("read: urgent %d io %d\n", urgent, io);
    shutdown(sv[0], SHUT_WR);
    printf("peer shut: urgent %d io %d\n", urgent, io);
    close(sv[0]);
    printf("peer closed: urgent %d io %d\n", urgent, io);
    close(sv[1]);
    // A chosen signal carries the band and the descriptor.
    struct sigaction action = {.sa_sigaction = on_chosen, .sa_flags = SA_SIGINFO};
    sigaction(SIGRTMIN + 2, &action, NULL);
    pair(sv);
    fcntl(sv[1], F_SETOWN, getpid());
    fcntl(sv[1], F_SETSIG, SIGRTMIN + 2);
    fcntl(sv[1], F_SETFL, O_ASYNC);
    send(sv[0], "e", 1, 0);
    printf("chosen data: %d code %d band %#lx fd %d\n", chosen, chosen_code, chosen_band,
           chosen_fd == sv[1]);
    send(sv[0], "f", 1, MSG_OOB);
    printf("chosen urgent: %d code %d band %#lx urgent %d io %d\n", chosen, chosen_code,
           chosen_band, urgent, io);
    // The writer's side: room comes back after a send found none.
    int writer = sv[0];
    fcntl(writer, F_SETOWN, getpid());
    fcntl(writer, F_SETSIG, SIGRTMIN + 2);
    fcntl(writer, F_SETFL, O_ASYNC | O_NONBLOCK);
    int before = chosen;
    long sent = 0;
    while (send(writer, buf, sizeof buf, MSG_DONTWAIT) > 0)
        sent++;
    printf("filled: %d more\n", chosen - before);
    before = chosen;
    while (recv(sv[1], buf, sizeof buf, MSG_DONTWAIT) > 0)
        ;
    printf("drained: %d more code %d band %#lx\n", chosen - before, chosen_code, chosen_band);
    close(sv[0]);
    close(sv[1]);
    // Datagram sockets and a listener.
    pair(sv);
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    fcntl(sv[1], F_SETOWN, getpid());
    fcntl(sv[1], F_SETFL, O_ASYNC);
    before = io;
    send(sv[0], "g", 1, 0);
    printf("datagram: io %d\n", io - before);
    close(sv[0]);
    close(sv[1]);
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un address = {.sun_family = AF_UNIX, .sun_path = "\0oobl"};
    bind(listener, (struct sockaddr *)&address, sizeof(sa_family_t) + 5);
    listen(listener, 4);
    fcntl(listener, F_SETOWN, getpid());
    fcntl(listener, F_SETFL, O_ASYNC);
    before = io;
    int client = socket(AF_UNIX, SOCK_STREAM, 0);
    connect(client, (struct sockaddr *)&address, sizeof(sa_family_t) + 5);
    printf("connection: io %d\n", io - before);
    close(client);
    close(listener);
    // A reader waiting for data hears of it by its wake, not by a signal.
    pair(sv);
    fcntl(sv[1], F_SETOWN, getpid());
    fcntl(sv[1], F_SETFL, O_ASYNC);
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
        send(sv[0], "h", 1, 0);
        _exit(0);
    }
    char one[1];
    long r = recv(sv[1], one, 1, 0);
    printf("waited read %ld: io %d\n", r, io - before);
    wait(NULL);
    send(sv[0], "i", 1, 0);
    printf("after the wait: io %d\n", io - before);
    close(sv[0]);
    close(sv[1]);
}

static void broken_pipes(void) {
    signal(SIGPIPE, on_pipe);
    int kinds[] = {SOCK_SEQPACKET, SOCK_DGRAM, SOCK_STREAM};
    for (int i = 0; i < 3; i++) {
        int sv[2];
        printf("kind %d\n", kinds[i]);
        socketpair(AF_UNIX, kinds[i], 0, sv);
        shutdown(sv[0], SHUT_WR);
        result("own shut", send(sv[0], "a", 1, 0));
        close(sv[0]);
        close(sv[1]);
        socketpair(AF_UNIX, kinds[i], 0, sv);
        shutdown(sv[1], SHUT_RD);
        result("peer shut", send(sv[0], "a", 1, 0));
        close(sv[1]);
        result("peer closed", send(sv[0], "a", 1, 0));
        close(sv[0]);
        printf("pipe signals %d\n", pipes);
    }
    signal(SIGPIPE, SIG_IGN);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    basics();
    mark();
    inline_urgent();
    refused();
    broken_pipes();
    passing();
    waiting();
    signals();
    return 0;
}
