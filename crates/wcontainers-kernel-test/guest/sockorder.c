// Answers layer 1 found differing: netlink's level checks before its arguments, its refusal of
// an empty message, and a nonblocking read of a UDP socket shut for reading.
#include <errno.h>
#include <linux/netlink.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static void result(const char *what, long r) {
    printf("%s: %ld %s\n", what, r, r < 0 ? strerror(errno) : "");
}

int main(void) {
    void *bad = (void *)0xfffffffffffff000UL;
    int n = socket(AF_NETLINK, SOCK_DGRAM, NETLINK_ROUTE);
    socklen_t length = (socklen_t)-1;
    result("get other level bad value", getsockopt(n, SOL_IPV6, 26, bad, &length));
    result("get other level bad length", getsockopt(n, SOL_IPV6, 26, bad, bad));
    result("get netlink level bad length", getsockopt(n, SOL_NETLINK, 1, bad, bad));
    result("set other level bad value", setsockopt(n, SOL_IP, 2, bad, 4));
    result("set netlink level bad value", setsockopt(n, SOL_NETLINK, 1, bad, 4));
    result("write nothing", write(n, "", 0));
    result("send nothing oob", send(n, "", 0, MSG_OOB));
    int u = socket(AF_INET, SOCK_DGRAM | SOCK_NONBLOCK, 0);
    result("udp shutdown unconnected", shutdown(u, SHUT_RDWR));
    char buf[8];
    result("udp read shut nonblocking", recv(u, buf, sizeof buf, 0));
    result("udp read shut waiting", recv(u, buf, sizeof buf, 0) < 0 ? -1 : 0);
    int w = socket(AF_INET, SOCK_DGRAM, 0);
    shutdown(w, SHUT_RD);
    result("udp read shut blocking", recv(w, buf, sizeof buf, 0));
    result("udp read shut dontwait", recv(w, buf, sizeof buf, MSG_DONTWAIT));
    return 0;
}
