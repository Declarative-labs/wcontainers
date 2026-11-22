// A connection to an echoing server outside, shut for reading by the program: what the server
// sends after the shutdown is still read, then a read finds the end. Arguments: the server's name
// and port.
#define _GNU_SOURCE
#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static void show(const char *what, int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLRDHUP};
    int r = poll(&p, 1, 0);
    printf("%s: poll %d%s%s%s%s%s\n", what, r, p.revents & POLLIN ? " in" : "",
           p.revents & POLLOUT ? " out" : "", p.revents & POLLERR ? " err" : "",
           p.revents & POLLHUP ? " hup" : "", p.revents & POLLRDHUP ? " rdhup" : "");
}

static void receive(const char *what, int fd, int flags) {
    char bytes[16];
    long n = recv(fd, bytes, sizeof bytes, flags);
    printf("%s: %ld %.*s%s\n", what, n, n > 0 ? (int)n : 0, bytes, n < 0 ? strerror(errno) : "");
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc < 3)
        return 2;
    struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
    if (getaddrinfo(argv[1], NULL, &hints, &found) != 0)
        return 3;
    struct sockaddr_in to = *(struct sockaddr_in *)found->ai_addr;
    freeaddrinfo(found);
    to.sin_port = htons(atoi(argv[2]));
    int s = socket(AF_INET, SOCK_STREAM, 0);
    printf("connect %d\n", connect(s, (void *)&to, sizeof to));
    printf("write %ld\n", (long)write(s, "a", 1));
    receive("before shutdown", s, 0);
    printf("shutdown rd %d\n", shutdown(s, SHUT_RD));
    show("after shutdown", s);
    printf("write %ld\n", (long)write(s, "bc", 2));
    usleep(300000);
    receive("echoed after shutdown", s, MSG_DONTWAIT);
    receive("then", s, MSG_DONTWAIT);
    receive("blocking", s, 0);
    printf("shutdown wr %d\n", shutdown(s, SHUT_WR));
    usleep(300000);
    show("after both", s);
    receive("at the end", s, 0);
    close(s);
    return 0;
}
