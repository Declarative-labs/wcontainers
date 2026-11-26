// A connection to a server outside, disconnected with a connect to AF_UNSPEC while its descriptor
// stays open: the server hears a reset at once. A second connection asks the server what the first
// heard. Arguments: the server's name and port.
#define _GNU_SOURCE
#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

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
    int first = socket(AF_INET, SOCK_STREAM, 0);
    printf("connect %d\n", connect(first, (void *)&to, sizeof to));
    struct sockaddr unspecified = {.sa_family = AF_UNSPEC};
    printf("connect unspecified %d\n", connect(first, &unspecified, sizeof unspecified));
    char bytes[32];
    long n = recv(first, bytes, sizeof bytes, MSG_DONTWAIT);
    printf("receive after: %ld %s\n", n, n < 0 ? strerror(errno) : "");
    int second = socket(AF_INET, SOCK_STREAM, 0);
    printf("second connect %d\n", connect(second, (void *)&to, sizeof to));
    n = recv(second, bytes, sizeof bytes, MSG_WAITALL);
    printf("the server heard: %.*s\n", n > 0 ? (int)n : 0, bytes);
    close(second);
    close(first);
    return 0;
}
