// The container's own files as programs meet them: epoll on regular files of the root, /tmp
// and /dev/shm, a pipe write from anonymous memory running on into a mapped program's pages not
// read yet, and directory change notices (dnotify).
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/mman.h>
#include <sys/stat.h>
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

int main(void) {
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
