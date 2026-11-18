// The container's own files as programs meet them: epoll on regular files of the root, /tmp
// and /dev/shm, a pipe write from anonymous memory running on into a mapped program's pages not
// read yet, directory change notices (dnotify), and what the root says it is (overlayfs).
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/xattr.h>
#include <sys/uio.h>
#include <unistd.h>

#define PAGE 4096

static unsigned sum(const unsigned char *bytes, long length) {
    unsigned total = 0;
    for (long i = 0; i < length; i++) {
        total = total * 31 + bytes[i];
    }
    return total;
}

static void epoll_on(const char *path) {
    int fd = open(path, O_RDWR | O_CREAT, 0600);
    int ep = epoll_create1(0);
    struct epoll_event event = {.events = EPOLLIN};
    int r = epoll_ctl(ep, EPOLL_CTL_ADD, fd, &event);
    printf("%s: epoll_ctl %d errno %d\n", path, r, r ? errno : 0);
    close(ep);
    close(fd);
    unlink(path);
}

// Five pages: one anonymous, then four of `path` from `page` on, mapped privately, not touched.
static unsigned char *span(const char *path, long page) {
    unsigned char *region =
        mmap(NULL, 5 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    memset(region, 'A', PAGE);
    int fd = open(path, O_RDONLY);
    if (mmap(region + PAGE, 4 * PAGE, PROT_READ, MAP_PRIVATE | MAP_FIXED, fd, page * PAGE) ==
        MAP_FAILED) {
        printf("map %s: %d\n", path, errno);
    }
    close(fd);
    return region;
}

static volatile sig_atomic_t notices, last_fd;
static void noticed(int signal, siginfo_t *info, void *context) {
    (void)signal;
    (void)context;
    notices++;
    last_fd = info->si_fd;
}

static void dnotify(const char *directory) {
    int fd = open(directory, O_RDONLY);
    int r = fcntl(fd, F_SETSIG, SIGRTMIN + 1);
    printf("%s: F_SETSIG %d\n", directory, r);
    r = fcntl(fd, F_NOTIFY, DN_CREATE | DN_DELETE | DN_MULTISHOT);
    printf("%s: F_NOTIFY %d errno %d\n", directory, r, r ? errno : 0);
    char path[256];
    snprintf(path, sizeof path, "%s/noticed", directory);
    close(open(path, O_RDWR | O_CREAT, 0600));
    unlink(path);
    printf("%s: notices %d from the directory %d\n", directory, notices, last_fd == fd);
    notices = 0;
    // A notice for what the mask leaves out does not come; one-shot notices end after one.
    r = fcntl(fd, F_NOTIFY, DN_ATTRIB);
    close(open(path, O_RDWR | O_CREAT, 0600));
    chmod(path, 0644);
    chmod(path, 0600);
    printf("%s: F_NOTIFY %d attribute notices %d\n", directory, r, notices);
    unlink(path);
    notices = 0;
    // Not a directory, and an unknown mask bit.
    int file = open(path, O_RDWR | O_CREAT, 0600);
    r = fcntl(file, F_NOTIFY, DN_CREATE);
    printf("%s: F_NOTIFY on a file %d errno %d\n", directory, r, r ? errno : 0);
    r = fcntl(fd, F_NOTIFY, 0x100);
    printf("%s: F_NOTIFY unknown bit %d errno %d\n", directory, r, r ? errno : 0);
    close(file);
    unlink(path);
    // Turned off, then a closed descriptor ends its notices.
    fcntl(fd, F_NOTIFY, 0);
    close(open(path, O_RDWR | O_CREAT, 0600));
    unlink(path);
    printf("%s: after off %d\n", directory, notices);
    // Renames within the directory and out of it, and a change of the directory itself.
    char other[256], inside[256];
    snprintf(other, sizeof other, "%s/renamed", directory);
    snprintf(inside, sizeof inside, "%s/sub", directory);
    mkdir(inside, 0700);
    close(open(path, O_RDWR | O_CREAT, 0600));
    r = fcntl(fd, F_NOTIFY, DN_RENAME | DN_MULTISHOT);
    rename(path, other);
    printf("%s: F_NOTIFY %d rename within %d\n", directory, r, notices);
    notices = 0;
    snprintf(path, sizeof path, "%s/sub/moved", directory);
    rename(other, path);
    printf("%s: rename out %d\n", directory, notices);
    notices = 0;
    fcntl(fd, F_NOTIFY, DN_ATTRIB | DN_MULTISHOT);
    chmod(directory, 01777);
    chmod(directory, directory[1] == 't' ? 01777 : 0700);
    printf("%s: own attributes %d\n", directory, notices);
    notices = 0;
    unlink(path);
    rmdir(inside);
    printf("%s: removed below %d\n", directory, notices);
    notices = 0;
    snprintf(path, sizeof path, "%s/noticed", directory);
    fcntl(fd, F_NOTIFY, DN_CREATE | DN_MULTISHOT);
    close(fd);
    close(open(path, O_RDWR | O_CREAT, 0600));
    unlink(path);
    printf("%s: after close %d\n", directory, notices);
}

#ifndef AT_HANDLE_FID
#define AT_HANDLE_FID 0x200
#endif

// The first line of a mount listing up to the layers' paths, which are the machine's own, from
// the file system's part of mountinfo's (its numbers are the machine's too).
static void first_mount(const char *listing) {
    char line[4096] = "";
    FILE *file = fopen(listing, "r");
    fgets(line, sizeof line, file);
    fclose(file);
    char *paths = strstr(line, "lowerdir=");
    char *last = strstr(line, ",nouserxattr");
    if (paths) {
        *paths = 0;
    }
    char *shown = strstr(line, " - ") ? strstr(line, " - ") : line;
    printf("%s: %s... %s", listing, shown, last ? last : "(no nouserxattr)\n");
}

static void presents(void) {
    struct statfs fs;
    statfs("/", &fs);
    printf("statfs / type %lx namelen %ld flags %lx bsize %ld fsid %s\n", (long)fs.f_type,
           (long)fs.f_namelen, (long)fs.f_flags, (long)fs.f_bsize,
           fs.f_fsid.__val[0] || fs.f_fsid.__val[1] ? "set" : "none");
    first_mount("/proc/mounts");
    first_mount("/proc/self/mountinfo");

    int fd = open("/presents", O_CREAT | O_RDWR, 0644);
    const char *names[] = {"foo.bar", "user.x", "trusted.x", "security.x", "system.x", "user.",
                           "trusted.overlay.x"};
    for (unsigned i = 0; i < sizeof names / sizeof *names; i++) {
        char value[16];
        errno = 0;
        int set = fsetxattr(fd, names[i], "v", 1, 0);
        int set_errno = errno;
        errno = 0;
        int got = fgetxattr(fd, names[i], value, sizeof value);
        printf("xattr %s set %d %s get %d %s\n", names[i], set, strerror(set_errno), got,
               strerror(errno));
    }
    struct {
        struct file_handle header;
        unsigned char bytes[128];
    } handle = {.header.handle_bytes = 128};
    int mount;
    errno = 0;
    int r = name_to_handle_at(AT_FDCWD, "/presents", &handle.header, &mount, 0);
    printf("handle %d %s\n", r, strerror(errno));
    handle.header.handle_bytes = 128;
    r = name_to_handle_at(AT_FDCWD, "/bin/busybox", &handle.header, &mount, AT_HANDLE_FID);
    printf("identifier %d bytes %u type %#x\n", r, handle.header.handle_bytes,
           handle.header.handle_type);
    errno = 0;
    r = fcntl(fd, F_SETLEASE, F_WRLCK);
    printf("write lease %d %s\n", r, strerror(errno));
    fcntl(fd, F_SETLEASE, F_UNLCK);
    close(fd);
    errno = 0;
    fd = open("/presents", O_RDONLY | O_DIRECT);
    printf("direct open %s\n", fd >= 0 ? "yes" : strerror(errno));
    close(fd);
    unlink("/presents");
}

static char direct_buffer[3 * 65536] __attribute__((aligned(65536)));

static void direct_both(int fd, long memory, long length, long offset) {
    errno = 0;
    long r = pwrite(fd, direct_buffer + memory, length, offset);
    printf("direct write memory+%ld length %ld at %ld: %ld %s\n", memory, length, offset, r,
           r < 0 ? strerror(errno) : "");
    errno = 0;
    r = pread(fd, direct_buffer + memory, length, offset);
    printf("direct read memory+%ld length %ld at %ld: %ld %s\n", memory, length, offset, r,
           r < 0 ? strerror(errno) : "");
}

// Direct reads and writes on the root keep its upper layer's alignment.
static void direct(void) {
    int fd = open("/direct", O_CREAT | O_RDWR | O_DIRECT, 0600);
    unsigned char sx[256] = {0};
    syscall(SYS_statx, fd, "", AT_EMPTY_PATH, 0x2000, sx); // STATX_DIOALIGN
    unsigned *words = (unsigned *)sx;
    printf("dioalign %d memory %u offset %u\n", !!(words[0] & 0x2000), words[152 / 4],
           words[156 / 4]);
    memset(direct_buffer, 'd', sizeof direct_buffer);
    direct_both(fd, 0, 4096, 0);
    direct_both(fd, 512, 512, 512);
    direct_both(fd, 1, 4096, 0);
    direct_both(fd, 0, 4095, 0);
    direct_both(fd, 0, 4096, 1);
    direct_both(fd, 4, 512, 0);
    direct_both(fd, 0, 100, 1 << 20);
    printf("size %ld\n", (long)lseek(fd, 0, SEEK_END));
    direct_both(fd, 0, 4096, 4096 * 300);
    struct iovec parts[2] = {{direct_buffer, 512}, {direct_buffer + 1024, 1024}};
    errno = 0;
    printf("direct writev %zd %s\n", pwritev(fd, parts, 2, 0), strerror(errno));
    parts[1].iov_len = 1000;
    errno = 0;
    printf("direct writev short segment %zd %s\n", pwritev(fd, parts, 2, 0), strerror(errno));
    fcntl(fd, F_SETFL, 0);
    errno = 0;
    printf("after clearing O_DIRECT %zd %s\n", pwrite(fd, direct_buffer + 1, 10, 3), strerror(errno));
    close(fd);
    unlink("/direct");
}

int main(void) {
    presents();
    direct();
    epoll_on("/tmp/epollfile");
    epoll_on("/dev/shm/epollfile");
    epoll_on("/root/epollfile");

    int pipes[2];
    pipe(pipes);
    unsigned char *region = span("/bin/busybox", 8);
    ssize_t written = write(pipes[1], region, 5 * PAGE);
    printf("write from anonymous then file %zd\n", written);
    static unsigned char back[5 * PAGE];
    ssize_t got = read(pipes[0], back, sizeof back);
    printf("read back %zd sum %08x\n", got, sum(back, got > 0 ? got : 0));
    region = span("/bin/busybox", 128);
    struct iovec parts[2] = {{region + 4000, 96}, {region + PAGE, 3 * PAGE}};
    written = writev(pipes[1], parts, 2);
    got = read(pipes[0], back, sizeof back);
    printf("writev across %zd read back %zd sum %08x\n", written, got, sum(back, got > 0 ? got : 0));

    struct sigaction action = {.sa_sigaction = noticed, .sa_flags = SA_SIGINFO};
    sigaction(SIGRTMIN + 1, &action, NULL);
    sigaction(SIGIO, &action, NULL);
    dnotify("/tmp");
    dnotify("/root");
    return 0;
}
