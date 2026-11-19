// MSG_MORE and TCP_CORK on a loopback TCP connection: bytes are held back until a send without
// MSG_MORE, the cork's removal, TCP_NODELAY set, the end of the connection, or a whole segment's
// worth; what the receiver and the sender's queue show.

#define _GNU_SOURCE
#include <errno.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <unistd.h>
#include <linux/sockios.h>

static char buffer[100000];

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

static void pair(int *client, int *server) {
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in address = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
    socklen_t length = sizeof address;
    bind(listener, (struct sockaddr *)&address, sizeof address);
    getsockname(listener, (struct sockaddr *)&address, &length);
    listen(listener, 1);
    *client = socket(AF_INET, SOCK_STREAM, 0);
    connect(*client, (struct sockaddr *)&address, sizeof address);
    *server = accept(listener, NULL, NULL);
    close(listener);
}

static void option(const char *what, int fd, int name, int value) {
    result(what, setsockopt(fd, IPPROTO_TCP, name, &value, sizeof value));
}

static void queue(const char *what, int fd) {
    int queued = -1, cork = -1;
    socklen_t length = sizeof cork;
    ioctl(fd, SIOCOUTQ, &queued);
    getsockopt(fd, IPPROTO_TCP, TCP_CORK, &cork, &length);
    printf("%s: sender queue %d cork %d\n", what, queued, cork);
}

static void drain(const char *what, int fd) {
    int total = 0;
    long got;
    while ((got = recv(fd, buffer, sizeof buffer, MSG_DONTWAIT)) > 0) {
        total += got;
    }
    printf("%s took %d\n", what, total);
}

static void waiting(const char *what, int fd) {
    int count = -1;
    usleep(20000);
    ioctl(fd, FIONREAD, &count);
    printf("%s readable %d\n", what, count);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int client, server;
    pair(&client, &server);
    memset(buffer, 'm', sizeof buffer);
    result("send", send(client, buffer, 16, 0));
    waiting("plain", server);
    result("recv", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));
    result("send more", send(client, buffer, 16, MSG_MORE));
    waiting("held", server);
    int queued = -1;
    ioctl(client, SIOCOUTQ, &queued);
    printf("sender queue %d\n", queued);
    result("recv held", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));
    result("send more again", send(client, buffer, 5, MSG_MORE));
    result("send nothing", send(client, buffer, 0, 0));
    waiting("after nothing", server);
    result("send one", send(client, buffer, 1, 0));
    waiting("pushed", server);
    result("recv", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));

    // A whole segment's worth goes, and with it the rest.
    result("send a segment more", send(client, buffer, 70000, MSG_MORE));
    usleep(20000);
    int total = 0;
    long got;
    while ((got = recv(server, buffer, sizeof buffer, MSG_DONTWAIT)) > 0) {
        total += got;
    }
    printf("took %d\n", total);

    result("send more before shutdown", send(client, buffer, 10, MSG_MORE));
    result("shutdown", shutdown(client, SHUT_WR));
    waiting("after shutdown", server);
    result("recv", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));
    result("recv end", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));
    close(client);
    close(server);

    pair(&client, &server);
    result("send more before close", send(client, buffer, 7, MSG_MORE));
    close(client);
    waiting("after close", server);
    result("recv", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));
    result("recv end", recv(server, buffer, sizeof buffer, MSG_DONTWAIT));
    close(server);

    pair(&client, &server);
    option("cork", client, TCP_CORK, 1);
    result("send corked", send(client, buffer, 16, 0));
    waiting("corked", server);
    queue("corked", client);
    result("send corked again", send(client, buffer, 5, 0));
    waiting("corked again", server);
    usleep(400000);
    waiting("corked after the probe timer", server);
    drain("the probe timer", server);
    result("send corked once more", send(client, buffer, 2, 0));
    option("uncork", client, TCP_CORK, 0);
    waiting("uncorked", server);
    queue("uncorked", client);
    drain("uncorked", server);
    result("send more", send(client, buffer, 8, MSG_MORE));
    usleep(400000);
    waiting("more after the probe timer", server);
    drain("more after the probe timer", server);
    option("cork", client, TCP_CORK, 1);
    result("send corked", send(client, buffer, 10, 0));
    option("nodelay while corked", client, TCP_NODELAY, 1);
    waiting("nodelay while corked", server);
    drain("nodelay while corked", server);
    result("send corked with nodelay", send(client, buffer, 3, 0));
    waiting("corked with nodelay", server);
    option("uncork", client, TCP_CORK, 0);
    waiting("uncorked with nodelay", server);
    drain("uncorked with nodelay", server);
    option("nodelay off", client, TCP_NODELAY, 0);
    result("send more", send(client, buffer, 4, MSG_MORE));
    option("nodelay after more", client, TCP_NODELAY, 1);
    waiting("nodelay after more", server);
    drain("nodelay after more", server);
    option("cork", client, TCP_CORK, 1);
    result("send corked before shutdown", send(client, buffer, 6, 0));
    result("shutdown", shutdown(client, SHUT_WR));
    waiting("corked shutdown", server);
    drain("corked shutdown", server);
    close(client);
    close(server);
    return 0;
}
