// POSIX message queues: mq_open's names, flags, modes and limits, the queue files in /dev/mqueue
// and what the file calls do with them, sends and receives by priority, sizes, nonblocking and
// timed calls, waits handed messages in order, signals during waits, notification by signal, the
// attributes, unlinking, and the per-user limit. Times are printed as set or not, and process
// numbers as the caller's or another's.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

struct attr {
    long flags, maxmsg, msgsize, curmsgs, reserved[4];
};
struct event {
    unsigned long value;
    int signo, notify;
    int pad[12];
};

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: errno %d\n", what, errno);
    else
        printf("%s: %ld\n", what, result);
    errno = 0;
}

static long mq_open_(const char *name, long flags, long mode, struct attr *a) {
    return syscall(SYS_mq_open, name, flags, mode, a);
}
static long send_(int fd, const char *text, size_t size, unsigned prio, struct timespec *t) {
    return syscall(SYS_mq_timedsend, fd, text, size, prio, t);
}
static long receive_(int fd, char *buffer, size_t size, unsigned *prio, struct timespec *t) {
    return syscall(SYS_mq_timedreceive, fd, buffer, size, prio, t);
}
static long getset(int fd, struct attr *new, struct attr *old) { return syscall(SYS_mq_getsetattr, fd, new, old); }
static long notify(int fd, struct event *e) { return syscall(SYS_mq_notify, fd, e); }

static void attributes(const char *what, int fd) {
    struct attr a;
    memset(&a, 0xff, sizeof a);
    long r = getset(fd, NULL, &a);
    if (r < 0) {
        show(what, r);
        return;
    }
    printf("%s: flags %lo maxmsg %ld msgsize %ld curmsgs %ld reserved %ld %ld %ld %ld\n", what, a.flags, a.maxmsg,
           a.msgsize, a.curmsgs, a.reserved[0], a.reserved[1], a.reserved[2], a.reserved[3]);
}

// The queue file's text, with the process number shown as the caller's or not.
static void text(const char *what, int fd) {
    char buffer[200];
    memset(buffer, 0, sizeof buffer);
    long n = pread(fd, buffer, sizeof buffer - 1, 0);
    if (n < 0) {
        show(what, n);
        return;
    }
    char *pid = strstr(buffer, "NOTIFY_PID:");
    if (pid) {
        int number = atoi(pid + 11);
        sprintf(pid + 11, "%s\n", number == 0 ? "0" : number == getpid() ? "me" : "other");
    }
    printf("%s: %ld %s", what, n, buffer);
}

static void received(const char *what, int fd, size_t size) {
    char buffer[9000];
    unsigned prio = 77;
    memset(buffer, 0, sizeof buffer);
    long r = receive_(fd, buffer, size, &prio, NULL);
    if (r < 0)
        show(what, r);
    else
        printf("%s: %ld prio %u text %.*s\n", what, r, prio, (int)r, buffer);
}

static void file_stat(const char *what, const char *path) {
    struct stat s;
    if (stat(path, &s) < 0) {
        show(what, -1);
        return;
    }
    printf("%s: mode %o size %lld links %lu uid %u gid %u blocks %lld blksize %ld\n", what, s.st_mode,
           (long long)s.st_size, (unsigned long)s.st_nlink, s.st_uid, s.st_gid, (long long)s.st_blocks,
           (long)s.st_blksize);
}

static void listing(const char *what) {
    DIR *d = opendir("/dev/mqueue");
    printf("%s:", what);
    struct dirent *e;
    while ((e = readdir(d)))
        printf(" %s(%d)", e->d_name, e->d_type);
    printf("\n");
    closedir(d);
}

static void opens(void) {
    umask(022);
    int q = mq_open_("q1", O_CREAT | O_RDWR, 0666, NULL);
    show("create", q >= 0);
    show("close on exec", fcntl(q, F_GETFD));
    printf("status flags: %o\n", fcntl(q, F_GETFL));
    attributes("defaults", q);
    file_stat("queue file", "/dev/mqueue/q1");
    file_stat("directory", "/dev/mqueue");
    struct statfs f;
    statfs("/dev/mqueue", &f);
    printf("statfs: type %lx bsize %ld blocks %ld files %ld namelen %ld flags %lx\n", (long)f.f_type, (long)f.f_bsize,
           (long)f.f_blocks, (long)f.f_files, (long)f.f_namelen, (long)f.f_flags);
    show("open again", mq_open_("q1", O_RDONLY, 0, NULL) >= 0);
    show("exclusive", mq_open_("q1", O_CREAT | O_EXCL | O_RDWR, 0600, NULL));
    show("access mode 3", mq_open_("q1", 3, 0, NULL));
    show("direct", mq_open_("q1", O_RDWR | O_DIRECT, 0, NULL));
    show("missing", mq_open_("nothere", O_RDONLY, 0, NULL));
    show("slash", mq_open_("/q1", O_RDONLY, 0, NULL));
    show("dot", mq_open_(".", O_RDONLY, 0, NULL));
    show("dot dot", mq_open_("..", O_CREAT | O_RDWR, 0600, NULL));
    show("empty", mq_open_("", O_RDONLY, 0, NULL));
    char long_name[300];
    memset(long_name, 'n', sizeof long_name);
    long_name[299] = 0;
    show("long name", mq_open_(long_name, O_CREAT | O_RDWR, 0600, NULL));
    show("bad name", mq_open_((char *)8, O_RDONLY, 0, NULL));
    show("bad attributes", mq_open_("q2", O_CREAT | O_RDWR, 0600, (struct attr *)8));
    struct attr a = {0, 5, 100, 0, {0}};
    int q2 = mq_open_("q2", O_CREAT | O_RDWR | O_NONBLOCK, 0640, &a);
    show("create with attributes", q2 >= 0);
    attributes("given", q2);
    printf("status flags: %o\n", fcntl(q2, F_GETFL));
    struct attr bad[] = {{0, 0, 100}, {0, 11, 100}, {0, 5, 0}, {0, 5, 8193}, {0, -1, 100}, {0, 10, 8192}};
    for (int i = 0; i < 6; i++) {
        char name[20];
        sprintf(name, "bad%d", i);
        long r = mq_open_(name, O_CREAT | O_RDWR, 0600, &bad[i]);
        printf("attributes %ld %ld: %ld errno %d\n", bad[i].maxmsg, bad[i].msgsize, r < 0 ? r : 0, r < 0 ? errno : 0);
        errno = 0;
    }
    show("existing ignores attributes", mq_open_("q2", O_CREAT | O_RDWR, 0600, &bad[0]) >= 0);
    file_stat("mode with umask", "/dev/mqueue/q2");
    listing("listing");
    file_stat("directory after", "/dev/mqueue");
    mq_open_("bad5", 0, 0, NULL);
    syscall(SYS_mq_unlink, "bad5");

    // What the file calls do with a queue.
    char buffer[100];
    show("write", write(q, "x", 1));
    show("read", read(q, buffer, 10));
    show("read rest", read(q, buffer, 100));
    show("read at end", read(q, buffer, 100));
    show("seek end", lseek(q, 0, SEEK_END));
    show("seek data", lseek(q, 5, SEEK_DATA));
    show("seek hole", lseek(q, 5, SEEK_HOLE));
    show("seek data past", lseek(q, 80, SEEK_DATA));
    show("map", (long)mmap(NULL, 4096, PROT_READ, MAP_SHARED, q, 0) == -1 ? -1 : 0);
    show("fsync", fsync(q));
    show("fallocate", fallocate(q, 0, 0, 10));
    show("truncate", ftruncate(q, 5));
    file_stat("after truncate", "/dev/mqueue/q1");
    show("seek end after truncate", lseek(q, 0, SEEK_END));
    show("ioctl", ioctl(q, 0x541B, buffer));
    int path = open("/dev/mqueue/q1", O_PATH);
    show("send through path", send_(path, "x", 1, 0, NULL));
    show("getsetattr through path", getset(path, NULL, (struct attr *)buffer));
    close(path);
    show("send through a pipe", ({
        int p[2];
        pipe(p);
        send_(p[1], "x", 1, 0, NULL);
    }));

    // The directory's own names.
    show("open create in directory", open("/dev/mqueue/viaopen", O_CREAT | O_RDWR, 0644) >= 0);
    int viaopen = open("/dev/mqueue/viaopen", O_RDWR);
    attributes("made by open", viaopen);
    show("open with truncate", open("/dev/mqueue/viaopen", O_RDWR | O_TRUNC) >= 0);
    file_stat("after open with truncate", "/dev/mqueue/viaopen");
    show("mknod regular", mknod("/dev/mqueue/vianod", S_IFREG | 0600, 0));
    show("mknod fifo", mknod("/dev/mqueue/fifo", S_IFIFO | 0600, 0));
    show("mkdir", mkdir("/dev/mqueue/dir", 0700));
    show("symlink", symlink("q1", "/dev/mqueue/link"));
    show("link", link("/dev/mqueue/q1", "/dev/mqueue/hard"));
    show("rename", rename("/dev/mqueue/q1", "/dev/mqueue/q9"));
    show("rmdir a queue", rmdir("/dev/mqueue/q1"));
    show("unlink", unlink("/dev/mqueue/vianod"));
    show("mq_unlink", syscall(SYS_mq_unlink, "viaopen"));
    show("mq_unlink again", syscall(SYS_mq_unlink, "viaopen"));
    show("mq_unlink slash", syscall(SYS_mq_unlink, "a/b"));
    show("mq_unlink empty", syscall(SYS_mq_unlink, ""));
    attributes("unlinked still open", viaopen);
    show("chmod", chmod("/dev/mqueue/q2", 0604));
    file_stat("after chmod", "/dev/mqueue/q2");
    listing("listing after");
    close(viaopen);
    close(q2);
    close(q);
    syscall(SYS_mq_unlink, "q2");
}

static void messages(void) {
    struct attr a = {0, 4, 16, 0, {0}};
    int q = mq_open_("m", O_CREAT | O_RDWR | O_NONBLOCK, 0600, &a);
    show("send prio 1", send_(q, "one", 3, 1, NULL));
    show("send prio 5", send_(q, "five", 4, 5, NULL));
    show("send prio 1 again", send_(q, "uno", 3, 1, NULL));
    show("send empty prio 5", send_(q, "", 0, 5, NULL));
    attributes("four", q);
    text("text", q);
    show("send full", send_(q, "x", 1, 0, NULL));
    show("send too large", send_(q, "0123456789abcdefg", 17, 0, NULL));
    show("send prio limit", send_(q, "x", 1, 32768, NULL));
    show("send bad buffer", send_(q, (char *)8, 1, 0, NULL));
    struct timespec bad = {0, 1000000000};
    show("send bad timeout", send_(q, "x", 1, 0, &bad));
    show("send bad timeout pointer", send_(q, "x", 1, 0, (struct timespec *)8));
    show("send bad fd", send_(99, "x", 1, 0, NULL));
    received("receive small buffer", q, 15);
    received("receive", q, 16);
    received("receive", q, 100);
    received("receive", q, 100);
    received("receive", q, 100);
    received("receive empty", q, 100);
    send_(q, "lost", 4, 3, NULL);
    show("receive bad buffer", receive_(q, (char *)8, 16, NULL, NULL));
    attributes("after bad buffer", q);
    send_(q, "kept", 4, 3, NULL);
    show("receive bad prio pointer", receive_(q, (char[16]){0}, 16, (unsigned *)8, NULL));
    attributes("after bad prio pointer", q);
    int ro = mq_open_("m", O_RDONLY, 0, NULL);
    int wo = mq_open_("m", O_WRONLY, 0, NULL);
    show("send read only", send_(ro, "x", 1, 0, NULL));
    show("receive write only", receive_(wo, (char[16]){0}, 16, NULL, NULL));

    // Attributes: only O_NONBLOCK may change.
    struct attr set = {O_NONBLOCK | 1, 0, 0, 0, {0}};
    show("set other flags", getset(ro, &set, NULL));
    set.flags = O_NONBLOCK;
    set.maxmsg = 99;
    show("set nonblocking", getset(ro, &set, NULL));
    attributes("nonblocking read only", ro);
    printf("status flags: %o\n", fcntl(ro, F_GETFL));
    show("getsetattr bad old", getset(ro, NULL, (struct attr *)8));
    show("getsetattr bad new", getset(ro, (struct attr *)8, NULL));
    show("getsetattr not a queue", getset(0, NULL, NULL));
    set.flags = 0;
    getset(ro, &set, NULL);
    printf("status flags: %o\n", fcntl(ro, F_GETFL));

    // A blocking call on a full or empty queue with a time in the past ends at once.
    set.flags = 0;
    getset(q, &set, NULL);
    struct timespec past = {1, 0};
    for (int i = 0; i < 4; i++)
        send_(q, "f", 1, 0, NULL);
    show("send full timed out", send_(q, "x", 1, 0, &past));
    struct timespec soon;
    clock_gettime(CLOCK_REALTIME, &soon);
    soon.tv_nsec += 50000000;
    if (soon.tv_nsec >= 1000000000) {
        soon.tv_sec++;
        soon.tv_nsec -= 1000000000;
    }
    show("send full timed out soon", send_(q, "x", 1, 0, &soon));
    for (int i = 0; i < 4; i++)
        receive_(q, (char[16]){0}, 16, NULL, NULL);
    show("receive empty timed out", receive_(q, (char[16]){0}, 16, NULL, &past));
    close(ro);
    close(wo);
    close(q);
    syscall(SYS_mq_unlink, "m");
}

static void waits(void) {
    struct attr a = {0, 2, 16, 0, {0}};
    int q = mq_open_("w", O_CREAT | O_RDWR, 0600, &a);
    int ends[2];
    pipe(ends);

    // Receivers wait in order and each is handed one message.
    pid_t kids[3];
    for (int i = 0; i < 3; i++) {
        kids[i] = fork();
        if (kids[i] == 0) {
            char buffer[16] = {0};
            unsigned prio = 0;
            long r = receive_(q, buffer, 16, &prio, NULL);
            char line[100];
            int n = snprintf(line, sizeof line, "receiver %d: %ld prio %u %s\n", i, r, prio, buffer);
            write(ends[1], line, n);
            _exit(0);
        }
        usleep(100000);
    }
    attributes("three receivers waiting", q);
    text("text with receivers", q);
    show("send a", send_(q, "a", 1, 1, NULL));
    usleep(100000);
    show("send b", send_(q, "b", 1, 9, NULL));
    usleep(100000);
    show("send c", send_(q, "c", 1, 4, NULL));
    for (int i = 0; i < 3; i++)
        waitpid(kids[i], NULL, 0);
    char buffer[1000];
    long n = read(ends[0], buffer, sizeof buffer);
    if (n > 0)
        fwrite(buffer, 1, n, stdout);
    attributes("after handing", q);

    // Senders wait for room and their messages go in as room is made.
    send_(q, "x", 1, 0, NULL);
    send_(q, "y", 1, 0, NULL);
    for (int i = 0; i < 2; i++) {
        kids[i] = fork();
        if (kids[i] == 0) {
            char text[2] = {'P' + i, 0};
            long r = send_(q, text, 1, 3 - i, NULL);
            char line[100];
            int n = snprintf(line, sizeof line, "sender %d: %ld\n", i, r);
            write(ends[1], line, n);
            _exit(0);
        }
        usleep(100000);
    }
    attributes("two senders waiting", q);
    received("make room", q, 16);
    usleep(100000);
    attributes("one sender left", q);
    received("make room again", q, 16);
    for (int i = 0; i < 2; i++)
        waitpid(kids[i], NULL, 0);
    n = read(ends[0], buffer, sizeof buffer);
    if (n > 0)
        fwrite(buffer, 1, n, stdout);
    received("left", q, 16);
    received("left", q, 16);

    // A signal with a restarting handler restarts the wait; without, it ends with EINTR.
    for (int restart = 0; restart < 2; restart++) {
        pid_t kid = fork();
        if (kid == 0) {
            struct sigaction handled = {0};
            handled.sa_handler = (void (*)(int))abs;
            handled.sa_flags = restart ? SA_RESTART : 0;
            sigaction(SIGUSR1, &handled, NULL);
            char got[16] = {0};
            long r = receive_(q, got, 16, NULL, NULL);
            printf("restart %d: %ld errno %d %s\n", restart, r, r < 0 ? errno : 0, got);
            _exit(0);
        }
        usleep(100000);
        kill(kid, SIGUSR1);
        usleep(100000);
        attributes("after the signal", q);
        send_(q, "z", 1, 0, NULL);
        waitpid(kid, NULL, 0);
        struct attr empty = {O_NONBLOCK};
        getset(q, &empty, NULL);
        receive_(q, (char[16]){0}, 16, NULL, NULL);
        empty.flags = 0;
        getset(q, &empty, NULL);
    }

    // A waiter killed leaves the line.
    pid_t kid = fork();
    if (kid == 0) {
        receive_(q, (char[16]){0}, 16, NULL, NULL);
        _exit(0);
    }
    usleep(100000);
    kill(kid, SIGKILL);
    waitpid(kid, NULL, 0);
    send_(q, "k", 1, 0, NULL);
    attributes("after the killed waiter", q);
    close(q);
    syscall(SYS_mq_unlink, "w");
}

static volatile int got_signal, got_code, got_pid;
static volatile long got_value;
static void on_signal(int signal, siginfo_t *info, void *context) {
    (void)context;
    got_signal = signal;
    got_code = info->si_code;
    got_pid = info->si_pid;
    got_value = (long)info->si_value.sival_ptr;
}

static void notifications(void) {
    struct sigaction action = {0};
    action.sa_sigaction = on_signal;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGUSR2, &action, NULL);
    struct attr a = {0, 4, 16, 0, {0}};
    int q = mq_open_("n", O_CREAT | O_RDWR | O_NONBLOCK, 0600, &a);
    struct event e = {0x1234, SIGUSR2, 0};
    show("notify", notify(q, &e));
    text("registered", q);
    show("notify again", notify(q, &e));
    pid_t kid = fork();
    if (kid == 0) {
        show("child notify", notify(q, &e));
        show("child unregister", notify(q, NULL));
        _exit(0);
    }
    waitpid(kid, NULL, 0);
    text("after child", q);
    kid = fork();
    if (kid == 0) {
        send_(q, "hi", 2, 0, NULL);
        _exit(0);
    }
    waitpid(kid, NULL, 0);
    printf("signal %d code %d from child %d value %lx\n", got_signal, got_code, got_pid == kid, got_value);
    text("after the signal", q);
    got_signal = 0;
    send_(q, "again", 5, 0, NULL);
    printf("no second signal: %d\n", got_signal);
    // Only a message to an empty queue tells.
    show("notify while not empty", notify(q, &e));
    send_(q, "more", 4, 0, NULL);
    printf("not empty: %d\n", got_signal);
    for (int i = 0; i < 3; i++)
        receive_(q, (char[16]){0}, 16, NULL, NULL);
    send_(q, "now", 3, 0, NULL);
    printf("empty again: %d\n", got_signal);
    receive_(q, (char[16]){0}, 16, NULL, NULL);

    // A waiting receiver takes the message before anyone is told.
    got_signal = 0;
    show("notify with a receiver", notify(q, &e));
    struct attr blocking = {0};
    getset(q, &blocking, NULL);
    kid = fork();
    if (kid == 0) {
        long r = receive_(q, (char[16]){0}, 16, NULL, NULL);
        printf("receiver: %ld\n", r);
        _exit(0);
    }
    usleep(100000);
    send_(q, "taken", 5, 0, NULL);
    waitpid(kid, NULL, 0);
    printf("with a receiver: %d\n", got_signal);
    text("still registered", q);

    // Closing any descriptor of the queue takes the request away.
    int other = mq_open_("n", O_RDONLY, 0, NULL);
    close(other);
    text("after a close", q);

    show("unregister when not registered", notify(q, NULL));
    struct event none = {0, 0, SIGEV_NONE};
    show("notify none", notify(q, &none));
    text("none", q);
    show("unregister", notify(q, NULL));
    struct event zero = {0, 0, SIGEV_SIGNAL};
    show("notify signal 0", notify(q, &zero));
    text("signal 0", q);
    notify(q, NULL);
    struct event bad_kind = {0, SIGUSR2, 7};
    show("notify bad kind", notify(q, &bad_kind));
    struct event bad_signal = {0, 65, SIGEV_SIGNAL};
    show("notify bad signal", notify(q, &bad_signal));
    struct event thread_bad_cookie = {8, 0, SIGEV_THREAD};
    show("notify thread bad cookie", notify(q, &thread_bad_cookie));
    char cookie[32] = {0};
    struct event thread_bad_fd = {(unsigned long)cookie, 99, SIGEV_THREAD};
    show("notify thread bad descriptor", notify(q, &thread_bad_fd));
    struct event thread_not_socket = {(unsigned long)cookie, q, SIGEV_THREAD};
    show("notify thread not a socket", notify(q, &thread_not_socket));
    show("notify bad pointer", notify(q, (struct event *)8));
    show("notify not a queue", notify(0, &e));
    show("unregister not a queue", notify(0, NULL));
    close(q);
    syscall(SYS_mq_unlink, "n");
}

static void limits(void) {
    // Each default queue charges its user about 80 KiB against RLIMIT_MSGQUEUE (800 KiB).
    int made = 0;
    for (; made < 20; made++) {
        char name[20];
        sprintf(name, "l%d", made);
        long r = mq_open_(name, O_CREAT | O_RDWR, 0600, NULL);
        if (r < 0) {
            printf("default queues made: %d errno %d\n", made, errno);
            break;
        }
        close(r);
    }
    struct attr small = {0, 1, 1, 0, {0}};
    show("a small one still fits", mq_open_("small", O_CREAT | O_RDWR, 0600, &small) >= 0);
    for (int i = 0; i < made; i++) {
        char name[20];
        sprintf(name, "l%d", i);
        syscall(SYS_mq_unlink, name);
    }
    syscall(SYS_mq_unlink, "small");
    show("room again", mq_open_("again", O_CREAT | O_RDWR, 0600, NULL) >= 0);
    syscall(SYS_mq_unlink, "again");

    int q = mq_open_("perm", O_CREAT | O_RDWR, 0600, NULL);
    pid_t kid = fork();
    if (kid == 0) {
        setresgid(1000, 1000, 1000);
        setresuid(1000, 1000, 1000);
        show("other user open", mq_open_("perm", O_RDONLY, 0, NULL));
        show("other user unlink", syscall(SYS_mq_unlink, "perm"));
        show("other user create", mq_open_("theirs", O_CREAT | O_RDWR, 0600, NULL) >= 0);
        show("other user send through inherited", send_(q, "x", 1, 0, NULL));
        _exit(0);
    }
    waitpid(kid, NULL, 0);
    file_stat("made by another user", "/dev/mqueue/theirs");
    show("unlink another user's", syscall(SYS_mq_unlink, "theirs"));
    attributes("after inherited send", q);
    close(q);
    syscall(SYS_mq_unlink, "perm");
    listing("at the end");
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    opens();
    messages();
    waits();
    notifications();
    limits();
    return 0;
}
