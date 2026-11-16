// Signal descriptors: signalfd4's flags, sizes and refusals, what the file is, reads of every kind of
// signal information (sent, queued, a child's end, an I/O signal), short buffers, several signals in
// one read, waiting and readiness, changing the mask, and a descriptor inherited by a child, which
// reads the child's own signals.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/signalfd.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

struct ssi {
    uint32_t signo;
    int32_t errno_;
    int32_t code;
    uint32_t pid, uid;
    int32_t fd;
    uint32_t tid, band, overrun, trapno;
    int32_t status, int_;
    uint64_t ptr, utime, stime, addr;
    uint16_t addr_lsb, pad2;
    int32_t syscall_;
    uint64_t call_addr;
    uint32_t arch;
    uint8_t pad[28];
};

static pid_t me;

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: errno %d\n", what, errno);
    else
        printf("%s: %ld\n", what, result);
    errno = 0;
}

static long sfd(int fd, uint64_t *mask, unsigned long size, int flags) {
    return syscall(SYS_signalfd4, fd, mask, size, flags);
}

static void print_info(const struct ssi *s) {
    printf("  signo %u errno %d code %d pid %s uid %u fd %d tid %u band %u overrun %u trapno %u status "
           "%d int %d ptr %llx utime %s stime %s addr %llx lsb %u syscall %d call %llx arch %x\n",
           s->signo, s->errno_, s->code, s->pid == (uint32_t)me ? "me" : s->pid ? "other" : "0", s->uid,
           s->fd, s->tid, s->band, s->overrun, s->trapno, s->status, s->int_, (unsigned long long)s->ptr,
           s->utime ? "set" : "0", s->stime ? "set" : "0", (unsigned long long)s->addr, s->addr_lsb,
           s->syscall_, (unsigned long long)s->call_addr, s->arch);
}

static void read_all(const char *what, int fd, size_t room) {
    struct ssi buffer[4];
    memset(buffer, 0xa5, sizeof buffer);
    long n = read(fd, buffer, room);
    show(what, n);
    for (long i = 0; n > 0 && i < n / (long)sizeof(struct ssi); i++)
        print_info(&buffer[i]);
}

static short ready(int fd) {
    struct pollfd p = {fd, POLLIN | POLLOUT | POLLPRI, 0};
    int n = poll(&p, 1, 0);
    return n < 0 ? -errno : p.revents;
}

static void block(int sig) {
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, sig);
    sigprocmask(SIG_BLOCK, &set, NULL);
}

static void file(void) {
    uint64_t mask = 1ull << (SIGUSR1 - 1);
    int fd = sfd(-1, &mask, 8, 0);
    show("make", fd);
    printf("  getfl %o getfd %d\n", fcntl(fd, F_GETFL), fcntl(fd, F_GETFD));
    struct stat st;
    fstat(fd, &st);
    printf("  mode %o nlink %ld uid %d size %ld blksize %ld\n", st.st_mode, (long)st.st_nlink, st.st_uid,
           (long)st.st_size, (long)st.st_blksize);
    struct statfs fs;
    fstatfs(fd, &fs);
    printf("  fs type %lx\n", (unsigned long)fs.f_type);
    char link[64] = {0};
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    readlink(path, link, sizeof link - 1);
    printf("  link %s\n", link);
    char byte = 0;
    show("  write", write(fd, &byte, 1));
    show("  lseek", lseek(fd, 0, SEEK_SET));
    show("  pread", pread(fd, &byte, 1, 0));
    int other = sfd(-1, &mask, 8, 0);
    struct stat st2;
    fstat(other, &st2);
    printf("  same inode as another %d\n", st.st_ino == st2.st_ino);
    close(other);
    int flagged = sfd(-1, &mask, 8, SFD_NONBLOCK | SFD_CLOEXEC);
    printf("flags: getfl %o getfd %d\n", fcntl(flagged, F_GETFL), fcntl(flagged, F_GETFD));
    show("bad flags", sfd(-1, &mask, 8, 1));
    show("size 4", sfd(-1, &mask, 4, 0));
    show("size 16", sfd(-1, &mask, 16, 0));
    show("bad mask", sfd(-1, (uint64_t *)8, 8, 0));
    show("update", sfd(fd, &mask, 8, 0));
    show("update with flags", sfd(fd, &mask, 8, SFD_NONBLOCK));
    printf("  getfl after %o\n", fcntl(fd, F_GETFL));
    int null = open("/dev/null", O_RDONLY);
    show("update not a signal descriptor", sfd(null, &mask, 8, 0));
    show("update bad descriptor", sfd(99, &mask, 8, 0));
    int path_only = open("/tmp", O_PATH);
    show("update a path descriptor", sfd(path_only, &mask, 8, 0));
    close(path_only);
    show("update -2", sfd(-2, &mask, 8, 0));
    close(null);
    show("read short", read(fd, link, 100));
    show("read nonblocking empty", read(flagged, link, 128));
    close(flagged);
    close(fd);
}

static void reads(void) {
    block(SIGUSR1);
    block(SIGUSR2);
    block(SIGRTMIN);
    block(SIGRTMIN + 1);
    block(SIGCHLD);
    block(SIGIO);
    uint64_t mask = 1ull << (SIGUSR1 - 1) | 1ull << (SIGUSR2 - 1) | 1ull << (SIGRTMIN - 1) |
                    1ull << SIGRTMIN | 1ull << (SIGCHLD - 1) | 1ull << (SIGIO - 1) |
                    1ull << (SIGKILL - 1) | 1ull << (SIGSTOP - 1);
    int fd = sfd(-1, &mask, 8, SFD_NONBLOCK);
    printf("empty: ready %x\n", ready(fd));
    kill(me, SIGUSR1);
    printf("pending: ready %x\n", ready(fd));
    read_all("kill", fd, sizeof(struct ssi));
    syscall(SYS_tgkill, me, me, SIGUSR2);
    read_all("tgkill", fd, sizeof(struct ssi));
    union sigval value = {.sival_int = 1234};
    sigqueue(me, SIGRTMIN, value);
    sigqueue(me, SIGRTMIN, value);
    sigqueue(me, SIGRTMIN + 1, value);
    kill(me, SIGUSR1);
    read_all("several", fd, 4 * sizeof(struct ssi));
    read_all("none left", fd, sizeof(struct ssi));
    // Information a program made up, with codes of each layout.
    siginfo_t info;
    memset(&info, 0, sizeof info);
    info.si_signo = SIGUSR1;
    info.si_code = SI_QUEUE;
    info.si_value.sival_ptr = (void *)0x1122334455667788;
    syscall(SYS_rt_sigqueueinfo, me, SIGUSR1, &info);
    read_all("queued pointer", fd, sizeof(struct ssi));
    memset(&info, 0, sizeof info);
    info.si_signo = SIGIO;
    info.si_code = POLL_IN;
    info.si_band = 0x41;
    info.si_fd = 9;
    syscall(SYS_rt_sigqueueinfo, me, SIGIO, &info);
    read_all("poll layout", fd, sizeof(struct ssi));
    memset(&info, 0, sizeof info);
    info.si_signo = SIGCHLD;
    info.si_code = CLD_EXITED;
    info.si_pid = 77;
    info.si_status = 5;
    syscall(SYS_rt_sigqueueinfo, me, SIGCHLD, &info);
    read_all("child layout", fd, sizeof(struct ssi));
    memset(&info, 0, sizeof info);
    info.si_signo = SIGUSR2;
    info.si_code = SI_TIMER;
    info.si_errno = 3;
    syscall(SYS_rt_sigqueueinfo, me, SIGUSR2, &info);
    read_all("timer layout", fd, sizeof(struct ssi));
    memset(&info, 0, sizeof info);
    info.si_signo = SIGUSR2;
    info.si_code = 3;
    info.si_addr = (void *)0x5000;
    syscall(SYS_rt_sigqueueinfo, me, SIGUSR2, &info);
    read_all("plain signal with a positive code", fd, sizeof(struct ssi));
    // A real child's end, and its process's ignored stops.
    pid_t child = fork();
    if (child == 0)
        _exit(3);
    waitid(P_PID, child, (siginfo_t *)&info, WEXITED | WNOWAIT);
    read_all("child ended", fd, sizeof(struct ssi));
    waitpid(child, NULL, 0);
    // An I/O signal from a pipe.
    int ends[2];
    pipe(ends);
    fcntl(ends[0], F_SETOWN, me);
    fcntl(ends[0], F_SETFL, O_ASYNC);
    fcntl(ends[0], F_SETSIG, SIGIO);
    write(ends[1], "x", 1);
    read_all("i/o signal", fd, sizeof(struct ssi));
    close(ends[0]);
    close(ends[1]);
    // Into memory that faults: the signal is gone.
    kill(me, SIGUSR1);
    show("into a bad buffer", read(fd, (void *)8, sizeof(struct ssi)));
    read_all("after the bad buffer", fd, sizeof(struct ssi));
    // A signal not in the mask stays pending.
    uint64_t only_usr2 = 1ull << (SIGUSR2 - 1);
    sfd(fd, &only_usr2, 8, 0);
    kill(me, SIGUSR1);
    printf("other signal: ready %x\n", ready(fd));
    read_all("other signal", fd, sizeof(struct ssi));
    sigset_t pending;
    sigpending(&pending);
    printf("  usr1 still pending %d\n", sigismember(&pending, SIGUSR1));
    sfd(fd, &mask, 8, 0);
    read_all("after widening", fd, sizeof(struct ssi));
    close(fd);
}

static void waits(void) {
    uint64_t mask = 1ull << (SIGUSR1 - 1);
    int fd = sfd(-1, &mask, 8, 0);
    // A blocking read that waits for a child's signal.
    pid_t parent = me;
    pid_t child = fork();
    if (child == 0) {
        usleep(50000);
        kill(parent, SIGUSR1);
        _exit(0);
    }
    read_all("waited", fd, sizeof(struct ssi));
    waitpid(child, NULL, 0);
    // A poll that waits.
    child = fork();
    if (child == 0) {
        usleep(50000);
        kill(parent, SIGUSR1);
        _exit(0);
    }
    struct pollfd p = {fd, POLLIN, 0};
    show("poll waited", poll(&p, 1, 5000));
    printf("  revents %x\n", p.revents);
    read_all("after poll", fd, sizeof(struct ssi));
    waitpid(child, NULL, 0);
    // A child reads its own signals through the descriptor it inherited.
    kill(me, SIGUSR1);
    child = fork();
    if (child == 0) {
        me = getpid();
        int flags = fcntl(fd, F_GETFL);
        fcntl(fd, F_SETFL, flags | O_NONBLOCK);
        read_all("child reads parent's pending", fd, sizeof(struct ssi));
        kill(me, SIGUSR1);
        read_all("child reads its own", fd, sizeof(struct ssi));
        _exit(0);
    }
    waitpid(child, NULL, 0);
    read_all("parent still has its own", fd, sizeof(struct ssi));
    close(fd);
}

static void unblocked(void) {
    // An unblocked signal with a handler is handled, not read: a blocking read is interrupted.
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGUSR1);
    sigprocmask(SIG_UNBLOCK, &set, NULL);
    struct sigaction action = {0};
    action.sa_handler = SIG_IGN;
    sigaction(SIGUSR1, &action, NULL);
    uint64_t mask = 1ull << (SIGUSR1 - 1);
    int fd = sfd(-1, &mask, 8, SFD_NONBLOCK);
    kill(me, SIGUSR1);
    read_all("ignored signal", fd, sizeof(struct ssi));
    close(fd);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    me = getpid();
    file();
    reads();
    waits();
    unblocked();
    return 0;
}
