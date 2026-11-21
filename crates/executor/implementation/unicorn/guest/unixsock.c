// Local sockets of every kind: pairs, bound names (paths, abstract and automatic), listening and
// accepting, message boundaries, peeking and truncation, shutting down, descriptors and
// credentials passed between processes, options, buffer limits, and readiness for poll and epoll.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static short revents(int fd) {
    struct pollfd p = {.fd = fd, .events = POLLIN | POLLOUT | POLLPRI | POLLRDHUP};
    poll(&p, 1, 0);
    return p.revents;
}

static void pairs(void) {
    int sv[2];
    char buf[64];
    result("pair stream", socketpair(AF_UNIX, SOCK_STREAM, 0, sv));
    printf("poll fresh %#x\n", revents(sv[0]));
    write(sv[0], "hello", 5);
    write(sv[0], "world", 5);
    printf("poll with data %#x\n", revents(sv[1]));
    result("read joined", read(sv[1], buf, sizeof buf));
    write(sv[0], "abcdef", 6);
    result("peek", recv(sv[1], buf, 3, MSG_PEEK));
    result("read part", read(sv[1], buf, 4));
    result("read rest", read(sv[1], buf, 64));
    result("nonblocking empty", recv(sv[1], buf, 64, MSG_DONTWAIT));
    result("shutdown write", shutdown(sv[0], SHUT_WR));
    printf("poll after peer shutdown %#x, own %#x\n", revents(sv[1]), revents(sv[0]));
    result("read at end", read(sv[1], buf, 64));
    signal(SIGPIPE, SIG_IGN);
    result("write after shutdown", write(sv[0], "x", 1));
    result("shutdown bad", shutdown(sv[0], 3));
    close(sv[0]);
    printf("poll after close %#x\n", revents(sv[1]));
    result("write to closed", send(sv[1], "x", 1, MSG_NOSIGNAL));
    close(sv[1]);
    // Unread bytes on close reset the other end.
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    write(sv[0], "lost", 4);
    close(sv[1]);
    result("read after reset", read(sv[0], buf, 64));
    close(sv[0]);
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    write(sv[1], "lost", 4);
    close(sv[1]);
    result("read unread then end", read(sv[0], buf, 64));
    result("read end again", read(sv[0], buf, 64));
    close(sv[0]);

    result("pair dgram", socketpair(AF_UNIX, SOCK_DGRAM, 0, sv));
    send(sv[0], "one", 3, 0);
    send(sv[0], "second", 6, 0);
    send(sv[0], "", 0, 0);
    result("dgram first", recv(sv[1], buf, 64, 0));
    result("dgram truncated", recv(sv[1], buf, 2, 0));
    result("dgram empty", recv(sv[1], buf, 64, 0));
    send(sv[0], "whole", 5, 0);
    result("dgram trunc flag", recv(sv[1], buf, 1, MSG_TRUNC));
    struct msghdr msg = {0};
    struct iovec iov = {buf, 2};
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    send(sv[0], "flags", 5, 0);
    result("recvmsg short", recvmsg(sv[1], &msg, 0));
    printf("msg_flags %#x\n", msg.msg_flags);
    close(sv[0]);
    result("dgram to closed", send(sv[1], "x", 1, 0));
    close(sv[1]);

    result("pair seqpacket", socketpair(AF_UNIX, SOCK_SEQPACKET, 0, sv));
    send(sv[0], "rec1", 4, 0);
    send(sv[0], "record2", 7, 0);
    iov.iov_len = 64;
    msg.msg_flags = 0;
    result("seqpacket first", recvmsg(sv[1], &msg, 0));
    printf("msg_flags %#x\n", msg.msg_flags);
    result("seqpacket truncated", recv(sv[1], buf, 3, 0));
    close(sv[0]);
    result("seqpacket end", recv(sv[1], buf, 64, 0));
    printf("poll seqpacket closed %#x\n", revents(sv[1]));
    close(sv[1]);

    result("pair raw", socketpair(AF_UNIX, SOCK_RAW, 0, sv));
    int type = 0;
    socklen_t length = sizeof type;
    getsockopt(sv[0], SOL_SOCKET, SO_TYPE, &type, &length);
    printf("raw is type %d\n", type);
    close(sv[0]);
    close(sv[1]);
    result("pair bad type", socketpair(AF_UNIX, 9, 0, sv));
    result("pair bad protocol", socketpair(AF_UNIX, SOCK_STREAM, 7, sv));
    result("pair bad flags", socketpair(AF_UNIX, SOCK_STREAM | 0x10000, 0, sv));
    result("pair fault", socketpair(AF_UNIX, SOCK_STREAM, 0, (int *)8));
    result("socket bad family", socket(4242, SOCK_STREAM, 0));
    int p[2];
    pipe(p);
    result("not a socket", send(p[1], "x", 1, 0));
    result("listen not a socket", listen(p[1], 1));
    close(p[0]);
    close(p[1]);
}

static void names(void) {
    struct sockaddr_un a = {.sun_family = AF_UNIX};
    strcpy(a.sun_path, "/tmp/sock");
    int s = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    printf("cloexec %d\n", fcntl(s, F_GETFD));
    umask(022);
    result("bind path", bind(s, (struct sockaddr *)&a, sizeof a));
    struct stat st;
    stat("/tmp/sock", &st);
    printf("node mode %o\n", st.st_mode);
    result("bind again", bind(s, (struct sockaddr *)&a, sizeof a));
    int t = socket(AF_UNIX, SOCK_STREAM, 0);
    result("bind taken", bind(t, (struct sockaddr *)&a, sizeof a));
    result("connect not listening", connect(t, (struct sockaddr *)&a, sizeof a));
    result("listen", listen(s, 2));
    int one = 0;
    socklen_t length = sizeof one;
    getsockopt(s, SOL_SOCKET, SO_ACCEPTCONN, &one, &length);
    printf("acceptconn %d\n", one);
    result("listen unbound", listen(t, 1));
    printf("poll listener idle %#x\n", revents(s));
    result("connect", connect(t, (struct sockaddr *)&a, sizeof a));
    result("connect again", connect(t, (struct sockaddr *)&a, sizeof a));
    printf("poll listener with one %#x\n", revents(s));
    struct sockaddr_un got;
    socklen_t got_length = sizeof got;
    int c = accept(s, (struct sockaddr *)&got, &got_length);
    printf("accepted %d, peer name length %d\n", c >= 0, got_length);
    got_length = sizeof got;
    getsockname(c, (struct sockaddr *)&got, &got_length);
    printf("accepted name %s length %d\n", got.sun_path, got_length);
    got_length = sizeof got;
    getpeername(t, (struct sockaddr *)&got, &got_length);
    printf("client's peer %s length %d\n", got.sun_path, got_length);
    got_length = sizeof got;
    getsockname(t, (struct sockaddr *)&got, &got_length);
    printf("client autobound length %d first byte %d\n", got_length, got.sun_path[0]);
    struct ucred cred;
    length = sizeof cred;
    getsockopt(c, SOL_SOCKET, SO_PEERCRED, &cred, &length);
    printf("peercred pid ok %d uid %d gid %d\n", cred.pid == getpid(), cred.uid, cred.gid);
    write(t, "ping", 4);
    char buf[16] = {0};
    read(c, buf, sizeof buf);
    printf("server read %s\n", buf);
    fcntl(s, F_SETFL, O_NONBLOCK);
    result("accept nonblocking empty", accept4(s, 0, 0, SOCK_NONBLOCK));
    fcntl(s, F_SETFL, 0);
    // A backlog of 2 takes three before connect would wait.
    int extra[4];
    for (int i = 0; i < 4; i++) {
        extra[i] = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0);
        printf("queued %d: ", i);
        result("connect", connect(extra[i], (struct sockaddr *)&a, sizeof a));
    }
    // A blocking client in another process waits until accepted.
    if (fork() == 0) {
        int child = socket(AF_UNIX, SOCK_STREAM, 0);
        connect(child, (struct sockaddr *)&a, sizeof a);
        write(child, "from child", 10);
        _exit(0);
    }
    usleep(20000);
    for (int i = 0; i < 4; i++) {
        int got_fd = accept(s, 0, 0);
        memset(buf, 0, sizeof buf);
        // Only the last, the child's, has bytes on their way.
        int n = recv(got_fd, buf, sizeof buf, i == 3 ? 0 : MSG_DONTWAIT);
        printf("accepted %d read %d %s\n", i, n, n > 0 ? buf : "");
        close(got_fd);
    }
    wait(0);
    for (int i = 0; i < 4; i++) close(extra[i]);
    close(c);
    close(t);
    close(s);
    result("connect after close", connect(socket(AF_UNIX, SOCK_STREAM, 0), (struct sockaddr *)&a, sizeof a));
    unlink("/tmp/sock");

    // Abstract names.
    struct sockaddr_un abstract = {.sun_family = AF_UNIX};
    memcpy(abstract.sun_path, "\0wc-abstract", 12);
    socklen_t abstract_length = 2 + 12;
    int d = socket(AF_UNIX, SOCK_DGRAM, 0);
    result("bind abstract", bind(d, (struct sockaddr *)&abstract, abstract_length));
    int e = socket(AF_UNIX, SOCK_DGRAM, 0);
    result("bind abstract taken", bind(e, (struct sockaddr *)&abstract, abstract_length));
    result("sendto abstract", sendto(e, "dg", 2, 0, (struct sockaddr *)&abstract, abstract_length));
    got_length = sizeof got;
    memset(&got, 0, sizeof got);
    result("recvfrom", recvfrom(d, buf, sizeof buf, 0, (struct sockaddr *)&got, &got_length));
    printf("sender name length %d first byte %d\n", got_length, got.sun_path[0]);
    got_length = sizeof got;
    getsockname(d, (struct sockaddr *)&got, &got_length);
    printf("abstract name length %d\n", got_length);
    result("connect stream to dgram", connect(socket(AF_UNIX, SOCK_STREAM, 0), (struct sockaddr *)&abstract, abstract_length));
    result("send unconnected", send(socket(AF_UNIX, SOCK_DGRAM, 0), "x", 1, 0));
    // Datagrams from a sender the receiver is not connected to queue up to the limit.
    int f = socket(AF_UNIX, SOCK_DGRAM | SOCK_NONBLOCK, 0);
    connect(f, (struct sockaddr *)&abstract, abstract_length);
    int sent = 0;
    while (send(f, "q", 1, 0) == 1) sent++;
    printf("queued datagrams %d, then %s\n", sent, strerror(errno));
    printf("poll sender full %#x\n", revents(f));
    recv(d, buf, sizeof buf, 0);
    printf("poll sender after one taken %#x\n", revents(f));
    close(d);
    result("send to closed name", send(f, "x", 1, 0));
    result("send again", send(f, "x", 1, 0));
    struct sockaddr_un bad = {.sun_family = AF_INET};
    result("bind bad family", bind(e, (struct sockaddr *)&bad, sizeof bad));
    result("bind short", bind(socket(AF_UNIX, SOCK_STREAM, 0), (struct sockaddr *)&a, 1));
    result("bind too long", bind(socket(AF_UNIX, SOCK_STREAM, 0), (struct sockaddr *)&a, 111));
    struct sockaddr_un none = {.sun_family = AF_UNIX};
    strcpy(none.sun_path, "/tmp/missing");
    result("connect missing", connect(socket(AF_UNIX, SOCK_STREAM, 0), (struct sockaddr *)&none, sizeof none));
    int g = socket(AF_UNIX, SOCK_STREAM, 0);
    result("autobind", bind(g, (struct sockaddr *)&none, 2));
    got_length = sizeof got;
    getsockname(g, (struct sockaddr *)&got, &got_length);
    printf("autobound length %d\n", got_length);
}

static void passing(void) {
    int sv[2], p[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    pipe(p);
    if (fork() == 0) {
        close(sv[0]);
        char control[CMSG_SPACE(sizeof(int) * 2)];
        char data = 'x';
        struct iovec iov = {&data, 1};
        struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control, .msg_controllen = sizeof control};
        struct cmsghdr *cmsg = CMSG_FIRSTHDR(&msg);
        cmsg->cmsg_level = SOL_SOCKET;
        cmsg->cmsg_type = SCM_RIGHTS;
        cmsg->cmsg_len = CMSG_LEN(sizeof(int) * 2);
        int fds[2] = {p[1], p[0]};
        memcpy(CMSG_DATA(cmsg), fds, sizeof fds);
        printf("child sendmsg %zd\n", sendmsg(sv[1], &msg, 0));
        write(sv[1], "after", 5);
        // The same with room for only one descriptor at the other end.
        sendmsg(sv[1], &msg, 0);
        _exit(0);
    }
    close(p[1]);
    wait(0);
    char control[CMSG_SPACE(sizeof(int) * 2)];
    char data[16] = {0};
    struct iovec iov = {data, sizeof data};
    struct msghdr msg = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = control, .msg_controllen = sizeof control};
    ssize_t n = recvmsg(sv[0], &msg, MSG_CMSG_CLOEXEC);
    struct cmsghdr *cmsg = CMSG_FIRSTHDR(&msg);
    int fds[2];
    memcpy(fds, CMSG_DATA(cmsg), sizeof fds);
    printf("received %zd bytes, controllen %zu, level %d type %d len %zu, flags %#x\n", n, msg.msg_controllen,
           cmsg->cmsg_level, cmsg->cmsg_type, cmsg->cmsg_len, msg.msg_flags);
    printf("passed cloexec %d\n", fcntl(fds[0], F_GETFD));
    write(fds[0], "through", 7);
    close(fds[0]);
    char got[16] = {0};
    read(fds[1], got, sizeof got);
    printf("through passed pipe: %s\n", got);
    close(fds[1]);
    memset(data, 0, sizeof data);
    printf("then read %zd %s\n", read(sv[0], data, 5), data);
    char small[CMSG_SPACE(sizeof(int))];
    msg.msg_control = small;
    msg.msg_controllen = sizeof small;
    msg.msg_flags = 0;
    n = recvmsg(sv[0], &msg, 0);
    printf("small control: %zd, controllen %zu, flags %#x\n", n, msg.msg_controllen, msg.msg_flags);
    close(sv[0]);
    close(sv[1]);

    // Credentials, as sent and as named.
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sv);
    int on = 1;
    setsockopt(sv[0], SOL_SOCKET, SO_PASSCRED, &on, sizeof on);
    send(sv[1], "c", 1, 0);
    char ccontrol[CMSG_SPACE(sizeof(struct ucred))];
    msg.msg_control = ccontrol;
    msg.msg_controllen = sizeof ccontrol;
    msg.msg_flags = 0;
    iov.iov_len = sizeof data;
    n = recvmsg(sv[0], &msg, 0);
    cmsg = CMSG_FIRSTHDR(&msg);
    struct ucred cred;
    memcpy(&cred, CMSG_DATA(cmsg), sizeof cred);
    printf("creds: %zd type %d pid ok %d uid %d gid %d\n", n, cmsg->cmsg_type, cred.pid == getpid(), cred.uid, cred.gid);
    struct msghdr out = {.msg_iov = &iov, .msg_iovlen = 1, .msg_control = ccontrol, .msg_controllen = sizeof ccontrol};
    cmsg = CMSG_FIRSTHDR(&out);
    cmsg->cmsg_level = SOL_SOCKET;
    cmsg->cmsg_type = SCM_CREDENTIALS;
    cmsg->cmsg_len = CMSG_LEN(sizeof cred);
    cred.uid = 1234;
    cred.gid = 5678;
    memcpy(CMSG_DATA(cmsg), &cred, sizeof cred);
    iov.iov_len = 1;
    result("send named creds", sendmsg(sv[1], &out, 0));
    msg.msg_controllen = sizeof ccontrol;
    iov.iov_len = sizeof data;
    recvmsg(sv[0], &msg, 0);
    memcpy(&cred, CMSG_DATA(CMSG_FIRSTHDR(&msg)), sizeof cred);
    printf("named creds uid %d gid %d\n", cred.uid, cred.gid);
    cred.pid = 1;
    memcpy(CMSG_DATA(cmsg), &cred, sizeof cred);
    iov.iov_len = 1;
    result("send another's pid", sendmsg(sv[1], &out, 0));
    close(sv[0]);
    close(sv[1]);
}

static void options_and_limits(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, sv);
    int value = 0;
    socklen_t length = sizeof value;
    getsockopt(sv[0], SOL_SOCKET, SO_SNDBUF, &value, &length);
    printf("sndbuf %d\n", value);
    getsockopt(sv[0], SOL_SOCKET, SO_RCVBUF, &value, &length);
    printf("rcvbuf %d\n", value);
    getsockopt(sv[0], SOL_SOCKET, SO_DOMAIN, &value, &length);
    printf("domain %d\n", value);
    getsockopt(sv[0], SOL_SOCKET, SO_PROTOCOL, &value, &length);
    printf("protocol %d\n", value);
    getsockopt(sv[0], SOL_SOCKET, SO_ERROR, &value, &length);
    printf("error %d\n", value);
    char chunk[65536] = {0};
    long total = 0;
    ssize_t n;
    while ((n = write(sv[0], chunk, 4096)) > 0) total += n;
    printf("stream holds %ld in 4096-byte writes, then %s\n", total, strerror(errno));
    printf("poll full %#x\n", revents(sv[0]));
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, sv);
    total = 0;
    while ((n = write(sv[0], chunk, sizeof chunk)) > 0) total += n;
    printf("stream holds %ld in 65536-byte writes\n", total);
    close(sv[0]);
    close(sv[1]);
    socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, sv);
    value = 10000;
    setsockopt(sv[0], SOL_SOCKET, SO_SNDBUF, &value, sizeof value);
    getsockopt(sv[0], SOL_SOCKET, SO_SNDBUF, &value, &length);
    printf("sndbuf set %d\n", value);
    value = 1;
    setsockopt(sv[0], SOL_SOCKET, SO_SNDBUF, &value, sizeof value);
    getsockopt(sv[0], SOL_SOCKET, SO_SNDBUF, &value, &length);
    printf("sndbuf smallest %d\n", value);
    total = 0;
    while ((n = write(sv[0], chunk, 100)) > 0) total += n;
    printf("small buffer holds %ld\n", total);
    value = 1;
    result("reuseaddr", setsockopt(sv[0], SOL_SOCKET, SO_REUSEADDR, &value, sizeof value));
    value = 0;
    getsockopt(sv[0], SOL_SOCKET, SO_REUSEADDR, &value, &length);
    printf("reuseaddr %d\n", value);
    result("short option", setsockopt(sv[0], SOL_SOCKET, SO_REUSEADDR, &value, 2));
    result("unknown option", setsockopt(sv[0], SOL_SOCKET, 9999, &value, sizeof value));
    result("set type", setsockopt(sv[0], SOL_SOCKET, SO_TYPE, &value, sizeof value));
    length = 2;
    getsockopt(sv[0], SOL_SOCKET, SO_TYPE, &value, &length);
    printf("short get length %d\n", length);
    struct timeval timeout = {0, 20000};
    setsockopt(sv[1], SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof timeout);
    int blocking = socket(AF_UNIX, SOCK_STREAM, 0);
    int pair2[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, pair2);
    setsockopt(pair2[0], SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof timeout);
    char buf[4];
    result("read timed out", read(pair2[0], buf, 4));
    struct timeval back;
    length = sizeof back;
    getsockopt(pair2[0], SOL_SOCKET, SO_RCVTIMEO, &back, &length);
    printf("rcvtimeo %ld.%06ld\n", (long)back.tv_sec, (long)back.tv_usec);
    struct stat st;
    fstat(pair2[0], &st);
    char link[64] = {0}, path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", pair2[0]);
    readlink(path, link, sizeof link - 1);
    printf("mode %o size %lld link %.8s flags %#x\n", st.st_mode, (long long)st.st_size, link, fcntl(pair2[0], F_GETFL));
    result("lseek", lseek(pair2[0], 0, SEEK_SET));
    result("pread", pread(pair2[0], buf, 1, 0));
    close(blocking);
}

static void readiness(void) {
    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, sv);
    int ep = epoll_create1(0);
    struct epoll_event event = {.events = EPOLLIN | EPOLLET | EPOLLRDHUP, .data.u64 = 1};
    epoll_ctl(ep, EPOLL_CTL_ADD, sv[1], &event);
    struct epoll_event got[4];
    printf("epoll idle %d\n", epoll_wait(ep, got, 4, 0));
    write(sv[0], "a", 1);
    int n = epoll_wait(ep, got, 4, 0);
    printf("epoll data %d %#x\n", n, n > 0 ? got[0].events : 0);
    printf("epoll edge again %d\n", epoll_wait(ep, got, 4, 0));
    char buf[4];
    read(sv[1], buf, 1);
    write(sv[0], "b", 1);
    n = epoll_wait(ep, got, 4, 0);
    printf("epoll more %d %#x\n", n, n > 0 ? got[0].events : 0);
    shutdown(sv[0], SHUT_WR);
    n = epoll_wait(ep, got, 4, 0);
    printf("epoll shut %d %#x\n", n, n > 0 ? got[0].events : 0);
    // A blocked reader in another process wakes when bytes come.
    int pair[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, pair);
    if (fork() == 0) {
        usleep(20000);
        write(pair[1], "late", 4);
        _exit(0);
    }
    char late[8] = {0};
    printf("blocking read %zd %s\n", read(pair[0], late, sizeof late), late);
    wait(0);
}

// A connect reads the socket's node, which moves its access time.
static void access_time(void) {
    int s = socket(AF_UNIX, SOCK_DGRAM, 0), c = socket(AF_UNIX, SOCK_DGRAM, 0);
    struct sockaddr_un a = {.sun_family = AF_UNIX, .sun_path = "/tmp/atime.sock"};
    bind(s, (void *)&a, sizeof a);
    struct timespec times[2] = {{.tv_sec = 1000}, {.tv_nsec = UTIME_OMIT}};
    utimensat(AT_FDCWD, a.sun_path, times, 0);
    struct stat st;
    stat(a.sun_path, &st);
    printf("before connect atime %ld\n", (long)st.st_atime);
    result("connect", connect(c, (void *)&a, sizeof a));
    stat(a.sun_path, &st);
    printf("after connect atime moved %d\n", st.st_atime != 1000);
    int t = socket(AF_UNIX, SOCK_STREAM, 0);
    utimensat(AT_FDCWD, a.sun_path, times, 0);
    result("connect of another kind", connect(t, (void *)&a, sizeof a));
    stat(a.sun_path, &st);
    printf("after a refused kind atime moved %d\n", st.st_atime != 1000);
    unlink(a.sun_path);
    close(s);
    close(c);
    close(t);
}

// A pending error comes before a queued message: the other end went with bytes unread.
static void error_first(void) {
    int kinds[] = {SOCK_SEQPACKET, SOCK_STREAM};
    for (int k = 0; k < 2; k++) {
        int sv[2];
        socketpair(AF_UNIX, kinds[k] | SOCK_NONBLOCK, 0, sv);
        send(sv[0], "", 0, 0);
        send(sv[0], "q", 1, 0);
        send(sv[1], "abcde", 5, 0);
        close(sv[0]);
        char buf[16];
        long r = recv(sv[1], buf, sizeof buf, MSG_PEEK);
        printf("kind %d first: %ld %s\n", kinds[k], r, r < 0 ? strerror(errno) : "");
        r = recv(sv[1], buf, sizeof buf, 0);
        printf("kind %d then: %ld %s\n", kinds[k], r, r < 0 ? strerror(errno) : "");
        close(sv[1]);
    }
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    error_first();
    access_time();
    pairs();
    names();
    passing();
    options_and_limits();
    readiness();
    return 0;
}
