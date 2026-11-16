// Writes to /proc files, seeks on them, and when their bytes are made: at each read, or at the read
// from the start as Linux's seq_file does. Prints each call and what it gave.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/sendfile.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <unistd.h>

#ifndef RWF_APPEND
#define RWF_APPEND 0x10
#endif

#define R(expr)                                                                                    \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long r_ = (long)(expr);                                                                    \
        printf("%-64s = %ld (%s)\n", #expr, r_, r_ < 0 ? strerror(errno) : "-");                 \
    } while (0)

static char name[32];

static void current(void) {
    memset(name, 0, sizeof(name));
    prctl(PR_GET_NAME, name);
    printf("name now %s|\n", name);
}

static void dump(const char *what, const char *buffer, long n) {
    printf("%s %ld:", what, n);
    for (long i = 0; i < n; i++) {
        unsigned char c = buffer[i];
        if (c >= 32 && c < 127) {
            putchar(c);
        } else {
            printf("\\x%02x", c);
        }
    }
    printf("\n");
}

static void seeks(const char *path) {
    int fd = open(path, O_RDONLY);
    printf("%s:\n", path);
    R(lseek(fd, 0, SEEK_END));
    R(lseek(fd, 5, SEEK_END));
    R(lseek(fd, -1, SEEK_END));
    R(lseek(fd, 7, SEEK_SET));
    R(lseek(fd, 3, SEEK_CUR));
    R(lseek(fd, 0, SEEK_DATA));
    R(lseek(fd, 0, SEEK_HOLE));
    R(lseek(fd, 0x80000000L, SEEK_SET));
    R(lseek(fd, 0x7fffffffL, SEEK_SET));
    close(fd);
}

int main(int argc, char **argv) {
    char buffer[256];
    long n;
    int comm = open("/proc/self/comm", O_RDWR);
    R(write(comm, "renamed-very-long-name\n", 23));
    current();
    R(lseek(comm, 0, SEEK_CUR));
    n = read(comm, buffer, sizeof(buffer));
    dump("comm read", buffer, n);
    R(write(comm, "ab\ncd", 5));
    current();
    n = pread(comm, buffer, sizeof(buffer), 0);
    dump("comm pread", buffer, n);
    R(write(comm, "x", 0));
    current();
    R(write(comm, (void *)16, 4));
    current();
    R(write(comm, "zz", 0x7ffffffffff0L));
    current();
    struct iovec parts[2] = {{"first", 5}, {"second", 6}};
    R(writev(comm, parts, 2));
    current();
    R(pwrite(comm, "seventh", 7, 1000));
    current();
    R(syscall(SYS_pwritev2, comm, parts, 2, -1L, 0, RWF_APPEND));
    current();
    R(syscall(SYS_pwritev2, comm, parts, 2, -1L, 0, 0x1000));
    current();
    R(lseek(comm, 0, SEEK_CUR));
    prctl(PR_SET_NAME, "procwrite");

    // Writes to files with no write of their own.
    const char *others[] = {"/proc/self/cmdline", "/proc/self/environ", "/proc/self/status",
                            "/proc/self/stat",    "/proc/self/maps",    "/proc/self/limits",
                            "/proc/cpuinfo",      "/proc/meminfo",      "/proc/version",
                            "/proc/uptime",       "/proc/loadavg",      "/proc/stat",
                            "/proc/filesystems",  "/proc/mounts",       "/proc/self/auxv",
                            "/proc/self/mounts",  "/proc/self/cgroup",  "/proc/self/statm"};
    for (unsigned i = 0; i < sizeof(others) / sizeof(*others); i++) {
        errno = 0;
        int fd = open(others[i], O_WRONLY);
        long w = fd < 0 ? -2 : write(fd, "1", 1);
        printf("write %-22s open %d = %ld (%s)\n", others[i], fd >= 0, w, w < 0 ? strerror(errno) : "-");
        close(fd);
    }
    R(open("/proc/sys/kernel/ostype", O_WRONLY));
    // Both access bits open for ioctl only, and ask for reading and writing.
    R(open("/proc/sys/kernel/ostype", 3));
    int ioctl_only = open("/proc/self/comm", 3);
    R(ioctl_only >= 0);
    R(read(ioctl_only, buffer, 1));
    R(write(ioctl_only, buffer, 1));
    close(ioctl_only);
    int ostype = open("/proc/sys/kernel/ostype", O_RDONLY);
    R(write(ostype, "x", 1));

    // Seeks.
    seeks("/proc/self/cmdline");
    seeks("/proc/self/environ");
    seeks("/proc/self/auxv");
    seeks("/proc/self/comm");
    seeks("/proc/self/status");
    seeks("/proc/self/maps");
    seeks("/proc/cpuinfo");
    seeks("/proc/sys/kernel/ostype");

    // When the bytes are made: a seq_file keeps what its read from the start made.
    int status = open("/proc/self/status", O_RDONLY);
    prctl(PR_SET_NAME, "before");
    n = read(status, buffer, 12);
    dump("status first", buffer, n);
    prctl(PR_SET_NAME, "after");
    n = read(status, buffer, 12);
    dump("status next", buffer, n);
    n = pread(status, buffer, 12, 6);
    dump("status pread 6", buffer, n);
    prctl(PR_SET_NAME, "again");
    n = read(status, buffer, 12);
    dump("status next", buffer, n);
    lseek(status, 6, SEEK_SET);
    prctl(PR_SET_NAME, "seeked");
    n = read(status, buffer, 12);
    dump("status after seek", buffer, n);
    lseek(status, 0, SEEK_SET);
    prctl(PR_SET_NAME, "zero");
    n = read(status, buffer, 12);
    dump("status at 0", buffer, n);

    int commfd = open("/proc/self/comm", O_RDONLY);
    prctl(PR_SET_NAME, "opened");
    n = read(commfd, buffer, 3);
    dump("comm first", buffer, n);
    prctl(PR_SET_NAME, "changed");
    n = read(commfd, buffer, 10);
    dump("comm rest", buffer, n);
    lseek(commfd, 0, SEEK_SET);
    n = read(commfd, buffer, 10);
    dump("comm again", buffer, n);

    // cmdline and environ read the process's memory at each read.
    int cmdline = open("/proc/self/cmdline", O_RDONLY);
    n = read(cmdline, buffer, 4);
    dump("cmdline first", buffer, n);
    argv[0][5] = 'X';
    n = read(cmdline, buffer, sizeof(buffer));
    dump("cmdline rest", buffer, n);
    n = pread(cmdline, buffer, sizeof(buffer), 0);
    dump("cmdline again", buffer, n);

    // The ostype sysctl is made at each read.
    n = read(ostype, buffer, 2);
    dump("ostype", buffer, n);
    n = read(ostype, buffer, 10);
    dump("ostype rest", buffer, n);
    // A sysctl's read takes a kernel buffer of the size asked for, up to the largest it may have.
    R(pread(ostype, buffer, (4L << 20) - 1, 0));
    R(pread(ostype, buffer, 4L << 20, 0));


    // Vectored reads and writes with flags, and positioned ones.
    const char *flagged[] = {"/proc/self/cmdline", "/proc/self/comm", "/proc/self/status",
                             "/proc/cpuinfo", "/proc/sys/kernel/ostype", "/proc/self/auxv"};
    const int flag_values[] = {0, 1, 2, 4, 8, 0x10, 0x20, 0x1000};
    for (unsigned i = 0; i < sizeof(flagged) / sizeof(*flagged); i++) {
        int fd = open(flagged[i], O_RDONLY);
        for (unsigned f = 0; f < sizeof(flag_values) / sizeof(*flag_values); f++) {
            struct iovec part = {buffer, 4};
            errno = 0;
            long r = syscall(SYS_preadv2, fd, &part, 1, -1L, 0, flag_values[f]);
            printf("preadv2 %-24s %4x = %ld (%s)\n", flagged[i], flag_values[f], r,
                   r < 0 ? strerror(errno) : "-");
            errno = 0;
            r = syscall(SYS_preadv2, fd, &part, 1, 0L, 0, flag_values[f]);
            printf("preadv2 at 0 %-19s %4x = %ld (%s)\n", flagged[i], flag_values[f], r,
                   r < 0 ? strerror(errno) : "-");
        }
        errno = 0;
        long r = pwrite(fd, "x", 1, 0);
        printf("pwrite read-only %-22s = %ld (%s)\n", flagged[i], r, r < 0 ? strerror(errno) : "-");
        close(fd);
    }
    for (unsigned f = 0; f < sizeof(flag_values) / sizeof(*flag_values); f++) {
        errno = 0;
        long r = syscall(SYS_pwritev2, comm, parts, 1, -1L, 0, flag_values[f]);
        printf("pwritev2 comm %4x = %ld (%s)\n", flag_values[f], r, r < 0 ? strerror(errno) : "-");
        current();
    }
    int cmdw = open("/proc/self/cmdline", O_WRONLY);
    R(pwrite(cmdw, "x", 1, 0));
    R(syscall(SYS_pwritev2, cmdw, parts, 1, -1L, 0, 0x10));
    int cpuw = open("/proc/cpuinfo", O_WRONLY);
    R(pwrite(cpuw, "x", 1, 0));
    R(syscall(SYS_pwritev2, cpuw, parts, 1, -1L, 0, 0x10));
    R(syscall(SYS_pwritev2, cpuw, parts, 1, -1L, 0, 0));
    R(pread(1, buffer, 1, 0));
    R(pwrite(0, buffer, 1, 0));

    // What can be spliced from and to.
    const char *sources[] = {"/proc/self/comm", "/proc/self/cmdline", "/proc/self/status",
                             "/proc/cpuinfo", "/proc/sys/kernel/ostype", "/dev/null",
                             "/dev/zero", "/dev/full", "/dev/urandom"};
    int sink = open("/dev/null", O_WRONLY);
    int full = open("/dev/full", O_WRONLY);
    for (unsigned i = 0; i < sizeof(sources) / sizeof(*sources); i++) {
        int fd = open(sources[i], O_RDONLY);
        errno = 0;
        long r = sendfile(sink, fd, 0, 64);
        printf("sendfile from %-24s = %ld (%s)\n", sources[i], r > 0 ? 1 : r, r < 0 ? strerror(errno) : "-");
        errno = 0;
        r = sendfile(full, fd, 0, 64);
        printf("sendfile to /dev/full from %-24s = %ld (%s)\n", sources[i], r > 0 ? 1 : r, r < 0 ? strerror(errno) : "-");
        close(fd);
    }
    int comm_out = open("/proc/self/comm", O_WRONLY);
    int zero = open("/dev/zero", O_RDONLY);
    R(sendfile(comm_out, zero, 0, 8));
    current();
    return 0;
}
