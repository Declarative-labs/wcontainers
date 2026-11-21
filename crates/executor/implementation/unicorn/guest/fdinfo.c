// /proc/<pid>/fdinfo for every kind of descriptor: position, flags, mount and inode, record
// locks, and what each kind adds (event counters, epoll items, inotify watches, signal masks,
// timers, process descriptors, local sockets' descriptors in flight, the pseudo-terminal
// master's index). What differs between machines is checked against the file itself and shown
// as "same": mount ids against statx, inode and device numbers against stat, file handles
// against name_to_handle_at. epoll's items come in the order of kernel addresses, so they are
// sorted.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <mqueue.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/file.h>
#include <sys/inotify.h>
#include <sys/mman.h>
#include <sys/signalfd.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/timerfd.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>

#ifndef AT_HANDLE_FID
#define AT_HANDLE_FID 0x200
#endif

static int watched_count;
// Whether the timer being shown was set relative to now, so its time left is only plausible.
static int relative_timer;
static const char *watched[8];

// The kernel's own encoding of a device number, as fdinfo prints it.
static unsigned kdev(dev_t dev) { return (major(dev) << 20) | minor(dev); }

// statx's mount id (STATX_MNT_ID, at offset 144 of struct statx).
static unsigned long long mount_id(int fd) {
    unsigned long long sx[32];
    if (syscall(SYS_statx, fd, "", AT_EMPTY_PATH, 0x1000, sx) != 0) return ~0ull;
    return sx[144 / 8];
}

// The handle name_to_handle_at gives as an identifier, as fdinfo prints it.
static void handle_text(const char *path, char *out) {
    struct {
        unsigned bytes;
        int type;
        unsigned char data[128];
    } fh = {.bytes = 128};
    int mnt;
    out[0] = 0;
    if (syscall(SYS_name_to_handle_at, AT_FDCWD, path, &fh, &mnt, AT_HANDLE_FID) != 0) {
        sprintf(out, "none %d", errno);
        return;
    }
    sprintf(out, "fhandle-bytes:%x fhandle-type:%x f_handle:", fh.bytes, fh.type);
    for (unsigned i = 0; i < fh.bytes; i++) sprintf(out + strlen(out), "%02x", fh.data[i]);
}

static int compare_lines(const void *a, const void *b) { return strcmp(*(char **)a, *(char **)b); }

// Prints one line of a descriptor's fdinfo with what differs between machines checked.
static void line(int fd, char *l, struct stat *st) {
    unsigned long long value;
    if (sscanf(l, "mnt_id:\t%llu", &value) == 1) {
        printf("  mnt_id: %s\n", value == mount_id(fd) ? "same" : "differs");
    } else if (sscanf(l, "ino:\t%llu", &value) == 1) {
        printf("  ino: %s\n", value == st->st_ino ? "same" : "differs");
    } else if (!strncmp(l, "lock:\t", 6)) {
        int id, pid;
        char kind[16], adv[16], rw[16], rest[64];
        unsigned maj, min;
        unsigned long ino;
        if (sscanf(l + 6, "%d: %15s %15s %15s %d %x:%x:%lu %63[^\n]", &id, kind, adv, rw, &pid, &maj, &min,
                   &ino, rest) == 9)
            printf("  lock %d %s %s %s pid %s dev %s ino %s %s\n", id, kind, adv, rw,
                   pid == getpid() ? "self" : pid == -1 ? "-1" : "other",
                   maj == major(st->st_dev) && min == minor(st->st_dev) ? "same" : "differs",
                   ino == st->st_ino ? "same" : "differs", rest);
        else
            printf("  lock unparsed %s\n", l);
    } else if (!strncmp(l, "eventfd-id:", 11)) {
        printf("  eventfd-id: N\n");
    } else if (!strncmp(l, "inotify ", 8)) {
        int wd;
        unsigned long ino;
        unsigned sdev, mask;
        char handle[200];
        if (sscanf(l, "inotify wd:%x ino:%lx sdev:%x mask:%x ignored_mask:0 %199[^\n]", &wd, &ino, &sdev, &mask,
                   handle) >= 4) {
            const char *path = wd >= 1 && wd <= watched_count ? watched[wd - 1] : "?";
            struct stat ws;
            char expected[200];
            stat(path, &ws);
            handle_text(path, expected);
            printf("  inotify wd %d (%s) ino %s sdev %s mask %x handle %s\n", wd, path,
                   ino == ws.st_ino ? "same" : "differs", sdev == kdev(ws.st_dev) ? "same" : "differs", mask,
                   !strcmp(handle, expected) ? "same" : handle);
        } else
            printf("  inotify unparsed %s\n", l);
    } else if (!strncmp(l, "it_value: (", 11) && relative_timer) {
        unsigned long long s, ns;
        sscanf(l, "it_value: (%llu, %llu)", &s, &ns);
        printf("  it_value: %s\n", s >= 90 && s < 100 && ns < 1000000000 ? "plausible" : l);
    } else {
        printf("  %s\n", l);
    }
}

static void show(const char *what, int fd) {
    char path[64], buf[8192];
    struct stat st;
    fstat(fd, &st);
    snprintf(path, sizeof path, "/proc/self/fdinfo/%d", fd);
    int f = open(path, O_RDONLY);
    if (f < 0) {
        printf("%s: open %s\n", what, strerror(errno));
        return;
    }
    int n = read(f, buf, sizeof buf - 1);
    close(f);
    if (n < 0) {
        printf("%s: read %s\n", what, strerror(errno));
        return;
    }
    buf[n] = 0;
    printf("%s (fd %d):\n", what, fd);
    char *lines[64];
    int count = 0, tfds = 0;
    for (char *l = strtok(buf, "\n"); l && count < 64; l = strtok(NULL, "\n")) lines[count++] = l;
    for (int i = 0; i < count; i++) tfds += !strncmp(lines[i], "tfd:", 4);
    if (tfds) qsort(lines + count - tfds, tfds, sizeof(char *), compare_lines);
    for (int i = 0; i < count; i++) {
        if (!strncmp(lines[i], "tfd:", 4)) {
            int tfd;
            unsigned events, sdev;
            unsigned long long data;
            long long pos;
            unsigned long ino;
            sscanf(lines[i], "tfd: %d events: %x data: %llx pos:%lld ino:%lx sdev:%x", &tfd, &events, &data, &pos,
                   &ino, &sdev);
            struct stat ts;
            fstat(tfd, &ts);
            printf("  tfd %d events %x data %llx pos %lld ino %s sdev %s\n", tfd, events, data, pos,
                   ino == ts.st_ino ? "same" : "differs", sdev == kdev(ts.st_dev) ? "same" : "differs");
            continue;
        }
        line(fd, lines[i], &st);
    }
}

// A relative timer's time left is only checked to be plausible.
static void show_timer(const char *what, int fd, int relative) {
    relative_timer = relative;
    show(what, fd);
    relative_timer = 0;
}

static void listing(void) {
    struct stat st;
    stat("/proc/self/fdinfo", &st);
    printf("fdinfo directory: mode %o links %lu size %lld\n", st.st_mode, (unsigned long)st.st_nlink,
           (long long)st.st_size);
    stat("/proc/self/fdinfo/0", &st);
    printf("fdinfo 0: mode %o links %lu size %lld\n", st.st_mode, (unsigned long)st.st_nlink, (long long)st.st_size);
    int dir = open("/proc/self/fdinfo", O_RDONLY | O_DIRECTORY);
    char buf[4096];
    long n = syscall(SYS_getdents64, dir, buf, sizeof buf);
    printf("listing:");
    for (long at = 0; at < n;) {
        struct {
            unsigned long long ino;
            long long off;
            unsigned short reclen;
            unsigned char type;
            char name[];
        } *d = (void *)(buf + at);
        printf(" %s/%d/%lld", d->name, d->type, d->off);
        at += d->reclen;
    }
    printf("\n");
    close(dir);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    listing();
    errno = 0;
    printf("missing: %d %s\n", open("/proc/self/fdinfo/999", O_RDONLY), strerror(errno));

    int f = open("/dev/shm/x", O_CREAT | O_RDWR | O_APPEND, 0600);
    write(f, "hello", 5);
    show("file appending", f);
    int g = open("/dev/shm/x", O_RDONLY | O_CLOEXEC | O_NONBLOCK | O_NOATIME);
    lseek(g, 3, SEEK_SET);
    show("file close on exec", g);
    struct flock l = {.l_type = F_WRLCK, .l_whence = SEEK_SET, .l_start = 1, .l_len = 2};
    fcntl(f, F_SETLK, &l);
    int h = open("/dev/shm/x", O_RDWR);
    struct flock l2 = {.l_type = F_RDLCK, .l_whence = SEEK_SET, .l_start = 10, .l_len = 0};
    fcntl(h, F_SETLK, &l2);
    show("record locks through f", f);
    show("record locks through h", h);
    // Locks joined: the one already held keeps its file.
    struct flock l3 = {.l_type = F_WRLCK, .l_whence = SEEK_SET, .l_start = 0, .l_len = 1};
    fcntl(h, F_SETLK, &l3);
    show("joined, through f", f);
    show("joined, through h", h);
    flock(g, LOCK_SH);
    show("flock", g);
    int o = open("/dev/shm/y", O_CREAT | O_RDWR, 0600);
    struct flock l4 = {.l_type = F_WRLCK, .l_whence = SEEK_SET, .l_start = 100, .l_len = 5};
    printf("ofd lock: %d\n", fcntl(o, F_OFD_SETLK, &l4));
    show("open file description lock", o);
    int dupd = dup(f);
    show("dup", dupd);
    int high = fcntl(f, F_DUPFD_CLOEXEC, 50);
    show("dup close on exec", high);
    close(high);
    close(dupd);

    int p[2];
    pipe2(p, O_NONBLOCK);
    write(p[1], "abc", 3);
    show("pipe read end", p[0]);
    show("pipe write end", p[1]);
    int e = eventfd(5, EFD_SEMAPHORE | EFD_NONBLOCK);
    show("eventfd", e);
    int e2 = eventfd(0x12345678, 0);
    eventfd_write(e2, 0x100000000ull);
    show("eventfd large", e2);

    int ep = epoll_create1(EPOLL_CLOEXEC);
    struct epoll_event ev = {.events = EPOLLIN | EPOLLET, .data.u64 = 0x1122334455667788ull};
    epoll_ctl(ep, EPOLL_CTL_ADD, e, &ev);
    ev.events = EPOLLOUT | EPOLLONESHOT;
    ev.data.u64 = 7;
    epoll_ctl(ep, EPOLL_CTL_ADD, p[1], &ev);
    ev.events = EPOLLIN | EPOLLEXCLUSIVE;
    ev.data.u64 = 0;
    epoll_ctl(ep, EPOLL_CTL_ADD, p[0], &ev);
    show("epoll", ep);
    struct epoll_event got[4];
    printf("waited: %d\n", epoll_wait(ep, got, 4, 0));
    show("epoll after a wait", ep);

    int in = inotify_init1(IN_NONBLOCK);
    watched[watched_count++] = "/dev/shm";
    watched[watched_count++] = "/dev/shm/x";
    watched[watched_count++] = "/proc";
    watched[watched_count++] = "/dev/null";
    inotify_add_watch(in, "/dev/shm", IN_CREATE | IN_DELETE);
    inotify_add_watch(in, "/dev/shm/x", IN_MODIFY | IN_ONESHOT);
    inotify_add_watch(in, "/proc", IN_ALL_EVENTS);
    inotify_add_watch(in, "/dev/null", IN_ALL_EVENTS | IN_EXCL_UNLINK | IN_MASK_CREATE);
    show("inotify", in);
    write(f, "x", 1);
    show("inotify after a one-shot watch fired", in);

    sigset_t s;
    sigemptyset(&s);
    sigaddset(&s, SIGUSR1);
    sigaddset(&s, SIGKILL);
    sigaddset(&s, SIGRTMIN + 3);
    int sf = signalfd(-1, &s, SFD_CLOEXEC);
    show("signalfd", sf);

    int t = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
    show_timer("timerfd idle", t, 0);
    struct itimerspec its = {.it_value = {100, 5}, .it_interval = {2, 30}};
    timerfd_settime(t, 0, &its, NULL);
    show_timer("timerfd armed", t, 1);
    int t2 = timerfd_create(CLOCK_REALTIME, 0);
    struct itimerspec its2 = {.it_value = {0, 1000}, .it_interval = {0, 1000}};
    timerfd_settime(t2, TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET, &its2, NULL);
    struct pollfd pfd = {t2, POLLIN, 0};
    poll(&pfd, 1, -1);
    show_timer("timerfd fired", t2, 0);

    int self = syscall(SYS_pidfd_open, getpid(), 0);
    show("pidfd of itself", self);
    pid_t child = fork();
    if (child == 0) {
        // A user other than root may not see root's descriptors.
        setgid(1000);
        setuid(1000);
        errno = 0;
        int r = open("/proc/1/fdinfo/0", O_RDONLY);
        printf("as a user, another's fdinfo: %d %s\n", r, strerror(errno));
        errno = 0;
        r = open("/proc/1/fdinfo", O_RDONLY | O_DIRECTORY);
        printf("as a user, another's fdinfo directory: %d %s\n", r, strerror(errno));
        struct stat st;
        errno = 0;
        printf("as a user, stat another's fdinfo: %d %s\n", stat("/proc/1/fdinfo/0", &st), strerror(errno));
        r = open("/proc/self/fdinfo/1", O_RDONLY);
        printf("as a user, its own: %s\n", r >= 0 ? "opened" : strerror(errno));
        _exit(0);
    }
    int pc = syscall(SYS_pidfd_open, child, 0);
    siginfo_t info;
    waitid(P_PID, child, &info, WEXITED | WNOWAIT);
    char buf[256];
    snprintf(buf, sizeof buf, "/proc/self/fdinfo/%d", pc);
    int pf = open(buf, O_RDONLY);
    int n = read(pf, buf, sizeof buf - 1);
    buf[n < 0 ? 0 : n] = 0;
    close(pf);
    printf("pidfd of a zombie: Pid %s\n", atoi(strstr(buf, "Pid:") + 5) == child ? "child" : buf);
    waitpid(child, NULL, 0);
    show("pidfd of a reaped child", pc);

    int sv[2];
    socketpair(AF_UNIX, SOCK_STREAM, 0, sv);
    char cbuf[CMSG_SPACE(3 * sizeof(int))];
    struct msghdr m = {0};
    struct iovec iov = {"x", 1};
    m.msg_iov = &iov;
    m.msg_iovlen = 1;
    m.msg_control = cbuf;
    m.msg_controllen = sizeof cbuf;
    struct cmsghdr *c = CMSG_FIRSTHDR(&m);
    c->cmsg_level = SOL_SOCKET;
    c->cmsg_type = SCM_RIGHTS;
    c->cmsg_len = CMSG_LEN(3 * sizeof(int));
    int three[3] = {f, g, e};
    memcpy(CMSG_DATA(c), three, sizeof three);
    sendmsg(sv[0], &m, 0);
    sendmsg(sv[0], &m, 0);
    show("local stream sender", sv[0]);
    show("local stream receiver", sv[1]);
    char one;
    struct msghdr r = {0};
    struct iovec riov = {&one, 1};
    char rbuf[CMSG_SPACE(3 * sizeof(int))];
    r.msg_iov = &riov;
    r.msg_iovlen = 1;
    r.msg_control = rbuf;
    r.msg_controllen = sizeof rbuf;
    recvmsg(sv[1], &r, MSG_PEEK);
    show("after a peek", sv[1]);
    r.msg_controllen = sizeof rbuf;
    recvmsg(sv[1], &r, 0);
    show("after a read", sv[1]);
    int ds[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, ds);
    m.msg_controllen = sizeof cbuf;
    sendmsg(ds[0], &m, 0);
    show("local datagram receiver", ds[1]);
    int lis = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un a = {.sun_family = AF_UNIX, .sun_path = "\0fdinfo"};
    bind(lis, (void *)&a, sizeof a);
    listen(lis, 4);
    int cl = socket(AF_UNIX, SOCK_STREAM, 0);
    connect(cl, (void *)&a, sizeof a);
    m.msg_controllen = sizeof cbuf;
    sendmsg(cl, &m, 0);
    show("listener with a connection not accepted", lis);
    show("unconnected local stream", socket(AF_UNIX, SOCK_STREAM, 0));
    show("tcp", socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0));
    show("udp", socket(AF_INET6, SOCK_DGRAM, 0));
    show("netlink", socket(AF_NETLINK, SOCK_RAW, 0));

    int pt = open("/dev/ptmx", O_RDWR | O_NOCTTY);
    show("terminal master", pt);
    grantpt(pt);
    unlockpt(pt);
    show("terminal slave", open(ptsname(pt), O_RDWR | O_NOCTTY));
    show("null", open("/dev/null", O_WRONLY));
    int dir = open("/dev/shm", O_RDONLY | O_DIRECTORY);
    show("directory", dir);
    show("path only", open("/dev/shm/x", O_PATH));
    show("memfd", memfd_create("m", MFD_ALLOW_SEALING));
    show("message queue", mq_open("/fdinfo", O_CREAT | O_RDWR, 0600, NULL));
    int pr = open("/proc/self/status", O_RDONLY);
    read(pr, buf, 10);
    show("proc file", pr);
    mkfifo("/dev/shm/fifo", 0600);
    show("named pipe", open("/dev/shm/fifo", O_RDWR));

    // A descriptor closed after its fdinfo was opened reads as gone.
    snprintf(buf, sizeof buf, "/proc/self/fdinfo/%d", g);
    int late = open(buf, O_RDONLY);
    close(g);
    errno = 0;
    printf("closed since: %d %s\n", (int)read(late, buf, sizeof buf), strerror(errno));
    return 0;
}
