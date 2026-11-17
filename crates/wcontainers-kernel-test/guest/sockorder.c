// Answers layer 1 found differing: netlink's level checks before its arguments, its refusal of
// an empty message, a nonblocking read of a UDP socket shut for reading, a local socket's end
// with credentials asked for, and sendfile from a socket into a pipe.
#include <errno.h>
#include <fcntl.h>
#include <sys/sendfile.h>
#include <linux/netlink.h>
#include <netinet/in.h>
#include <netinet/udp.h>
#include <arpa/inet.h>
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
    // sendfile from a socket into a pipe reads the socket (`sock_splice_read`).
    int kinds2[] = {SOCK_DGRAM, SOCK_STREAM, SOCK_SEQPACKET};
    for (int k = 0; k < 3; k++) {
        int sv[2], p[2];
        socketpair(AF_UNIX, kinds2[k] | SOCK_NONBLOCK, 0, sv);
        pipe2(p, O_NONBLOCK);
        result("sendfile from an empty socket", sendfile(p[1], sv[0], NULL, 1UL << 40));
        send(sv[1], "hello", 5, 0);
        result("sendfile from a socket", sendfile(p[1], sv[0], NULL, 3));
        char got[8] = {0};
        result("pipe read", read(p[0], got, sizeof got));
        printf("got %s\n", got);
        off_t at = 0;
        result("sendfile from a socket at an offset", sendfile(p[1], sv[0], &at, 3));
        close(sv[0]);
        close(sv[1]);
        close(p[0]);
        close(p[1]);
    }
    result("netlink shutdown of no kind", shutdown(n, 3));
    // A peek of local datagrams starts at SO_PEEK_OFF, past whole datagrams.
    {
        int sv[2];
        socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_NONBLOCK, 0, sv);
        send(sv[0], "abcdef", 6, 0);
        send(sv[0], "gh", 2, 0);
        int off = 4;
        setsockopt(sv[1], SOL_SOCKET, SO_PEEK_OFF, &off, sizeof off);
        char got[16] = {0};
        result("peek at 4", recv(sv[1], got, sizeof got, MSG_PEEK));
        printf("got %s\n", got);
        memset(got, 0, sizeof got);
        result("peek on", recv(sv[1], got, sizeof got, MSG_PEEK));
        printf("got %s\n", got);
        result("peek past all", recv(sv[1], got, 0, MSG_PEEK));
        result("read", recv(sv[1], got, sizeof got, 0));
        socklen_t length = sizeof off;
        getsockopt(sv[1], SOL_SOCKET, SO_PEEK_OFF, &off, &length);
        printf("offset after read %d\n", off);
        close(sv[0]);
        close(sv[1]);
    }
    // UDP corking with MSG_MORE and UDP_CORK.
    {
        int c = socket(AF_INET, SOCK_DGRAM | SOCK_NONBLOCK, 0);
        struct sockaddr_in self = {.sin_family = AF_INET, .sin_port = htons(5191),
                                   .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
        bind(c, (struct sockaddr *)&self, sizeof self);
        char got[16] = {0};
        result("more ab", sendto(c, "ab", 2, MSG_MORE, (struct sockaddr *)&self, sizeof self));
        result("nothing yet", recv(c, got, sizeof got, 0));
        result("cd to nowhere", send(c, "cd", 2, 0));
        result("read whole", recv(c, got, sizeof got, 0));
        printf("got %s\n", got);
        result("more ef", sendto(c, "ef", 2, MSG_MORE, (struct sockaddr *)&self, sizeof self));
        result("bad buffer", send(c, bad, 2, 0));
        result("dropped", recv(c, got, sizeof got, 0));
        int on = 1, value = -1;
        socklen_t length = sizeof value;
        result("cork", setsockopt(c, SOL_UDP, 1, &on, sizeof on));
        getsockopt(c, SOL_UDP, 1, &value, &length);
        printf("cork value %d\n", value);
        sendto(c, "gh", 2, 0, (struct sockaddr *)&self, sizeof self);
        sendto(c, "ij", 2, 0, (struct sockaddr *)&self, sizeof self);
        result("corked", recv(c, got, sizeof got, 0));
        on = 0;
        result("uncork", setsockopt(c, SOL_UDP, 1, &on, sizeof on));
        memset(got, 0, sizeof got);
        result("read uncorked", recv(c, got, sizeof got, 0));
        printf("got %s\n", got);
        result("no check tx", setsockopt(c, SOL_UDP, 101, &on, sizeof on));
        result("unknown udp option", setsockopt(c, SOL_UDP, 99, &on, sizeof on));
        close(c);
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
