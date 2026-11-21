// TCP_DEFER_ACCEPT: the value as seconds of whole SYN-ACK retransmits, a listener holding a
// connection back until data or a FIN comes or the deferral ends, the held connection shown as
// SYN_RECV in /proc/net/tcp, requests past the backlog going straight to the accept queue (as
// SYN cookies), a reset dropping a held connection, and an accepted socket's own value.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

static double now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec / 1e9;
}

static struct sockaddr_in at(int port) {
    struct sockaddr_in a = {.sin_family = AF_INET, .sin_port = htons(port), .sin_addr.s_addr = htonl(0x7f000001)};
    return a;
}

static int listener(int port, int defer, int backlog) {
    int l = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in a = at(port);
    bind(l, (void *)&a, sizeof a);
    setsockopt(l, IPPROTO_TCP, TCP_DEFER_ACCEPT, &defer, sizeof defer);
    listen(l, backlog);
    return l;
}

static int client(int port, int flags) {
    int c = socket(AF_INET, SOCK_STREAM | flags, 0);
    struct sockaddr_in a = at(port);
    int r = connect(c, (void *)&a, sizeof a);
    if (r) printf("connect: %s\n", strerror(errno));
    return c;
}

// The states /proc/net/tcp shows for the connections to a port, the listener's queue, and for
// requests the timer and retransmits.
static void states(int port) {
    char buf[16384], text[8];
    int f = open("/proc/net/tcp", O_RDONLY), n, got = 0;
    while ((n = read(f, buf + got, sizeof buf - 1 - got)) > 0) got += n;
    buf[got] = 0;
    close(f);
    snprintf(text, sizeof text, ":%04X", port);
    int listening = 0, established = 0, requests = 0;
    for (char *l = strtok(buf, "\n"); l; l = strtok(NULL, "\n")) {
        char local[64], remote[64], st[8], queues[32], timer[32], retransmits[16];
        if (sscanf(l, "%*s %63s %63s %7s %31s %31s %15s", local, remote, st, queues, timer, retransmits) != 6) continue;
        int to_port = strstr(remote, text) != NULL, at_port = strstr(local, text) != NULL;
        if (at_port && !strcmp(st, "0A")) {
            listening++;
            printf("  listener queue %s\n", queues);
        } else if (at_port && !strcmp(st, "03")) {
            requests++;
            printf("  request timer %.3s retransmits %s\n", timer, retransmits);
        } else if (at_port && !strcmp(st, "01"))
            established++;
        (void)to_port;
    }
    printf("  listening %d, established %d, requests %d\n", listening, established, requests);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int values[] = {0, 1, 2, 3, 4, 5, 7, 8, 100, 1000, 100000, -5, 255 * 120};
    int s = socket(AF_INET, SOCK_STREAM, 0);
    for (unsigned i = 0; i < sizeof values / sizeof *values; i++) {
        int v = values[i], got = -1;
        socklen_t len = sizeof got;
        int r = setsockopt(s, IPPROTO_TCP, TCP_DEFER_ACCEPT, &v, sizeof v);
        getsockopt(s, IPPROTO_TCP, TCP_DEFER_ACCEPT, &got, &len);
        printf("set %d: %d, reads %d\n", v, r, got);
    }
    char small = 1;
    errno = 0;
    printf("one byte: %d %s\n", setsockopt(s, IPPROTO_TCP, TCP_DEFER_ACCEPT, &small, 1), strerror(errno));

    // No data: the connection is accepted when its deferral ends.
    for (int defer = 1; defer <= 3; defer += 2) {
        int l = listener(7101 + defer, defer, 4);
        double start = now();
        int c = client(7101 + defer, 0);
        struct pollfd p = {l, POLLIN, 0};
        printf("defer %d: listener ready at once %d\n", defer, poll(&p, 1, 0));
        states(7101 + defer);
        int a = accept(l, NULL, NULL);
        printf("defer %d: accepted after %.0f s\n", defer, now() - start);
        char b[8];
        fcntl(a, F_SETFL, O_NONBLOCK);
        errno = 0;
        printf("  read %d %s\n", (int)read(a, b, 8), strerror(errno));
        states(7101 + defer);
        close(a), close(c), close(l);
    }
    // Data before the deferral ends.
    {
        int l = listener(7110, 5, 4);
        double start = now();
        int c = client(7110, 0);
        usleep(500000);
        write(c, "hi", 2);
        int a = accept(l, NULL, NULL);
        char b[8];
        printf("data: accepted before the deferral ended %d, read %d\n", now() - start < 6.5, (int)read(a, b, 8));
        int got = -1;
        socklen_t len = sizeof got;
        getsockopt(a, IPPROTO_TCP, TCP_DEFER_ACCEPT, &got, &len);
        printf("  accepted socket's value %d\n", got);
        close(a), close(c), close(l);
    }
    // A FIN before the deferral ends.
    {
        int l = listener(7111, 5, 4);
        double start = now();
        int c = client(7111, 0);
        usleep(300000);
        shutdown(c, SHUT_WR);
        int a = accept(l, NULL, NULL);
        char b[8];
        printf("fin: accepted before the deferral ended %d, read %d\n", now() - start < 6.5, (int)read(a, b, 8));
        close(a), close(c), close(l);
    }
    // A reset drops the held connection: nothing to accept after the deferral.
    {
        int l = listener(7112, 1, 4);
        int c = client(7112, 0);
        struct linger lg = {1, 0};
        setsockopt(c, SOL_SOCKET, SO_LINGER, &lg, sizeof lg);
        close(c);
        usleep(1500000);
        struct pollfd p = {l, POLLIN, 0};
        printf("reset: listener ready after the deferral %d\n", poll(&p, 1, 0));
        states(7112);
        close(l);
    }
    // Past the backlog's requests, connections are not held back.
    {
        int l = listener(7113, 30, 2);
        int cs[6];
        for (int i = 0; i < 6; i++) cs[i] = client(7113, SOCK_NONBLOCK);
        usleep(100000);
        for (int i = 0; i < 6; i++) {
            struct pollfd p = {cs[i], POLLOUT, 0};
            poll(&p, 1, 0);
            printf("client %d revents %x\n", i, p.revents);
        }
        states(7113);
        write(cs[0], "x", 1);
        usleep(100000);
        states(7113);
        int a = accept4(l, NULL, NULL, SOCK_NONBLOCK);
        char b[8];
        errno = 0;
        printf("first accepted reads %d %s\n", (int)read(a, b, 8), strerror(errno));
        // Held requests go without a word; client 0's data, dropped while the queue was full,
        // is sent again (after 200 ms) to the closed port and draws a reset, as does any
        // client's next send.
        close(l);
        usleep(600000);
        for (int i = 0; i < 6; i++) {
            errno = 0;
            int r = (int)write(cs[i], "y", 1);
            printf("client %d after the listener closed: write %d %s\n", i, r, r < 0 ? strerror(errno) : "");
        }
        usleep(100000);
        for (int i = 0; i < 6; i++) {
            errno = 0;
            int r = (int)read(cs[i], b, 8);
            printf("client %d then reads %d %s\n", i, r, r < 0 ? strerror(errno) : "");
        }
    }
    return 0;
}
