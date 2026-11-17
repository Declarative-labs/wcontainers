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

static char buf_for_none[1];

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
    // Before the control messages are read: a closed descriptor among them is not seen.
    char space[CMSG_SPACE(sizeof(int))] = {0};
    struct iovec none = {.iov_base = buf_for_none, .iov_len = 0};
    struct msghdr msg = {.msg_iov = &none, .msg_iovlen = 1, .msg_control = space,
                         .msg_controllen = sizeof space};
    struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    c->cmsg_len = CMSG_LEN(sizeof(int));
    int closed = 99;
    memcpy(CMSG_DATA(c), &closed, sizeof closed);
    result("sendmsg nothing with a closed descriptor", sendmsg(n, &msg, 0));
    none.iov_len = 1;
    result("sendmsg a byte with a closed descriptor", sendmsg(n, &msg, 0));
    // A read at the end gives credentials and a process descriptor only on a stream.
    int kinds[] = {SOCK_SEQPACKET, SOCK_DGRAM, SOCK_STREAM};
    for (int k = 0; k < 3; k++) {
        int sv[2];
        socketpair(AF_UNIX, kinds[k] | SOCK_NONBLOCK, 0, sv);
        int on = 1;
        setsockopt(sv[1], SOL_SOCKET, SO_PASSCRED, &on, sizeof on);
        setsockopt(sv[1], SOL_SOCKET, 76, &on, sizeof on);
        shutdown(sv[0], SHUT_WR);
        close(sv[0]);
        char byte;
        struct iovec iov = {.iov_base = &byte, .iov_len = 1};
        for (int room = 0; room <= 64; room += 64) {
            char control[64];
            struct msghdr m = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control,
                               .msg_controllen = room};
            long r = recvmsg(sv[1], &m, MSG_DONTWAIT);
            printf("kind %d end room %d: %ld %s flags %#x controllen %zu\n", kinds[k], room, r,
                   r < 0 ? strerror(errno) : "", m.msg_flags, (size_t)m.msg_controllen);
        }
        close(sv[1]);
    }
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
