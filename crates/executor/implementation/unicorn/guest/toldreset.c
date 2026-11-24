// Connections to the outside, each reset by its peer just after the peer said so on a second
// connection. A moment later the program looks at the reset connection once, by poll, by select
// or by SO_ERROR, one way for each pair, before anything else touches it.
// Arguments: the server's name and port.
#define _GNU_SOURCE
#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <time.h>
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

static void by_poll(int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLRDHUP};
    int r = poll(&p, 1, 0);
    printf("poll %d%s%s%s%s%s\n", r, p.revents & POLLIN ? " in" : "",
           p.revents & POLLOUT ? " out" : "", p.revents & POLLERR ? " err" : "",
           p.revents & POLLHUP ? " hup" : "", p.revents & POLLRDHUP ? " rdhup" : "");
}

static void by_select(int fd) {
    fd_set readable, writable, exceptional;
    FD_ZERO(&readable);
    FD_ZERO(&writable);
    FD_ZERO(&exceptional);
    FD_SET(fd, &readable);
    FD_SET(fd, &writable);
    FD_SET(fd, &exceptional);
    struct timeval none = {0, 0};
    int r = select(fd + 1, &readable, &writable, &exceptional, &none);
    printf("select %d%s%s%s\n", r, FD_ISSET(fd, &readable) ? " readable" : "",
           FD_ISSET(fd, &writable) ? " writable" : "",
           FD_ISSET(fd, &exceptional) ? " exceptional" : "");
}

static void by_so_error(int fd) {
    int error = -1;
    socklen_t length = sizeof error;
    int r = getsockopt(fd, SOL_SOCKET, SO_ERROR, &error, &length);
    printf("so_error %d %s\n", r, error ? strerror(error) : "none");
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc < 3)
        return 2;
    struct sockaddr_in to = server(argv[1], atoi(argv[2]));
    void (*ways[])(int) = {by_poll, by_select, by_so_error};
    for (unsigned way = 0; way < sizeof ways / sizeof *ways; way++) {
        int data = socket(AF_INET, SOCK_STREAM, 0);
        printf("connect data %d\n", connect(data, (void *)&to, sizeof to));
        int signal = socket(AF_INET, SOCK_STREAM, 0);
        printf("connect signal %d\n", connect(signal, (void *)&to, sizeof to));
        printf("asked %ld\n", (long)write(signal, "?", 1));
        char word[16];
        long n = read(signal, word, sizeof word);
        printf("told %ld %.*s\n", n, n > 0 ? (int)n : 0, word);
        // The reset follows the word at once; a moment lets it reach this machine.
        struct timespec moment = {0, 200 * 1000 * 1000};
        nanosleep(&moment, NULL);
        ways[way](data);
        close(data);
        close(signal);
    }
    return 0;
}
