// TCP's urgent data (MSG_OOB) between the container's own addresses: the mark, out of band reads,
// SO_OOBINLINE, a second urgent byte, poll, SIOCATMARK and SIGURG.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <unistd.h>

static volatile int urgent;
static void on_urgent(int signal) { (void)signal, urgent++; }

static short revents(int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLPRI | POLLRDHUP | POLLRDBAND};
    poll(&p, 1, 0);
    return p.revents;
}

static void state(const char *what, int fd) {
    int mark = -1, queued = -1;
    long a = ioctl(fd, SIOCATMARK, &mark);
    long b = ioctl(fd, FIONREAD, &queued);
    printf("%s: mark %ld/%d queued %ld/%d poll %#x urgent %d\n", what, a, mark, b, queued,
           revents(fd), urgent);
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

static int listener;

static void pair(int *client, int *server) {
    struct sockaddr_in a;
    socklen_t length = sizeof a;
    getsockname(listener, (struct sockaddr *)&a, &length);
    *client = socket(AF_INET, SOCK_STREAM, 0);
    connect(*client, (struct sockaddr *)&a, sizeof a);
    *server = accept(listener, NULL, NULL);
    fcntl(*server, F_SETOWN, getpid());
}

static void sent(const char *what, int fd, const char *bytes, int flags) {
    long r = send(fd, bytes, strlen(bytes), flags);
    usleep(20000);
    if (r < 0)
        printf("send %s: -1 %s\n", what, strerror(errno));
}

int main(void) {
    signal(SIGURG, on_urgent);
    listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(listener, (struct sockaddr *)&a, sizeof a);
    listen(listener, 8);
    int c, s;
    // One urgent byte after ordinary ones.
    pair(&c, &s);
    receive("nothing yet", s, 10, MSG_OOB);
    sent("ab", c, "ab", 0);
    sent("cd", c, "cd", MSG_OOB);
    state("after", s);
    receive("peek oob", s, 10, MSG_OOB | MSG_PEEK);
    receive("read", s, 10, 0);
    state("at mark", s);
    receive("read at mark", s, 10, 0);
    receive("oob", s, 10, MSG_OOB);
    state("after oob", s);
    receive("oob again", s, 10, MSG_OOB);
    sent("ef", c, "ef", 0);
    receive("read past", s, 10, 0);
    state("past", s);
    close(c);
    close(s);
    // The urgent byte read in band before it is taken out of band.
    pair(&c, &s);
    sent("gh", c, "gh", MSG_OOB);
    sent("ij", c, "ij", 0);
    receive("read to mark", s, 10, 0);
    receive("read over mark", s, 10, 0);
    state("over", s);
    receive("oob after", s, 10, MSG_OOB);
    close(c);
    close(s);
    // A second urgent byte before the first is read.
    pair(&c, &s);
    sent("kl", c, "kl", MSG_OOB);
    sent("mn", c, "mn", MSG_OOB);
    state("two", s);
    receive("two read", s, 10, 0);
    receive("two read again", s, 10, 0);
    receive("two oob", s, 10, MSG_OOB);
    receive("two read last", s, 10, 0);
    close(c);
    close(s);
    // Taken out of band first, then passed in band.
    pair(&c, &s);
    sent("xy", c, "xy", MSG_OOB);
    sent("z", c, "z", 0);
    receive("first oob", s, 10, MSG_OOB);
    state("after first oob", s);
    receive("first read", s, 10, 0);
    state("at taken mark", s);
    receive("first read on", s, 10, 0);
    state("taken passed", s);
    sent("AB", c, "AB", MSG_OOB);
    receive("peeked in band", s, 10, MSG_PEEK);
    receive("read after peek", s, 10, 0);
    receive("peek at mark", s, 10, MSG_PEEK);
    receive("oob after peek", s, 10, MSG_OOB);
    close(c);
    close(s);
    // A second urgent byte when the first is next.
    pair(&c, &s);
    sent("o", c, "o", MSG_OOB);
    sent("pq", c, "pq", MSG_OOB);
    state("next", s);
    receive("next read", s, 10, 0);
    receive("next oob", s, 10, MSG_OOB);
    receive("next read again", s, 10, 0);
    close(c);
    close(s);
    // Inline.
    pair(&c, &s);
    int one = 1;
    setsockopt(s, SOL_SOCKET, SO_OOBINLINE, &one, sizeof one);
    sent("rs", c, "rs", MSG_OOB);
    sent("tu", c, "tu", 0);
    state("inline", s);
    receive("inline oob", s, 10, MSG_OOB);
    receive("inline read", s, 10, 0);
    state("inline at mark", s);
    receive("inline read again", s, 10, 0);
    close(c);
    close(s);
    // Room for nothing, truncation, and an end.
    pair(&c, &s);
    sent("vw", c, "vw", MSG_OOB);
    receive("oob into nothing", s, 0, MSG_OOB);
    receive("oob trunc", s, 10, MSG_OOB | MSG_TRUNC);
    receive("oob after trunc", s, 10, MSG_OOB);
    shutdown(c, SHUT_WR);
    usleep(20000);
    receive("read before end", s, 10, 0);
    receive("oob at end", s, 10, MSG_OOB);
    close(c);
    close(s);
    pair(&c, &s);
    shutdown(c, SHUT_WR);
    usleep(20000);
    receive("oob at bare end", s, 10, MSG_OOB);
    close(c);
    close(s);
    // An urgent byte alone, many bytes, and a send of nothing.
    pair(&c, &s);
    char big[5000];
    memset(big, 'x', sizeof big);
    big[sizeof big - 1] = 0;
    big[sizeof big - 2] = 'Z';
    sent("big", c, big, MSG_OOB);
    int queued = -1;
    ioctl(s, FIONREAD, &queued);
    printf("big queued %d\n", queued);
    receive("big oob", s, 10, MSG_OOB);
    long total = 0, r;
    char sink[8192];
    while ((r = recv(s, sink, sizeof sink, MSG_DONTWAIT)) > 0)
        total += r;
    printf("big in band %ld\n", total);
    printf("send nothing oob %zd\n", send(c, "", 0, MSG_OOB));
    close(c);
    close(s);
    printf("signals %d\n", urgent);
    return 0;
}
