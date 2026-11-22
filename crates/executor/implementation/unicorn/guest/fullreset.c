// A connection to the outside whose peer fills its receive queue and then resets it, while the
// program reads nothing; a second connection tells when the reset was sent. The program then
// polls the first, reads its pending error and reads it to its end.
// Arguments: the server's name and port.
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

static void show(const char *what, int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLRDHUP};
    int r = poll(&p, 1, 0);
    printf("%s: poll %d%s%s%s%s%s\n", what, r, p.revents & POLLIN ? " in" : "",
           p.revents & POLLOUT ? " out" : "", p.revents & POLLERR ? " err" : "",
           p.revents & POLLHUP ? " hup" : "", p.revents & POLLRDHUP ? " rdhup" : "");
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc < 3)
        return 2;
    struct sockaddr_in to = server(argv[1], atoi(argv[2]));
    int data = socket(AF_INET, SOCK_STREAM, 0);
    printf("connect data %d\n", connect(data, (void *)&to, sizeof to));
    int signal = socket(AF_INET, SOCK_STREAM, 0);
    printf("connect signal %d\n", connect(signal, (void *)&to, sizeof to));
    char word[16];
    long n = read(signal, word, sizeof word);
    printf("signal read %ld %.*s\n", n, n > 0 ? (int)n : 0, word);
    show("after reset", data);
    int error = -1;
    socklen_t length = sizeof error;
    getsockopt(data, SOL_SOCKET, SO_ERROR, &error, &length);
    printf("so_error %s\n", error ? strerror(error) : "none");
    char buffer[65536];
    long total = 0;
    while ((n = read(data, buffer, sizeof buffer)) > 0)
        total += n;
    printf("read ends %ld %s, some data %s\n", n, n < 0 ? strerror(errno) : "eof",
           total > 0 ? "yes" : "no");
    return 0;
}
