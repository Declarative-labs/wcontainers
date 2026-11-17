// Local socket options that hand over process descriptors and queue sizes: SO_PEERPIDFD,
// SO_PASSPIDFD with SCM_PIDFD, SO_PASSRIGHTS, and SO_INQ with SCM_INQ.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>

#ifndef SO_PASSPIDFD
#define SO_PASSPIDFD 76
#define SO_PEERPIDFD 77
#endif
#ifndef SCM_PIDFD
#define SCM_PIDFD 0x04
#endif
#ifndef SO_PASSRIGHTS
#define SO_PASSRIGHTS 83
#endif
#ifndef SO_INQ
#define SO_INQ 84
#define SCM_INQ SO_INQ
#endif

struct pidfd_info {
    uint64_t mask;
    uint64_t cgroupid;
    uint32_t pid, tgid, ppid, ruid, rgid, euid, egid, suid, sgid, fsuid, fsgid;
    int32_t exit_code;
    uint32_t coredump_mask, pad;
};
#define PIDFD_GET_INFO _IOWR(0xFF, 11, struct pidfd_info)

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

// Who a process descriptor names, as this process, its parent, a child or someone else.
static const char *names(int fd, pid_t child) {
    struct pidfd_info info = {.mask = 1};
    if (ioctl(fd, PIDFD_GET_INFO, &info) < 0)
        return errno == ESRCH ? "a process gone" : "not a process descriptor";
    if ((pid_t)info.tgid == getpid())
        return "this process";
    if ((pid_t)info.tgid == child)
        return "the child";
    return "another process";
}

static void peer_descriptor(const char *what, int fd, socklen_t room, pid_t child) {
    int pidfd = -1;
    socklen_t length = room;
    long r = getsockopt(fd, SOL_SOCKET, SO_PEERPIDFD, &pidfd, &length);
    if (r < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
        return;
    }
    printf("%s: %ld length %u names %s close on exec %d\n", what, r, length, names(pidfd, child),
           fcntl(pidfd, F_GETFD) & FD_CLOEXEC);
    close(pidfd);
}

static void show_control(const char *what, struct msghdr *msg, long r, pid_t child) {
    printf("%s: %ld flags %#x controllen %zu", what, r, msg->msg_flags, (size_t)msg->msg_controllen);
    for (struct cmsghdr *c = CMSG_FIRSTHDR(msg); c; c = CMSG_NXTHDR(msg, c)) {
        printf(" [level %d type %d len %zu", c->cmsg_level, c->cmsg_type, (size_t)c->cmsg_len);
        if (c->cmsg_type == SCM_PIDFD && c->cmsg_len >= CMSG_LEN(sizeof(int))) {
            int fd;
            memcpy(&fd, CMSG_DATA(c), sizeof fd);
            printf(" names %s cloexec %d", names(fd, child), fcntl(fd, F_GETFD) & FD_CLOEXEC);
            close(fd);
        } else if (c->cmsg_type == SCM_INQ && c->cmsg_len >= CMSG_LEN(sizeof(int))) {
            int inq;
            memcpy(&inq, CMSG_DATA(c), sizeof inq);
            printf(" inq %d", inq);
        } else if (c->cmsg_type == SCM_CREDENTIALS) {
            struct ucred cred;
            memcpy(&cred, CMSG_DATA(c), sizeof cred);
            printf(" pid is %s", cred.pid == getpid() ? "this" : cred.pid == child ? "child" : "other");
        }
        printf("]");
    }
    printf("\n");
}

static void receive(const char *what, int fd, size_t room, int flags, pid_t child) {
    char buf[16], control[256];
    struct iovec iov = {.iov_base = buf, .iov_len = sizeof buf};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control,
                         .msg_controllen = room};
    long r = recvmsg(fd, &msg, flags | MSG_DONTWAIT);
    if (r < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
        return;
    }
    if (!room)
        msg.msg_control = NULL;
    show_control(what, &msg, r, child);
}

static void option(const char *what, int fd, int name) {
    int value = -1;
    socklen_t length = sizeof value;
    long r = getsockopt(fd, SOL_SOCKET, name, &value, &length);
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %d length %u\n", what, value, length);
}

static void set(const char *what, int fd, int name, int value) {
    result(what, setsockopt(fd, SOL_SOCKET, name, &value, sizeof value));
}

static void peers(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    peer_descriptor("pair peer", sv[0], sizeof(int), 0);
    peer_descriptor("pair peer short", sv[0], 2, 0);
    peer_descriptor("pair peer long", sv[0], 16, 0);
    result("set peer descriptor", setsockopt(sv[0], SOL_SOCKET, SO_PEERPIDFD, &sv[1], 4));
    close(sv[0]);
    close(sv[1]);
    int lone = socket(AF_UNIX, SOCK_STREAM, 0);
    peer_descriptor("unconnected", lone, sizeof(int), 0);
    close(lone);
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    peer_descriptor("datagram pair", sv[0], sizeof(int), 0);
    close(sv[0]);
    close(sv[1]);
    int udp = socket(AF_INET, SOCK_DGRAM, 0);
    peer_descriptor("internet", udp, sizeof(int), 0);
    close(udp);
    // A child connects, and the peer is the child, before and after it ends.
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un address = {.sun_family = AF_UNIX, .sun_path = "\0pidfdl"};
    socklen_t size = sizeof(sa_family_t) + 7;
    bind(listener, (struct sockaddr *)&address, size);
    listen(listener, 4);
    int ready[2];
    pipe(ready);
    pid_t child = fork();
    if (child == 0) {
        int client = socket(AF_UNIX, SOCK_STREAM, 0);
        connect(client, (struct sockaddr *)&address, size);
        write(client, "c", 1);
        char byte;
        read(ready[0], &byte, 1);
        _exit(0);
    }
    int accepted = accept(listener, NULL, NULL);
    char byte;
    read(accepted, &byte, 1);
    peer_descriptor("child peer", accepted, sizeof(int), child);
    write(ready[1], "g", 1);
    waitpid(child, NULL, 0);
    peer_descriptor("child peer reaped", accepted, sizeof(int), child);
    close(accepted);
    close(listener);
    close(ready[0]);
    close(ready[1]);
}

static void passing(void) {
    int sv[2];
    for (int kind = 0; kind < 3; kind++) {
        int type = kind == 0 ? SOCK_STREAM : kind == 1 ? SOCK_DGRAM : SOCK_SEQPACKET;
        printf("kind %d\n", type);
        socketpair(AF_UNIX, type, 0, sv);
        option("pass descriptor default", sv[1], SO_PASSPIDFD);
        set("pass descriptor", sv[1], SO_PASSPIDFD, 1);
        option("pass descriptor set", sv[1], SO_PASSPIDFD);
        send(sv[0], "a", 1, 0);
        receive("with descriptor", sv[1], 256, 0, 0);
        send(sv[0], "b", 1, 0);
        receive("descriptor without room", sv[1], CMSG_SPACE(sizeof(int)) - 1, 0, 0);
        send(sv[0], "c", 1, 0);
        receive("descriptor with no control", sv[1], 0, 0, 0);
        set("pass credentials", sv[1], SO_PASSCRED, 1);
        send(sv[0], "d", 1, 0);
        receive("descriptor and credentials", sv[1], 256, 0, 0);
        send(sv[0], "e", 1, 0);
        receive("peeked", sv[1], 256, MSG_PEEK, 0);
        receive("after peek", sv[1], 256, 0, 0);
        set("pass descriptor off", sv[1], SO_PASSPIDFD, 0);
        send(sv[0], "f", 1, 0);
        receive("credentials only", sv[1], 256, 0, 0);
        close(sv[0]);
        close(sv[1]);
    }
    // From a child that ended before the read.
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    set("pass descriptor", sv[1], SO_PASSPIDFD, 1);
    pid_t child = fork();
    if (child == 0) {
        send(sv[0], "g", 1, 0);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    receive("from a reaped child", sv[1], 256, 0, child);
    close(sv[0]);
    close(sv[1]);
}

static void rights(void) {
    int sv[2], other[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    socketpair(AF_UNIX, SOCK_STREAM, 0, other);
    option("rights default", sv[1], SO_PASSRIGHTS);
    set("rights off", sv[1], SO_PASSRIGHTS, 0);
    option("rights now", sv[1], SO_PASSRIGHTS);
    char control[CMSG_SPACE(sizeof(int))] = {0};
    struct iovec iov = {.iov_base = "r", .iov_len = 1};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control,
                         .msg_controllen = sizeof control};
    struct cmsghdr *c = CMSG_FIRSTHDR(&msg);
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    c->cmsg_len = CMSG_LEN(sizeof(int));
    memcpy(CMSG_DATA(c), &other[0], sizeof(int));
    result("rights refused", sendmsg(sv[0], &msg, MSG_DONTWAIT));
    result("plain still goes", send(sv[0], "p", 1, 0));
    set("rights on", sv[1], SO_PASSRIGHTS, 1);
    result("rights again", sendmsg(sv[0], &msg, MSG_DONTWAIT));
    receive("plain", sv[1], 256, 0, 0);
    receive("rights received", sv[1], 256, 0, 0);
    result("rights and bytes after", sendmsg(sv[0], &msg, MSG_DONTWAIT));
    send(sv[0], "q", 1, 0);
    receive("rights then bytes", sv[1], 256, 0, 0);
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    set("datagram rights off", sv[1], SO_PASSRIGHTS, 0);
    result("datagram rights refused", sendmsg(sv[0], &msg, MSG_DONTWAIT));
    close(sv[0]);
    close(sv[1]);
    int udp = socket(AF_INET, SOCK_DGRAM, 0);
    set("internet rights", udp, SO_PASSRIGHTS, 0);
    set("internet credentials", udp, SO_PASSCRED, 1);
    option("internet credentials now", udp, SO_PASSCRED);
    set("internet descriptor", udp, SO_PASSPIDFD, 1);
    option("internet descriptor now", udp, SO_PASSPIDFD);
    peer_descriptor("internet peer", udp, sizeof(int), 0);
    option("internet rights now", udp, SO_PASSRIGHTS);
    close(udp);
    close(other[0]);
    close(other[1]);
}

static void queued(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    option("inq default", sv[1], SO_INQ);
    set("inq on", sv[1], SO_INQ, 1);
    option("inq now", sv[1], SO_INQ);
    send(sv[0], "0123456789abcdefghij", 20, 0);
    receive("with inq", sv[1], 256, 0, 0);
    receive("with inq rest", sv[1], 256, 0, 0);
    receive("with inq empty", sv[1], 256, 0, 0);
    send(sv[0], "xy", 2, 0);
    receive("inq no room", sv[1], CMSG_SPACE(sizeof(int)) - 1, 0, 0);
    set("inq two", sv[1], SO_INQ, 2);
    option("inq two now", sv[1], SO_INQ);
    set("pass credentials", sv[1], SO_PASSCRED, 1);
    send(sv[0], "uv", 2, 0);
    receive("inq with credentials", sv[1], 256, 0, 0);
    set("pass credentials off", sv[1], SO_PASSCRED, 0);
    set("inq off", sv[1], SO_INQ, 0);
    send(sv[0], "z", 1, 0);
    receive("without inq", sv[1], 256, 0, 0);
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    set("datagram inq", sv[1], SO_INQ, 1);
    option("datagram inq now", sv[1], SO_INQ);
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_SEQPACKET, 0, sv);
    set("packet inq", sv[1], SO_INQ, 1);
    close(sv[0]);
    close(sv[1]);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    peers();
    passing();
    rights();
    queued();
    return 0;
}
