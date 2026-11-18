// A TCP socket disconnected with AF_UNSPEC lets go of the address and port its program did not
// choose, and an accepted socket chose what its listener's program chose: it may then be bound
// again. With IPV6_ADDRFORM first, as cve-2018-9568's test does.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static void p(const char *what, int r) { printf("%s: %d %s\n", what, r, r < 0 ? strerror(errno) : ""); }

static void round(int choose_port) {
    printf("listener %s its port\n", choose_port ? "chose" : "did not choose");
    int listener = socket(AF_INET6, SOCK_STREAM, 0);
    struct sockaddr_in6 any6 = {.sin6_family = AF_INET6, .sin6_addr = in6addr_any};
    if (choose_port) any6.sin6_port = htons(40123);
    p("bind listener", bind(listener, (void *)&any6, sizeof any6));
    socklen_t length = sizeof any6;
    getsockname(listener, (void *)&any6, &length);
    p("listen", listen(listener, 2));
    struct sockaddr_in to = {.sin_family = AF_INET, .sin_port = any6.sin6_port};
    inet_pton(AF_INET, "127.0.0.1", &to.sin_addr);
    int client = socket(AF_INET, SOCK_STREAM, 0);
    p("connect", connect(client, (void *)&to, sizeof to));
    int accepted = accept(listener, NULL, NULL);
    int family = AF_INET;
    p("addrform", setsockopt(accepted, SOL_IPV6, IPV6_ADDRFORM, &family, sizeof family));
    struct sockaddr unspecified = {.sa_family = AF_UNSPEC};
    p("connect unspecified", connect(accepted, &unspecified, sizeof unspecified));
    struct sockaddr_in name;
    length = sizeof name;
    getsockname(accepted, (void *)&name, &length);
    printf("after: port kept %d address %s\n", name.sin_port == any6.sin6_port, inet_ntoa(name.sin_addr));
    struct sockaddr_in any4 = {.sin_family = AF_INET};
    p("bind again", bind(accepted, (void *)&any4, sizeof any4));
    p("listen again", listen(accepted, 2));
    close(accepted);
    close(client);
    close(listener);
    // A connection the program made itself, its port given at connect.
    listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in here = {.sin_family = AF_INET};
    inet_pton(AF_INET, "127.0.0.1", &here.sin_addr);
    bind(listener, (void *)&here, sizeof here);
    length = sizeof here;
    getsockname(listener, (void *)&here, &length);
    listen(listener, 2);
    client = socket(AF_INET, SOCK_STREAM, 0);
    connect(client, (void *)&here, sizeof here);
    p("client connect unspecified", connect(client, &unspecified, sizeof unspecified));
    length = sizeof name;
    getsockname(client, (void *)&name, &length);
    printf("client after: port shown %d address %s\n", name.sin_port != 0, inet_ntoa(name.sin_addr));
    p("client bind", bind(client, (void *)&any4, sizeof any4));
    close(client);
    close(listener);
}

// An IPv4 listener bound to an address and a port it chose.
static void bound(void) {
    printf("listener chose its address and port\n");
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in here = {.sin_family = AF_INET, .sin_port = htons(40125)};
    inet_pton(AF_INET, "127.0.0.1", &here.sin_addr);
    p("bind listener", bind(listener, (void *)&here, sizeof here));
    p("listen", listen(listener, 2));
    int client = socket(AF_INET, SOCK_STREAM, 0);
    p("connect", connect(client, (void *)&here, sizeof here));
    int accepted = accept(listener, NULL, NULL);
    struct sockaddr unspecified = {.sa_family = AF_UNSPEC};
    p("connect unspecified", connect(accepted, &unspecified, sizeof unspecified));
    struct sockaddr_in name;
    socklen_t length = sizeof name;
    getsockname(accepted, (void *)&name, &length);
    printf("after: port %d address %s\n", ntohs(name.sin_port), inet_ntoa(name.sin_addr));
    struct sockaddr_in any4 = {.sin_family = AF_INET};
    p("bind again", bind(accepted, (void *)&any4, sizeof any4));
    close(accepted);
    close(client);
    close(listener);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    round(0);
    round(1);
    bound();
    return 0;
}
