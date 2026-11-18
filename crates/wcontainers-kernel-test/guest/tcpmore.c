// MSG_MORE on a loopback TCP connection: bytes are held back until a send without it, the end of
// the connection, or a whole segment's worth; what the receiver and the sender's queue show.

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
    return 0;
}
