// A connection to a server outside held open while the container may be saved and brought back:
// connects, gives way many times, then writes, finishes and reads. Arguments: the server's name
// and port.
#define _GNU_SOURCE
#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <sched.h>
#include <signal.h>
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

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    signal(SIGPIPE, SIG_IGN);
    struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
    if (argc < 3 || getaddrinfo(argv[1], argv[2], &hints, &found))
        return 2;
    int s = socket(AF_INET, SOCK_STREAM, 0);
    result("connect", connect(s, found->ai_addr, found->ai_addrlen));
    for (int i = 0; i < 400; i++)
        sched_yield();
    result("write", write(s, "x", 1));
    result("shutdown", shutdown(s, SHUT_WR));
    char buf[64];
    long n = read(s, buf, sizeof buf);
    result("read", n);
    if (n > 0)
        printf("got %.*s\n", (int)n, buf);
    return 0;
}
