// Talking to a server outside the container, by name: looking the name up, TCP with a blocking and
// a nonblocking connect, the server's end of the stream, a refused connect, UDP, and a name that
// does not exist. Arguments: the server's name, its TCP and UDP port, and a closed port.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static struct sockaddr_in server(const char *name, int port) {
    struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
    int r = getaddrinfo(name, NULL, &hints, &found);
    printf("lookup %s\n", r ? gai_strerror(r) : "found");
    struct sockaddr_in a = {0};
    if (r == 0) {
        a = *(struct sockaddr_in *)found->ai_addr;
        freeaddrinfo(found);
    }
    a.sin_port = htons(port);
    return a;
}

// Reads until the end, printing what came.
static void read_all(int fd) {
    char buf[256];
    long total = 0, n;
    while ((n = read(fd, buf, sizeof buf)) > 0) {
        printf("got %.*s", (int)n, buf);
        total += n;
    }
    result("end", n);
    printf("total %ld\n", total);
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc < 5)
        return 2;
    struct sockaddr_in to = server(argv[1], atoi(argv[2]));
    int s = socket(AF_INET, SOCK_STREAM, 0);
    result("connect", connect(s, (void *)&to, sizeof to));
    result("write", write(s, "hello\n", 6));
    result("shutdown", shutdown(s, SHUT_WR));
    read_all(s);
    close(s);
    // Nonblocking, waiting with poll.
    s = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0);
    result("nonblocking connect", connect(s, (void *)&to, sizeof to));
    struct pollfd p = {.fd = s, .events = POLLOUT};
    result("poll out", poll(&p, 1, 10000));
    int error = -1;
    socklen_t length = sizeof error;
    getsockopt(s, SOL_SOCKET, SO_ERROR, &error, &length);
    printf("error %d\n", error);
    result("write", write(s, "second\n", 7));
    result("shutdown", shutdown(s, SHUT_WR));
    fcntl(s, F_SETFL, 0);
    p.events = POLLIN;
    result("poll in", poll(&p, 1, 10000));
    read_all(s);
    close(s);
    // A port where nothing listens.
    struct sockaddr_in closed = to;
    closed.sin_port = htons(atoi(argv[4]));
    s = socket(AF_INET, SOCK_STREAM, 0);
    result("closed port", connect(s, (void *)&closed, sizeof closed));
    close(s);
    // UDP.
    struct sockaddr_in udp = to;
    udp.sin_port = htons(atoi(argv[3]));
    int u = socket(AF_INET, SOCK_DGRAM, 0);
    result("sendto", sendto(u, "ping", 4, 0, (void *)&udp, sizeof udp));
    p = (struct pollfd){.fd = u, .events = POLLIN};
    result("poll udp", poll(&p, 1, 10000));
    char buf[64];
    struct sockaddr_in from;
    socklen_t fromlength = sizeof from;
    long n = recvfrom(u, buf, sizeof buf, MSG_DONTWAIT, (void *)&from, &fromlength);
    result("recvfrom", n);
    if (n > 0)
        printf("udp %.*s from the server %d\n", (int)n, buf,
               from.sin_addr.s_addr == udp.sin_addr.s_addr && from.sin_port == udp.sin_port);
    close(u);
    server("nonexistent.invalid", 1);
    return 0;
}
