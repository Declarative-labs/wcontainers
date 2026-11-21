// Waits of one kind, repeated, for timing what a wait costs: a byte back and forth over two pipes
// between two processes, a byte back and forth with a server outside, a file opened and read whole,
// and a name looked up. Arguments: pipe COUNT | socket NAME PORT COUNT | file PATH COUNT |
// name NAME COUNT.
#include <fcntl.h>
#include <netdb.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>

static int fail(const char *what) {
    perror(what);
    return 1;
}

static int pipes(long count) {
    int there[2], back[2];
    if (pipe(there) || pipe(back))
        return fail("pipe");
    pid_t child = fork();
    if (child < 0)
        return fail("fork");
    char byte = 0;
    if (child == 0) {
        for (long i = 0; i < count; i++) {
            if (read(there[0], &byte, 1) != 1 || write(back[1], &byte, 1) != 1)
                _exit(1);
        }
        _exit(0);
    }
    for (long i = 0; i < count; i++) {
        byte = (char)i;
        if (write(there[1], &byte, 1) != 1 || read(back[0], &byte, 1) != 1)
            return fail("ping");
    }
    int status;
    waitpid(child, &status, 0);
    return status != 0;
}

static struct sockaddr_in address(const char *name, int port) {
    struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
    struct sockaddr_in a = {0};
    if (getaddrinfo(name, NULL, &hints, &found) == 0) {
        a = *(struct sockaddr_in *)found->ai_addr;
        freeaddrinfo(found);
    }
    a.sin_port = htons(port);
    return a;
}

static int socket_bytes(const char *name, int port, long count) {
    struct sockaddr_in a = address(name, port);
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0 || connect(fd, (struct sockaddr *)&a, sizeof a))
        return fail("connect");
    char byte = 0;
    for (long i = 0; i < count; i++) {
        byte = (char)i;
        if (write(fd, &byte, 1) != 1 || read(fd, &byte, 1) != 1)
            return fail("echo");
    }
    close(fd);
    return 0;
}

static int file_reads(const char *path, long count) {
    char buf[4096];
    for (long i = 0; i < count; i++) {
        int fd = open(path, O_RDONLY);
        if (fd < 0)
            return fail("open");
        while (read(fd, buf, sizeof buf) > 0) {
        }
        close(fd);
    }
    return 0;
}

static int names(const char *name, long count) {
    for (long i = 0; i < count; i++) {
        struct addrinfo hints = {.ai_family = AF_INET, .ai_socktype = SOCK_STREAM}, *found;
        int r = getaddrinfo(name, NULL, &hints, &found);
        if (r)
            return fprintf(stderr, "lookup: %s\n", gai_strerror(r)), 1;
        freeaddrinfo(found);
    }
    return 0;
}

int main(int argc, char **argv) {
    int r = 2;
    if (argc == 3 && !strcmp(argv[1], "pipe"))
        r = pipes(atol(argv[2]));
    else if (argc == 5 && !strcmp(argv[1], "socket"))
        r = socket_bytes(argv[2], atoi(argv[3]), atol(argv[4]));
    else if (argc == 4 && !strcmp(argv[1], "file"))
        r = file_reads(argv[2], atol(argv[3]));
    else if (argc == 4 && !strcmp(argv[1], "name"))
        r = names(argv[2], atol(argv[3]));
    printf("%s %s\n", argv[1], r ? "failed" : "done");
    return r;
}
