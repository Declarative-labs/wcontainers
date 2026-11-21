// Process descriptors: pidfd_open's flags and refusals, what the file is (status, flags, link, file
// system), readiness as the process runs, ends and is reaped, pidfd_send_signal with and without
// information, waitid on a descriptor, clone's CLONE_PIDFD, and process_mrelease.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/signalfd.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/file.h>
#include <sys/wait.h>
#include <sys/xattr.h>
#include <unistd.h>

#define PIDFD_NONBLOCK O_NONBLOCK
#define PIDFD_THREAD O_EXCL
#define SIGNAL_THREAD 1
#define SIGNAL_THREAD_GROUP 2
#define SIGNAL_PROCESS_GROUP 4
#ifndef P_PIDFD
#define P_PIDFD 3
#endif

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: errno %d\n", what, errno);
    else
        printf("%s: %ld\n", what, result);
    errno = 0;
}

static int open_pid(pid_t pid, unsigned flags) { return syscall(SYS_pidfd_open, pid, flags); }
static long send(int fd, int sig, siginfo_t *info, unsigned flags) {
    return syscall(SYS_pidfd_send_signal, fd, sig, info, flags);
}
static long mrelease(int fd, unsigned flags) { return syscall(SYS_process_mrelease, fd, flags); }

static short ready(int fd) {
    struct pollfd p = {fd, POLLIN | POLLOUT | POLLPRI, 0};
    int n = poll(&p, 1, 0);
    return n < 0 ? -errno : p.revents;
}

static void describe(const char *what, int fd) {
    struct stat st;
    show(what, fd);
    if (fd < 0)
        return;
    printf("  getfl %o getfd %d\n", fcntl(fd, F_GETFL), fcntl(fd, F_GETFD));
    if (fstat(fd, &st) == 0)
        printf("  mode %o nlink %ld uid %d gid %d size %ld blksize %ld rdev %lx\n", st.st_mode,
               (long)st.st_nlink, st.st_uid, st.st_gid, (long)st.st_size, (long)st.st_blksize,
               (unsigned long)st.st_rdev);
    struct statfs fs;
    if (fstatfs(fd, &fs) == 0)
        printf("  fs type %lx bsize %ld\n", (unsigned long)fs.f_type, (long)fs.f_bsize);
    char path[64], link[128] = {0};
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    long n = readlink(path, link, sizeof link - 1);
    if (n > 0) {
        // The inode number differs by machine.
        char *bracket = strchr(link, '[');
        if (bracket && strncmp(link, "pidfd:", 6) == 0)
            bracket[1] = 0;
        printf("  link %s\n", link);
    }
    char byte;
    show("  read", read(fd, &byte, 1));
    show("  write", write(fd, &byte, 1));
    show("  lseek", lseek(fd, 0, SEEK_SET));
    show("  ioctl", ioctl(fd, FIONREAD, &n));
}

// A child that waits until its pipe closes.
static pid_t waiting_child(int *write_end) {
    int ends[2];
    pipe(ends);
    pid_t pid = fork();
    if (pid == 0) {
        close(ends[1]);
        char byte;
        read(ends[0], &byte, 1);
        _exit(4);
    }
    close(ends[0]);
    *write_end = ends[1];
    return pid;
}

static void opens(void) {
    describe("open self", open_pid(getpid(), 0));
    describe("open self nonblocking", open_pid(getpid(), PIDFD_NONBLOCK));
    describe("open self thread", open_pid(getpid(), PIDFD_THREAD));
    show("open bad flag", open_pid(getpid(), O_CLOEXEC));
    show("open bad flag 2", open_pid(getpid(), 1));
    show("open 0", open_pid(0, 0));
    show("open -1", open_pid(-1, 0));
    show("open none", open_pid(99999, 0));
    int a = open_pid(getpid(), 0), b = open_pid(getpid(), 0);
    struct stat sa, sb;
    fstat(a, &sa);
    fstat(b, &sb);
    printf("same inode for two opens %d\n", sa.st_ino == sb.st_ino);
    struct {
        unsigned bytes;
        int type;
        unsigned char handle[128];
    } handle = {128, 0, {0}};
    int mount;
    show("handle", syscall(SYS_name_to_handle_at, a, "", &handle, &mount, AT_EMPTY_PATH));
    printf("  bytes %u type %x\n", handle.bytes, handle.type);
    handle.bytes = 128;
    show("handle identifier", syscall(SYS_name_to_handle_at, a, "", &handle, &mount, AT_EMPTY_PATH | 0x400));
    printf("  bytes %u type %x\n", handle.bytes, handle.type);
    close(a);
    close(b);
}

static void lifetime(void) {
    int end;
    pid_t child = waiting_child(&end);
    int fd = open_pid(child, 0);
    int quiet = open_pid(child, PIDFD_NONBLOCK);
    printf("running: ready %x\n", ready(fd));
    siginfo_t info = {0};
    show("waitid nonblocking descriptor", waitid(P_PIDFD, quiet, &info, WEXITED));
    show("waitid nonblocking descriptor nohang", waitid(P_PIDFD, quiet, &info, WEXITED | WNOHANG));
    printf("  pid %d\n", info.si_pid);
    close(end);
    struct pollfd until_end = {fd, POLLIN, 0};
    show("poll until the end", poll(&until_end, 1, -1));
    printf("  revents %x\n", until_end.revents);
    show("waitid nowait", waitid(P_PIDFD, fd, &info, WEXITED | WNOWAIT));
    printf("  is child %d code %d status %d\n", info.si_pid == child, info.si_code, info.si_status);
    printf("ended: ready %x\n", ready(fd));
    show("open ended", open_pid(child, 0) >= 0);
    show("signal ended", send(fd, SIGTERM, NULL, 0));
    show("release ended", mrelease(fd, 0));
    struct rusage usage;
    show("waitid", syscall(SYS_waitid, P_PIDFD, fd, &info, WEXITED, &usage));
    printf("  is child %d code %d status %d\n", info.si_pid == child, info.si_code, info.si_status);
    printf("reaped: ready %x\n", ready(fd));
    show("open reaped", open_pid(child, 0));
    show("signal reaped", send(fd, SIGTERM, NULL, 0));
    show("signal 0 reaped", send(fd, 0, NULL, 0));
    show("waitid reaped", waitid(P_PIDFD, fd, &info, WEXITED));
    show("release reaped", mrelease(fd, 0));
    close(fd);
    close(quiet);
}

static volatile int got_signal, got_code, got_value, got_from_me;

static void on_signal(int sig, siginfo_t *info, void *context) {
    (void)context;
    got_signal = sig;
    got_code = info->si_code;
    got_value = info->si_value.sival_int;
    got_from_me = info->si_pid == getpid();
}

static void signals(void) {
    struct sigaction action = {0};
    action.sa_sigaction = on_signal;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGUSR1, &action, NULL);
    int self = open_pid(getpid(), 0);
    show("signal self", send(self, SIGUSR1, NULL, 0));
    printf("  got %d code %d from me %d\n", got_signal, got_code, got_from_me);
    // A reopen for its path alone is no descriptor these calls find (`fdget`).
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", self);
    int path_only = open(path, O_PATH);
    show("path reopen", path_only < 0 ? -1 : 0);
    show("signal through a path descriptor", send(path_only, 0, NULL, 0));
    siginfo_t waited;
    show("wait through a path descriptor", waitid(P_PIDFD, path_only, &waited, WEXITED | WNOHANG));
    close(path_only);
    siginfo_t info;
    memset(&info, 0, sizeof info);
    info.si_signo = SIGUSR1;
    info.si_code = SI_QUEUE;
    info.si_value.sival_int = 77;
    got_signal = 0;
    show("signal self with information", send(self, SIGUSR1, &info, 0));
    printf("  got %d code %d value %d\n", got_signal, got_code, got_value);
    info.si_code = SI_USER;
    got_signal = 0;
    show("signal self code user", send(self, SIGUSR1, &info, 0));
    printf("  got %d code %d\n", got_signal, got_code);
    info.si_signo = SIGUSR2;
    show("signal number differs", send(self, SIGUSR1, &info, 0));
    show("signal bad flags", send(self, SIGUSR1, NULL, 8));
    show("signal two flags", send(self, SIGUSR1, NULL, SIGNAL_THREAD | SIGNAL_THREAD_GROUP));
    got_signal = 0;
    show("signal thread flag", send(self, SIGUSR1, NULL, SIGNAL_THREAD));
    printf("  got %d code %d\n", got_signal, got_code);
    got_signal = 0;
    show("signal group flag", send(self, SIGUSR1, NULL, SIGNAL_THREAD_GROUP));
    printf("  got %d code %d\n", got_signal, got_code);
    got_signal = 0;
    show("signal process group flag", send(self, SIGUSR1, NULL, SIGNAL_PROCESS_GROUP));
    printf("  got %d code %d\n", got_signal, got_code);
    show("signal 0", send(self, 0, NULL, 0));
    show("signal 65", send(self, 65, NULL, 0));
    show("signal bad information", send(self, SIGUSR1, (siginfo_t *)8, 0));
    got_signal = 0;
    show("signal self thread number", send(-10000, SIGUSR1, NULL, 0));
    printf("  got %d code %d\n", got_signal, got_code);
    got_signal = 0;
    show("signal self process number", send(-10001, SIGUSR1, NULL, 0));
    printf("  got %d code %d\n", got_signal, got_code);
    show("signal bad descriptor", send(99, SIGUSR1, NULL, 0));
    show("signal not a process", send(0, SIGUSR1, NULL, 0));
    int thread = open_pid(getpid(), PIDFD_THREAD);
    got_signal = 0;
    show("signal thread descriptor", send(thread, SIGUSR1, NULL, 0));
    printf("  got %d code %d\n", got_signal, got_code);
    close(thread);

    // Another process: information with a code of the kernel's is refused.
    int end;
    pid_t child = waiting_child(&end);
    int fd = open_pid(child, 0);
    info.si_signo = SIGTERM;
    info.si_code = SI_KERNEL;
    show("signal child kernel code", send(fd, SIGTERM, &info, 0));
    info.si_code = SI_USER;
    show("signal child user code", send(fd, SIGTERM, &info, 0));
    info.si_code = SI_QUEUE;
    show("signal child queue code", send(fd, SIGTERM, &info, 0));
    int status;
    waitpid(child, &status, 0);
    printf("  child killed by %d\n", WIFSIGNALED(status) ? WTERMSIG(status) : -1);
    close(end);
    close(fd);

    // A child of another user may not be signalled.
    int ready_ends[2];
    pipe(ready_ends);
    child = fork();
    if (child == 0) {
        setresuid(1000, 1000, 1000);
        write(ready_ends[1], "", 1);
        pause();
        _exit(0);
    }
    char byte;
    read(ready_ends[0], &byte, 1);
    close(ready_ends[0]);
    close(ready_ends[1]);
    fd = open_pid(child, 0);
    struct stat st;
    fstat(fd, &st);
    printf("other user's descriptor: uid %d gid %d mode %o\n", st.st_uid, st.st_gid, st.st_mode);
    show("signal other user", send(fd, SIGTERM, NULL, 0));
    kill(child, SIGKILL);
    waitpid(child, &status, 0);
    close(fd);
    close(self);
}

static void waits(void) {
    siginfo_t info;
    int self = open_pid(getpid(), 0);
    show("waitid self", waitid(P_PIDFD, self, &info, WEXITED | WNOHANG));
    int null = open("/dev/null", O_RDONLY);
    show("waitid not a process", waitid(P_PIDFD, null, &info, WEXITED));
    show("waitid bad descriptor", waitid(P_PIDFD, 99, &info, WEXITED));
    show("waitid negative descriptor", waitid(P_PIDFD, -1, &info, WEXITED));
    close(null);
    close(self);

    // A grandchild's descriptor names no child of the caller.
    int ends[2];
    pipe(ends);
    pid_t child = fork();
    if (child == 0) {
        pid_t grandchild = fork();
        if (grandchild == 0) {
            pause();
            _exit(0);
        }
        write(ends[1], &grandchild, sizeof grandchild);
        waitpid(grandchild, NULL, 0);
        _exit(0);
    }
    pid_t grandchild;
    read(ends[0], &grandchild, sizeof grandchild);
    int fd = open_pid(grandchild, 0);
    show("waitid grandchild", waitid(P_PIDFD, fd, &info, WEXITED | WNOHANG));
    show("signal grandchild", send(fd, SIGKILL, NULL, 0));
    int status;
    waitpid(child, &status, 0);
    printf("  child exited %d\n", WIFEXITED(status) ? WEXITSTATUS(status) : -1);
    close(fd);
    close(ends[0]);
    close(ends[1]);

    // Stops and continues through a descriptor.
    int end;
    child = waiting_child(&end);
    fd = open_pid(child, 0);
    kill(child, SIGSTOP);
    show("waitid stopped", waitid(P_PIDFD, fd, &info, WSTOPPED));
    printf("  code %d status %d\n", info.si_code, info.si_status);
    printf("stopped: ready %x\n", ready(fd));
    kill(child, SIGCONT);
    show("waitid continued", waitid(P_PIDFD, fd, &info, WCONTINUED));
    printf("  code %d status %d\n", info.si_code, info.si_status);
    close(end);
    show("waitid exited", waitid(P_PIDFD, fd, &info, WEXITED));
    printf("  code %d status %d\n", info.si_code, info.si_status);
    close(fd);
}

static int child_main(void *argument) {
    (void)argument;
    _exit(6);
}

static void clones(void) {
    static char stack[65536];
    int fd = -1;
    pid_t child = clone(child_main, stack + sizeof stack, CLONE_PIDFD | SIGCHLD, NULL, &fd);
    printf("clone pidfd: child %d descriptor %d\n", child > 0, fd > 2);
    if (fd >= 0) {
        printf("  getfd %d\n", fcntl(fd, F_GETFD));
        siginfo_t info;
        show("  waitid", waitid(P_PIDFD, fd, &info, WEXITED));
        printf("  is child %d status %d\n", info.si_pid == child, info.si_status);
        close(fd);
    }
    show("clone pidfd with parent tid",
         clone(child_main, stack + sizeof stack, CLONE_PIDFD | CLONE_PARENT_SETTID | SIGCHLD, NULL, &fd));
    show("clone pidfd detached", clone(child_main, stack + sizeof stack, CLONE_PIDFD | 0x00400000 | SIGCHLD, NULL, &fd));
    show("clone pidfd bad pointer", clone(child_main, stack + sizeof stack, CLONE_PIDFD | SIGCHLD, NULL, (int *)8));
    while (waitpid(-1, NULL, WNOHANG) > 0) {
    }
}

static void releases(void) {
    int self = open_pid(getpid(), 0);
    show("release self", mrelease(self, 0));
    show("release bad flags", mrelease(self, 1));
    int null = open("/dev/null", O_RDONLY);
    show("release not a process", mrelease(null, 0));
    show("release bad descriptor", mrelease(99, 0));
    close(null);
    close(self);
    int end;
    pid_t child = waiting_child(&end);
    int fd = open_pid(child, 0);
    show("release running child", mrelease(fd, 0));
    close(end);
    waitpid(child, NULL, 0);
    close(fd);
}

static void inherited(void) {
    // A descriptor survives a fork and names the same process in the child.
    int self = open_pid(getpid(), 0);
    pid_t child = fork();
    if (child == 0) {
        siginfo_t info;
        show("child waitid parent", waitid(P_PIDFD, self, &info, WEXITED | WNOHANG));
        printf("child: ready %x\n", ready(self));
        _exit(0);
    }
    waitpid(child, NULL, 0);
    close(self);
}

// The calls every file takes, on a process descriptor and a signal descriptor.
static void generic(const char *what, int fd) {
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    printf("%s:\n", what);
    show("  fchmod", fchmod(fd, 0600));
    show("  fchown", fchown(fd, 0, 0));
    show("  fchown other", fchown(fd, 1000, 1000));
    show("  chmod path", chmod(path, 0600));
    show("  ftruncate", ftruncate(fd, 0));
    show("  truncate path", truncate(path, 0));
    show("  fallocate", fallocate(fd, 0, 0, 10));
    show("  fallocate past the largest", fallocate(fd, 0, 0x7fffffff, 10));
    show("  fsync", fsync(fd));
    show("  fdatasync", fdatasync(fd));
    struct timespec omit[2] = {{0, UTIME_OMIT}, {0, UTIME_OMIT}};
    show("  utimens omit", futimens(fd, omit));
    show("  utimens now", futimens(fd, NULL));
    show("  mmap", (long)mmap(NULL, 4096, PROT_READ, MAP_SHARED, fd, 0) == -1 ? -1 : 0);
    show("  mmap private", (long)mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, fd, 0) == -1 ? -1 : 0);
    int reopened = open(path, O_RDONLY);
    show("  reopen", reopened);
    if (reopened >= 0)
        close(reopened);
    show("  reopen for writing", open(path, O_WRONLY));
    char *argv[] = {path, NULL};
    show("  execve", execve(path, argv, argv + 1));
    show("  set append", fcntl(fd, F_SETFL, O_APPEND | fcntl(fd, F_GETFL)));
    printf("  getfl %o\n", fcntl(fd, F_GETFL));
    show("  fadvise", posix_fadvise(fd, 0, 0, POSIX_FADV_NORMAL) ? (errno = posix_fadvise(fd, 0, 0, 0), -1) : 0);
    show("  readahead", readahead(fd, 0, 10));
    show("  flock", flock(fd, LOCK_EX));
    struct flock lock = {F_WRLCK, SEEK_SET, 0, 1, 0};
    show("  record lock", fcntl(fd, F_SETLK, &lock));
    show("  sync_file_range", sync_file_range(fd, 0, 1, 0));
    char buffer[16];
    show("  getdents", syscall(SYS_getdents64, fd, buffer, sizeof buffer));
    show("  sendfile from", syscall(SYS_sendfile, 1, fd, NULL, 1));
    int ends[2];
    pipe(ends);
    show("  splice from", splice(fd, NULL, ends[1], NULL, 1, SPLICE_F_NONBLOCK));
    show("  copy_file_range", copy_file_range(fd, NULL, ends[1], NULL, 1, 0));
    close(ends[0]);
    close(ends[1]);
    const char *names[] = {"user.a", "trusted.a", "security.a", "security.capability",
                           "system.posix_acl_access", "bogus.a", "user."};
    for (int n = 0; n < 7; n++) {
        printf("  %s:", names[n]);
        show(" set", fsetxattr(fd, names[n], "v", 1, 0));
        show("  get", fgetxattr(fd, names[n], buffer, sizeof buffer));
        show("  remove", fremovexattr(fd, names[n]));
    }
    show("  list", flistxattr(fd, buffer, sizeof buffer));
}

// pidfs's requests: PIDFD_GET_INFO in each size, FS_IOC_GETVERSION, and the namespace requests'
// refusals (a namespace's own file is not compared: nsfs is not built).
#define IOC(dir, nr, size) (((unsigned)(dir) << 30) | ((unsigned)(size) << 16) | (0xffu << 8) | (nr))
#define GET_INFO(size) IOC(3, 11, size)

static void info(const char *what, int fd, unsigned long long asked, unsigned size) {
    unsigned char buffer[128];
    memset(buffer, 0x5a, sizeof buffer);
    memcpy(buffer, &asked, 8);
    long result = ioctl(fd, GET_INFO(size), buffer);
    if (result < 0) {
        printf("%s info %u: errno %d\n", what, size, errno);
        return;
    }
    unsigned long long mask, cgroup, supported;
    unsigned v[16];
    memcpy(&mask, buffer, 8);
    memcpy(&cgroup, buffer + 8, 8);
    memcpy(v, buffer + 16, 56);
    memcpy(&supported, buffer + 72, 8);
    printf("%s info %u: mask %llx cgroup %d pid %d tgid %d ppid %d", what, size, mask, cgroup != 0,
           v[0] == 0 ? 0 : (pid_t)v[0] == getpid() ? 1 : 2, v[1] == v[0], v[2] == 0 ? 0 : (pid_t)v[2] == getpid() ? 1 : (pid_t)v[2] == getppid() ? 2 : 3);
    printf(" ids %u %u %u %u %u %u %u %u exit %x dump %x signal %u", v[3], v[4], v[5], v[6], v[7],
           v[8], v[9], v[10], v[11], v[12], v[13]);
    printf(" supported %llx tail", size >= 80 ? supported : 0);
    for (unsigned n = size < 80 ? size : 80; n < (size < 96 ? size : 96); n++) printf(" %x", buffer[n]);
    printf(" after %x\n", buffer[size < 127 ? size : 127]);
}

static void namespaces(const char *what, int fd, unsigned long argument) {
    printf("%s namespaces:", what);
    for (unsigned nr = 1; nr <= 10; nr++) {
        long result = ioctl(fd, IOC(0, nr, 0), argument);
        printf(" %d", result < 0 ? -errno : 0);
        if (result >= 0) close(result);
    }
    printf("\n");
}

static void requests(void) {
    int self = open_pid(getpid(), 0);
    unsigned long long all = 0x7f;
    info("self", self, 0, 64);
    info("self", self, all, 64);
    info("self", self, all, 72);
    info("self", self, all, 80);
    info("self", self, all, 96);
    info("self", self, 0x10, 80);
    info("self", self, 0x20, 80);
    info("self", self, 1ull << 40, 80);
    printf("too small: %d errno %d\n", ioctl(self, GET_INFO(63), 0) < 0 ? -1 : 0, errno);
    printf("read only: %d errno %d\n", ioctl(self, IOC(2, 11, 80), 0) < 0 ? -1 : 0, errno);
    printf("number 12: %d errno %d\n", ioctl(self, IOC(3, 12, 80), 0) < 0 ? -1 : 0, errno);
    printf("null: %d errno %d\n", ioctl(self, GET_INFO(80), 0) < 0 ? -1 : 0, errno);
    printf("bad pointer: %d errno %d\n", ioctl(self, GET_INFO(80), 8) < 0 ? -1 : 0, errno);
    unsigned version = 7;
    printf("version: %d %u\n", ioctl(self, 0x80087601, &version), version);
    printf("version null: %d errno %d\n", ioctl(self, 0x80087601, 0) < 0 ? -1 : 0, errno);
    namespaces("self argument", self, 1);
    // The caller's own namespaces open on Linux and are refused here; neither is printed.
    for (unsigned nr = 1; nr <= 10; nr++) {
        long result = ioctl(self, IOC(0, nr, 0), 0);
        if (result >= 0) close(result);
    }
    int write_end;
    pid_t pid = waiting_child(&write_end);
    int child = open_pid(pid, 0);
    info("running", child, all, 80);
    close(write_end);
    siginfo_t si;
    waitid(P_PID, pid, &si, WEXITED | WNOWAIT);
    info("ended", child, all, 80);
    namespaces("ended", child, 0);
    waitpid(pid, NULL, 0);
    info("reaped", child, all, 80);
    info("reaped", child, 0, 80);
    namespaces("reaped", child, 0);
    close(child);
    pid = fork();
    if (pid == 0) {
        pause();
        _exit(0);
    }
    child = open_pid(pid, 0);
    kill(pid, SIGKILL);
    waitpid(pid, NULL, 0);
    info("killed", child, all, 80);
    close(child);
    int ends[2];
    pipe(ends);
    pid = fork();
    if (pid == 0) {
        setresgid(3, 4, 5);
        setresuid(1000, 1001, 1002);
        write(ends[1], "x", 1);
        pause();
        _exit(0);
    }
    char byte;
    read(ends[0], &byte, 1);
    child = open_pid(pid, 0);
    info("other user", child, all, 80);
    namespaces("other user", child, 0);
    kill(pid, SIGKILL);
    waitpid(pid, NULL, 0);
    close(child);
    pid = fork();
    if (pid == 0) {
        prctl(PR_SET_DUMPABLE, 0);
        write(ends[1], "x", 1);
        pause();
        _exit(0);
    }
    read(ends[0], &byte, 1);
    child = open_pid(pid, 0);
    info("not dumpable", child, all, 80);
    namespaces("not dumpable", child, 0);
    kill(pid, SIGKILL);
    waitpid(pid, NULL, 0);
    close(child);
    close(ends[0]);
    close(ends[1]);
    close(self);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    opens();
    lifetime();
    signals();
    waits();
    clones();
    releases();
    inherited();
    requests();
    int fd = open_pid(getpid(), 0);
    generic("process descriptor", fd);
    close(fd);
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGUSR2);
    fd = signalfd(-1, &set, 0);
    generic("signal descriptor", fd);
    close(fd);
    // A process's /proc directory names it to pidfd_send_signal, and only to it.
    int dir = open("/proc/self", O_RDONLY | O_DIRECTORY);
    printf("proc dir signal 0: %ld\n", syscall(SYS_pidfd_send_signal, dir, 0, NULL, 0));
    printf("proc dir signal thread scope: %ld\n",
           syscall(SYS_pidfd_send_signal, dir, SIGCONT, NULL, 1));
    long r = syscall(SYS_pidfd_getfd, dir, 0, 0);
    printf("proc dir getfd: %ld %s\n", r, r < 0 ? strerror(errno) : "");
    char task[64];
    snprintf(task, sizeof task, "/proc/self/task/%d", getpid());
    int thread = open(task, O_RDONLY | O_DIRECTORY);
    r = syscall(SYS_pidfd_send_signal, thread, 0, NULL, 0);
    printf("task dir signal: %ld %s\n", r, r < 0 ? strerror(errno) : "");
    return 0;
}
