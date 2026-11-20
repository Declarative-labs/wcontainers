// Sends a server outside one byte at a time and reads each byte's echo with a blocking read, so
// that the echo often arrives while the read waits. Arguments: the server's name, its TCP port and
// how many bytes.
#define _GNU_SOURCE
#include <netdb.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <unistd.h>

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc < 4)
        return 2;
    struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
    if (getaddrinfo(argv[1], NULL, &hints, &found) != 0)
        return 3;
    struct sockaddr_in to = *(struct sockaddr_in *)found->ai_addr;
    freeaddrinfo(found);
    to.sin_port = htons(atoi(argv[2]));
    long count = atol(argv[3]);
    int s = socket(AF_INET, SOCK_STREAM, 0);
    int on = 1;
    setsockopt(s, IPPROTO_TCP, TCP_NODELAY, &on, sizeof on);
    printf("connect %d\n", connect(s, (void *)&to, sizeof to));
    long echoed = 0, wrong = 0;
    for (long i = 0; i < count; i++) {
        unsigned char sent = (unsigned char)i, got;
        if (write(s, &sent, 1) != 1)
            break;
        if (read(s, &got, 1) != 1)
            break;
        echoed++;
        wrong += got != sent;
    }
    shutdown(s, SHUT_WR);
    unsigned char rest;
    printf("echoed %ld wrong %ld end %zd\n", echoed, wrong, read(s, &rest, 1));
    close(s);
    return 0;
}
