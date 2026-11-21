// sendfile and splice with sockets at either end: a file into TCP and local stream and datagram
// sockets, a pipe into a socket and a socket into a pipe, with offsets, lengths past the end,
// full buffers and nonblocking ends.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/sendfile.h>
#include <sys/socket.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static void on_alarm(int sig) { (void)sig; }

// A sendfile of a large file into a blocking local socket waits for the reader in turns.
static void blocking(void) {
    int big = open("/tmp/big", O_CREAT | O_RDWR | O_TRUNC, 0644);
    static char chunk[65536];
    for (int i = 0; i < 16; i++) {
        memset(chunk, 'A' + i, sizeof chunk);
        write(big, chunk, sizeof chunk);
    }
    int pair[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, pair);
    pid_t child = fork();
    if (child == 0) {
        close(pair[0]);
        long total = 0, n;
        char buf[4096];
        while ((n = read(pair[1], buf, sizeof buf)) > 0)
            total += n;
        printf("reader got %ld\n", total);
        _exit(0);
    }
    close(pair[1]);
    off_t offset = 0;
    result("blocking sendfile", sendfile(pair[0], big, &offset, 16 * 65536));
    printf("offset %ld\n", (long)offset);
    // A pipe spliced into the blocking socket in rounds.
    int p[2];
    pipe(p);
    long spliced = 0;
    for (int i = 0; i < 8; i++) {
        memset(chunk, 'a' + i, sizeof chunk);
        write(p[1], chunk, sizeof chunk);
        long n = splice(p[0], 0, pair[0], 0, sizeof chunk, 0);
        if (n > 0)
            spliced += n;
    }
    printf("spliced %ld\n", spliced);
    close(pair[0]);
    waitpid(child, 0, 0);
    // SPLICE_F_NONBLOCK on a blocking socket with nothing to read.
    struct sigaction sa = {.sa_handler = on_alarm};
    sigaction(SIGALRM, &sa, 0);
    int q[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, q);
    alarm(1);
    result("splice flag on blocking unix", splice(q[0], 0, p[1], 0, 100, SPLICE_F_NONBLOCK));
    alarm(0);
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(listener, (void *)&a, sizeof a);
    socklen_t length = sizeof a;
    getsockname(listener, (void *)&a, &length);
    listen(listener, 1);
    int client = socket(AF_INET, SOCK_STREAM, 0);
    connect(client, (void *)&a, sizeof a);
    int server = accept(listener, 0, 0);
    alarm(1);
    result("splice flag on blocking tcp", splice(server, 0, p[1], 0, 100, SPLICE_F_NONBLOCK));
    alarm(0);
    int d[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, d);
    alarm(1);
    result("splice flag on blocking dgram", splice(d[0], 0, p[1], 0, 100, SPLICE_F_NONBLOCK));
    alarm(0);
}

static void drain(const char *what, int fd) {
    char buf[70000];
    long n = recv(fd, buf, sizeof buf, MSG_DONTWAIT);
    printf("%s got %ld", what, n);
    if (n > 0)
        printf(" first %.5s last %.5s", buf, buf + n - 5);
    printf("\n");
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int file = open("/tmp/data", O_CREAT | O_RDWR | O_TRUNC, 0644);
    char text[100000];
    for (int i = 0; i < (int)sizeof text; i++)
        text[i] = 'a' + i % 26;
    write(file, text, sizeof text);

    int listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    bind(listener, (void *)&a, sizeof a);
    socklen_t length = sizeof a;
    getsockname(listener, (void *)&a, &length);
    listen(listener, 1);
    int client = socket(AF_INET, SOCK_STREAM, 0);
    connect(client, (void *)&a, sizeof a);
    int server = accept(listener, 0, 0);

    off_t offset = 10;
    result("sendfile tcp", sendfile(client, file, &offset, 1000));
    printf("offset %ld\n", (long)offset);
    drain("tcp", server);
    lseek(file, 99990, SEEK_SET);
    result("sendfile tcp past end", sendfile(client, file, 0, 1000));
    printf("position %ld\n", (long)lseek(file, 0, SEEK_CUR));
    drain("tcp", server);
    result("sendfile from a socket", sendfile(client, server, 0, 10));

    int stream[2], dgram[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, stream);
    socketpair(AF_UNIX, SOCK_DGRAM, 0, dgram);
    offset = 0;
    result("sendfile unix stream", sendfile(stream[0], file, &offset, 5000));
    drain("unix stream", stream[1]);
    offset = 0;
    result("sendfile unix dgram", sendfile(dgram[0], file, &offset, 5000));
    drain("unix dgram 1", dgram[1]);
    drain("unix dgram 2", dgram[1]);

    // Filling a nonblocking stream socket.
    fcntl(stream[0], F_SETFL, O_NONBLOCK);
    long total = 0, n;
    offset = 0;
    while ((n = sendfile(stream[0], file, &offset, 100000)) > 0) {
        total += n;
        if (offset >= 100000)
            offset = 0;
    }
    printf("filled %d %s\n", total > 0, n < 0 ? strerror(errno) : "end");

    int pipefd[2];
    pipe(pipefd);
    write(pipefd[1], "from the pipe", 13);
    result("splice pipe to tcp", splice(pipefd[0], 0, client, 0, 100, 0));
    drain("tcp", server);
    send(client, "into the pipe", 13, 0);
    usleep(10000);
    result("splice tcp to pipe", splice(server, 0, pipefd[1], 0, 100, 0));
    char buf[64] = {0};
    result("pipe read", read(pipefd[0], buf, sizeof buf));
    printf("pipe has %s\n", buf);
    result("splice socket offset", splice(server, &offset, pipefd[1], 0, 100, 0));
    fcntl(server, F_SETFL, O_NONBLOCK);
    result("splice empty nonblocking", splice(server, 0, pipefd[1], 0, 100, 0));
    result("splice empty flag", splice(server, 0, pipefd[1], 0, 100, SPLICE_F_NONBLOCK));
    write(pipefd[1], "datagram", 8);
    result("splice pipe to dgram", splice(pipefd[0], 0, dgram[0], 0, 100, 0));
    drain("dgram", dgram[1]);
    shutdown(client, SHUT_WR);
    result("splice at end", splice(server, 0, pipefd[1], 0, 100, 0));
    blocking();
    return 0;
}
