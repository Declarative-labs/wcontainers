// What calls leave in a buffer that runs into a page they cannot write: for each call and each count
// of writable bytes before that page, the end of what it stored and whether it failed.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/select.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static unsigned char *page;
static int pipe_ends[2];
static int directory;

static long c_statfs(void *b) { return syscall(SYS_statfs, "/dev/shm", b); }
static long c_fstatfs(void *b) { return syscall(SYS_fstatfs, 0, b); }
static long c_fstat(void *b) { return syscall(SYS_fstat, 0, b); }
static long c_newfstatat(void *b) { return syscall(SYS_newfstatat, AT_FDCWD, "/dev/null", b, 0); }
static long c_statx(void *b) { return syscall(SYS_statx, AT_FDCWD, "/dev/null", 0, 0xfff, b); }
static long c_uname(void *b) { return syscall(SYS_uname, b); }
static long c_clock(void *b) { return syscall(SYS_clock_gettime, CLOCK_MONOTONIC, b); }
static long c_getres(void *b) { return syscall(SYS_clock_getres, CLOCK_MONOTONIC, b); }
static long c_tod(void *b) { return syscall(SYS_gettimeofday, b, 0); }
static long c_zone(void *b) { return syscall(SYS_gettimeofday, 0, b); }
static long c_sysinfo(void *b) { return syscall(SYS_sysinfo, b); }
static long c_rusage(void *b) { return syscall(SYS_getrusage, 0, b); }
static long c_times(void *b) { return syscall(SYS_times, b); }
static long c_prlimit(void *b) { return syscall(SYS_prlimit64, 0, 7, 0, b); }
static long c_getrlimit(void *b) { return syscall(SYS_getrlimit, 7, b); }
static long c_sigpending(void *b) { return syscall(SYS_rt_sigpending, b, 8); }
static long c_sigprocmask(void *b) { return syscall(SYS_rt_sigprocmask, SIG_BLOCK, 0, b, 8); }
static long c_pipe(void *b) {
    long r = syscall(SYS_pipe2, b, 0);
    if (r == 0) { close(((int *)b)[0]); close(((int *)b)[1]); }
    return r;
}
static long c_getcwd(void *b) { return syscall(SYS_getcwd, b, 64); }
static long c_sigaction(void *b) { return syscall(SYS_rt_sigaction, 10, 0, b, 8); }
static long c_altstack(void *b) { return syscall(SYS_sigaltstack, 0, b); }
static long c_sched_param(void *b) { return syscall(SYS_sched_getparam, 0, b); }
static long c_sched_attr(void *b) { return syscall(SYS_sched_getattr, 0, b, 56, 0); }
static long c_affinity(void *b) { return syscall(SYS_sched_getaffinity, 0, 8, b); }
static long c_readlink(void *b) { return syscall(SYS_readlinkat, AT_FDCWD, "/proc/self/fd/0", b, 64); }
static long c_readlink_tmpfs(void *b) { return syscall(SYS_readlinkat, AT_FDCWD, "/dev/shm/p/link", b, 64); }
static long c_getdents(void *b) {
    lseek(directory, 0, SEEK_SET);
    return syscall(SYS_getdents64, directory, b, 200);
}
static long c_read_file(void *b) {
    int fd = open("/dev/shm/p/file", O_RDONLY);
    long r = read(fd, b, 200);
    close(fd);
    return r;
}
static long c_read_pipe(void *b) {
    char drain[200];
    while (read(pipe_ends[0], drain, sizeof drain) > 0) {}
    if (write(pipe_ends[1], "0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ", 72) != 72) return -1;
    return read(pipe_ends[0], b, 72);
}
static long read_path(const char *path, void *b, int count) {
    int fd = open(path, O_RDONLY);
    long r = read(fd, b, count);
    int saved = errno;
    close(fd);
    errno = saved;
    return r;
}
static long c_read_limits(void *b) { return read_path("/proc/self/limits", b, 200); }
static long c_read_cmdline(void *b) { return read_path("/proc/self/cmdline", b, 32); }
static long c_read_maxmap(void *b) { return read_path("/proc/sys/kernel/pid_max", b, 200); }
static long c_read_random(void *b) {
    int fd = open("/dev/urandom", O_RDONLY);
    long r = read(fd, b, 200);
    close(fd);
    return r;
}
static long c_getrandom(void *b) { return syscall(SYS_getrandom, b, 200, 0); }
static long c_getrandom_small(void *b) { return syscall(SYS_getrandom, b, 40, 0); }
static int master = -1, slave = -1;
static long c_read_terminal(void *b) {
    char drain[200];
    while (read(master, drain, sizeof drain) > 0) {}
    if (write(slave, "0123456789abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ", 72) != 72) return -1;
    // A poll lets the terminal's buffer work run.
    poll(&(struct pollfd){master, POLLIN, 0}, 1, 1000);
    long r = read(master, b, 72);
    int saved = errno;
    while (read(master, drain, sizeof drain) > 0) {}
    errno = saved;
    return r;
}
static long c_getresuid(void *b) {
    unsigned *u = b;
    return syscall(SYS_getresuid, u, u + 1, u + 2);
}
static long c_getgroups(void *b) { return syscall(SYS_getgroups, 64, b); }
static long c_getcpu(void *b) {
    unsigned *u = b;
    return syscall(SYS_getcpu, u, u + 1, 0);
}
static long c_wait(void *b) {
    pid_t child = fork();
    if (child == 0) _exit(3);
    long r = syscall(SYS_wait4, child, b, 0, 0);
    if (r < 0) waitpid(child, 0, 0);
    return r;
}
static long c_wait_usage(void *b) {
    pid_t child = fork();
    if (child == 0) _exit(3);
    long r = syscall(SYS_wait4, child, 0, 0, b);
    if (r < 0) waitpid(child, 0, 0);
    return r;
}
static void p_poll(void *b) {
    struct pollfd *p = b;
    for (int i = 0; i < 4; i++) {
        p[i].fd = i == 3 ? -1 : 0;
        p[i].events = POLLIN | POLLOUT;
    }
}
static long c_poll(void *b) { return syscall(SYS_ppoll, b, 4, &(struct timespec){0, 0}, 0, 8); }
static void p_select(void *b) { memset(b, 0xff, 8); }
static long c_select(void *b) { return syscall(SYS_pselect6, 3, b, 0, 0, &(struct timespec){0, 0}, 0); }
static long c_prctl_name(void *b) { return syscall(SYS_prctl, PR_GET_NAME, b, 0, 0, 0); }
static long c_pdeathsig(void *b) { return syscall(SYS_prctl, PR_GET_PDEATHSIG, b, 0, 0, 0); }
static long c_pipe_size(void *b) {
    // FIONREAD's int.
    if (write(pipe_ends[1], "x", 1) != 1) return -1;
    long r = syscall(SYS_ioctl, pipe_ends[0], 0x541B, b);
    char drain[200];
    while (read(pipe_ends[0], drain, sizeof drain) > 0) {}
    return r;
}
static void p_offset(void *b) { memset(b, 0, 8); }
static long c_sendfile_offset(void *b) {
    int in = open("/dev/shm/p/file", O_RDONLY), out = open("/dev/null", O_WRONLY);
    long r = syscall(SYS_sendfile, out, in, b, 5);
    close(in);
    close(out);
    return r;
}

static const struct {
    const char *name;
    int size;
    long (*call)(void *);
    void (*prepare)(void *);
} calls[] = {
    {"statfs", 120, c_statfs}, {"fstatfs", 120, c_fstatfs}, {"fstat", 128, c_fstat},
    {"newfstatat", 128, c_newfstatat}, {"statx", 256, c_statx}, {"uname", 390, c_uname},
    {"clock_gettime", 16, c_clock}, {"clock_getres", 16, c_getres}, {"gettimeofday", 16, c_tod},
    {"timezone", 8, c_zone}, {"sysinfo", 112, c_sysinfo}, {"getrusage", 144, c_rusage},
    {"times", 32, c_times}, {"prlimit", 16, c_prlimit}, {"getrlimit", 16, c_getrlimit},
    {"rt_sigpending", 8, c_sigpending}, {"rt_sigprocmask", 8, c_sigprocmask}, {"pipe2", 8, c_pipe},
    {"getcwd", 64, c_getcwd}, {"rt_sigaction", 32, c_sigaction}, {"sigaltstack", 24, c_altstack},
    {"sched_getparam", 4, c_sched_param}, {"sched_getattr", 56, c_sched_attr},
    {"sched_getaffinity", 8, c_affinity}, {"readlink proc", 64, c_readlink},
    {"readlink tmpfs", 64, c_readlink_tmpfs}, {"getdents64", 200, c_getdents},
    {"read file", 200, c_read_file}, {"read pipe", 72, c_read_pipe}, {"read terminal", 72, c_read_terminal},
    {"read limits", 200, c_read_limits}, {"read cmdline", 32, c_read_cmdline},
    {"read pid_max", 24, c_read_maxmap}, {"read urandom", 200, c_read_random}, {"getrandom", 200, c_getrandom},
    {"getrandom small", 40, c_getrandom_small}, {"getresuid", 12, c_getresuid},
    {"getgroups", 48, c_getgroups}, {"getcpu", 8, c_getcpu}, {"wait4 status", 4, c_wait},
    {"wait4 usage", 144, c_wait_usage}, {"ppoll", 32, c_poll, p_poll}, {"pselect6", 8, c_select, p_select},
    {"PR_GET_NAME", 16, c_prctl_name}, {"PR_GET_PDEATHSIG", 4, c_pdeathsig},
    {"FIONREAD", 4, c_pipe_size}, {"sendfile offset", 8, c_sendfile_offset, p_offset},
};

int main(void) {
    page = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (pipe2(pipe_ends, O_NONBLOCK) != 0) return 1;
    mkdir("/dev/shm/p", 0755);
    int fd = open("/dev/shm/p/file", O_CREAT | O_WRONLY, 0644);
    for (int i = 0; i < 250; i++) {
        char c = 'a' + i % 26;
        if (write(fd, &c, 1) != 1) return 1;
    }
    close(fd);
    symlink("/a/link/target/of/twenty-nine", "/dev/shm/p/link");
    mkdir("/dev/shm/d", 0755);
    const char *names[] = {"a", "bb", "ccc", "dddd", "eeeeeeeee", "fffffffffffffffff"};
    for (unsigned i = 0; i < 6; i++) {
        char path[64];
        snprintf(path, sizeof path, "/dev/shm/d/%s", names[i]);
        close(open(path, O_CREAT | O_WRONLY, 0644));
    }
    directory = open("/dev/shm/d", O_RDONLY | O_DIRECTORY);
    master = posix_openpt(O_RDWR | O_NOCTTY | O_NONBLOCK);
    if (master < 0 || grantpt(master) || unlockpt(master)) return 1;
    slave = open(ptsname(master), O_RDWR | O_NOCTTY);
    if (slave < 0) return 1;
    for (unsigned i = 0; i < sizeof calls / sizeof calls[0]; i++) {
        printf("%s:", calls[i].name);
        for (int writable = 0; writable <= calls[i].size; writable++) {
            int most = 0;
            long answer = 0;
            for (int pass = 0; pass < 2; pass++) {
                const unsigned char fill = pass ? 0x55 : 0xaa;
                unsigned char *buffer = page + 4096 - writable;
                mprotect(page + 4096, 4096, PROT_READ | PROT_WRITE);
                memset(page, fill, 8192);
                // Calls that read their buffer first find it filled in, the part they cannot
                // write too.
                if (calls[i].prepare) calls[i].prepare(buffer);
                unsigned char before[512];
                memcpy(before, buffer, writable);
                mprotect(page + 4096, 4096, PROT_READ);
                errno = 0;
                answer = calls[i].call(buffer);
                if (answer < 0) answer = -errno;
                for (int at = 0; at < writable; at++)
                    if (buffer[at] != before[at] && at + 1 > most) most = at + 1;
            }
            printf(" %d%s", most, answer < 0 ? "!" : "");
        }
        printf("\n");
    }
    return 0;
}
