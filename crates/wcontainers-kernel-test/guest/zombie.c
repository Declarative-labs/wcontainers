// What every call that names a child that has ended and is not yet reaped answers: its /proc files and
// links, its CPU clocks, signals, groups, limits and scheduling. Numbers that differ between machines
// (process numbers, times) are printed as how they relate to known ones.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

static int readfile(pid_t c, const char *name, char *buf, int size) {
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/%s", c, name);
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        printf("%s: open errno %d\n", name, errno);
        return -1;
    }
    int n = read(fd, buf, size - 1);
    close(fd);
    printf("%s: read %d\n", name, n > 0 ? (n > 1 ? 2 : 1) : n);
    if (n >= 0) {
        buf[n] = 0;
    }
    return n;
}

// The fields of stat a zombie's values are known for, by their number in proc(5).
static void stat_fields(pid_t c) {
    char buf[1024];
    if (readfile(c, "stat", buf, sizeof buf) <= 0) {
        return;
    }
    char *close_paren = strrchr(buf, ')');
    printf("stat comm %.*s\n", (int)(close_paren - strchr(buf, '(') + 1), strchr(buf, '('));
    char *field = close_paren + 2;
    for (int number = 3; field && *field; number++) {
        char *end = strchr(field, ' ');
        int length = end ? (int)(end - field) : (int)strcspn(field, "\n");
        long long value = atoll(field);
        switch (number) {
        case 3: case 7: case 8: case 18: case 19: case 20: case 23: case 24: case 25: case 31: case 32: case 33:
        case 34: case 40: case 41: case 52:
            printf("stat %d: %.*s\n", number, length, field);
            break;
        case 4:
            printf("stat 4 is the parent: %d\n", value == getpid());
            break;
        case 5: case 6:
            printf("stat %d is ours: %d\n", number, value == (number == 5 ? getpgid(0) : getsid(0)));
            break;
        }
        field = end ? end + 1 : NULL;
    }
}

static void status_lines(pid_t c) {
    char buf[4096];
    if (readfile(c, "status", buf, sizeof buf) <= 0) {
        return;
    }
    const char *wanted[] = {"Name:", "Umask:", "State:", "FDSize:", "VmSize:", "Threads:", "SigBlk:",
                            "SigIgn:", "SigCgt:", "CapEff:", "Seccomp:", "Seccomp_filters:"};
    for (char *line = strtok(buf, "\n"); line; line = strtok(NULL, "\n")) {
        for (unsigned i = 0; i < sizeof wanted / sizeof *wanted; i++) {
            if (strncmp(line, wanted[i], strlen(wanted[i])) == 0) {
                printf("status [%s]\n", line);
            }
        }
    }
}

static void proc_link(pid_t c, const char *name) {
    char path[64], target[256];
    snprintf(path, sizeof path, "/proc/%d/%s", c, name);
    p(name, readlink(path, target, sizeof target));
    int fd = open(path, O_RDONLY | O_PATH);
    p("open through it", fd);
    if (fd >= 0) {
        close(fd);
    }
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    sigset_t blocked;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGUSR2);
    sigaddset(&blocked, SIGRTMIN + 3);
    pid_t c = fork();
    if (c == 0) {
        signal(SIGHUP, SIG_IGN);
        sigprocmask(SIG_BLOCK, &blocked, NULL);
        setpriority(PRIO_PROCESS, 0, 3);
        _exit(3);
    }
    siginfo_t si;
    waitid(P_PID, c, &si, WEXITED | WNOWAIT);
    char buf[8192];

    stat_fields(c);
    status_lines(c);
    const char *files[] = {"statm", "maps", "cmdline", "environ", "auxv", "comm", "limits", "mounts", "cgroup"};
    for (unsigned i = 0; i < sizeof files / sizeof *files; i++) {
        if (readfile(c, files[i], buf, sizeof buf) > 0 && (i == 0 || i == 5)) {
            printf("  [%.*s]\n", (int)strcspn(buf, "\n"), buf);
        }
    }
    proc_link(c, "exe");
    proc_link(c, "cwd");
    proc_link(c, "root");
    snprintf(buf, sizeof buf, "/proc/%d/fd", c);
    int fd = open(buf, O_RDONLY | O_DIRECTORY);
    p("open fd directory", fd);
    if (fd >= 0) {
        p("its entries", syscall(SYS_getdents64, fd, buf, sizeof buf));
        close(fd);
    }

    clockid_t id;
    p("getcpuclockid", clock_getcpuclockid(c, &id));
    struct timespec t;
    p("gettime of its process clock", clock_gettime(id, &t));
    p("getres of its process clock", clock_getres(id, &t));
    p("its scheduler clock", syscall(SYS_clock_gettime, (~(clockid_t)c << 3) | 2, &t));
    p("its thread clock", syscall(SYS_clock_gettime, (~(clockid_t)c << 3) | 6, &t));

    p("kill 0", kill(c, 0));
    p("kill usr1", kill(c, SIGUSR1));
    p("tgkill 0", syscall(SYS_tgkill, c, c, 0));
    p("tkill 0", syscall(SYS_tkill, c, 0));
    union sigval value = {0};
    p("sigqueue", sigqueue(c, SIGUSR1, value));

    p("getpgid is ours", getpgid(c) == getpgid(0));
    p("getsid is ours", getsid(c) == getsid(0));
    p("setpgid", setpgid(c, c));
    p("getpgid is its own", getpgid(c) == c);
    p("setpgid back", setpgid(c, getpgid(0)));

    struct rlimit limit;
    p("prlimit get", prlimit(c, RLIMIT_NOFILE, NULL, &limit));
    limit.rlim_cur = 1000;
    p("prlimit set", prlimit(c, RLIMIT_NOFILE, &limit, NULL));
    p("prlimit get again", prlimit(c, RLIMIT_NOFILE, NULL, &limit));
    printf("its limit now %ld\n", (long)limit.rlim_cur);

    cpu_set_t set;
    p("getaffinity", sched_getaffinity(c, sizeof set, &set));
    p("getscheduler", sched_getscheduler(c));
    errno = 0;
    p("getpriority", getpriority(PRIO_PROCESS, c));
    p("ioprio_get", syscall(SYS_ioprio_get, 1, c));
    p("pidfd_open", syscall(SYS_pidfd_open, c, 0) >= 0);
    char remote[4];
    struct iovec local = {remote, 4}, there = {(void *)0x400000, 4};
    p("process_vm_readv", process_vm_readv(c, &local, 1, &there, 1, 0));
    struct rusage usage;
    getrusage(RUSAGE_CHILDREN, &usage);
    printf("children's time before the reap is none: %d\n", usage.ru_utime.tv_sec == 0 && usage.ru_utime.tv_usec == 0 &&
                                                        usage.ru_stime.tv_sec == 0 && usage.ru_stime.tv_usec == 0);

    int status;
    p("reaped", waitpid(c, &status, 0) == c);
    printf("status %x\n", status);
    p("gettime of its clock after the reap", clock_gettime(id, &t));
    snprintf(buf, sizeof buf, "/proc/%d/stat", c);
    p("open stat after the reap", open(buf, O_RDONLY));

    // A group whose only member has ended and is not yet reaped still exists.
    pid_t leader = fork();
    if (leader == 0) {
        setpgid(0, 0);
        _exit(0);
    }
    setpgid(leader, leader);
    waitid(P_PID, leader, &si, WEXITED | WNOWAIT);
    p("kill its group", kill(-leader, 0));
    p("getpgid of it is itself", getpgid(leader) == leader);
    pid_t joiner = fork();
    if (joiner == 0) {
        p("join its group", setpgid(0, leader));
        _exit(0);
    }
    waitpid(joiner, NULL, 0);
    pid_t starter = fork();
    if (starter == 0) {
        p("join it", setpgid(0, leader));
        p("setsid as a member", setsid() > 0);
        _exit(0);
    }
    waitpid(starter, NULL, 0);
    // A process whose own group keeps only a child that has ended cannot start a session.
    pid_t former = fork();
    if (former == 0) {
        pid_t group = getpgid(0);
        setpgid(0, 0);
        pid_t member = fork();
        if (member == 0) {
            _exit(0);
        }
        setpgid(0, group);
        waitid(P_PID, member, &si, WEXITED | WNOWAIT);
        p("setsid with an ended member", setsid());
        waitpid(member, NULL, 0);
        p("setsid once it is reaped", setsid() > 0);
        _exit(0);
    }
    waitpid(former, NULL, 0);
    waitpid(leader, NULL, 0);
    p("kill its group after the reap", kill(-leader, 0));
    return 0;
}
