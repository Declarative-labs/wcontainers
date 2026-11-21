// fcntl's record locks in one process: a process's own locks never conflict, locks of open files
// (OFD) conflict with every other owner, ranges join and split, closing lets them go, and every
// argument check. Lock ranges are shown by asking another open file what each byte meets.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

static void show(const char *label, int result, struct flock *lock) {
    if (result < 0) {
        printf("%-44s -1 %s\n", label, strerror(errno));
        return;
    }
    const char *pid = lock->l_pid == getpid() ? "self" : lock->l_pid == -1 ? "-1" : "other";
    if (lock->l_pid == 0) {
        pid = "0";
    }
    printf("%-44s %d type %d whence %d start %lld len %lld pid %s\n", label, result, lock->l_type,
           lock->l_whence, (long long)lock->l_start, (long long)lock->l_len, pid);
}

static int lock(int fd, int command, short type, short whence, off_t start, off_t length) {
    struct flock request = {.l_type = type, .l_whence = whence, .l_start = start, .l_len = length};
    errno = 0;
    return fcntl(fd, command, &request);
}

static void try(const char *label, int fd, int command, short type, short whence, off_t start,
                off_t length) {
    struct flock request = {.l_type = type, .l_whence = whence, .l_start = start, .l_len = length};
    errno = 0;
    int result = fcntl(fd, command, &request);
    show(label, result, &request);
}

// What a write lock on each byte from 0 to 40 meets, asked through `probe` (another open file).
static void ranges(const char *label, int probe) {
    printf("%s:", label);
    for (int at = 0; at < 40; at++) {
        struct flock request = {.l_type = F_WRLCK, .l_whence = SEEK_SET, .l_start = at, .l_len = 1};
        if (fcntl(probe, F_OFD_GETLK, &request) < 0) {
            printf(" error %d", errno);
            continue;
        }
        printf(" %c", request.l_type == F_UNLCK ? '.' : request.l_type == F_RDLCK ? 'r' : 'w');
        if (request.l_type != F_UNLCK && request.l_start == at) {
            printf("[%lld+%lld]", (long long)request.l_start, (long long)request.l_len);
        }
    }
    printf("\n");
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    int fd = open("/tmp/locked", O_RDWR | O_CREAT | O_TRUNC, 0644);
    write(fd, "0123456789abcdefghij", 20);
    int probe = open("/tmp/locked", O_RDWR);

    // A process's own locks join, split and change kind.
    lock(fd, F_SETLK, F_RDLCK, SEEK_SET, 0, 5);
    lock(fd, F_SETLK, F_RDLCK, SEEK_SET, 5, 5);
    ranges("joined", probe);
    lock(fd, F_SETLK, F_WRLCK, SEEK_SET, 3, 2);
    ranges("split", probe);
    lock(fd, F_SETLK, F_UNLCK, SEEK_SET, 2, 6);
    ranges("cut", probe);
    lock(fd, F_SETLK, F_WRLCK, SEEK_SET, 12, 3);
    lock(fd, F_SETLK, F_WRLCK, SEEK_SET, 16, 3);
    lock(fd, F_SETLK, F_RDLCK, SEEK_SET, 15, 1);
    ranges("three", probe);
    lock(fd, F_SETLK, F_WRLCK, SEEK_SET, 15, 1);
    ranges("bridged", probe);
    lock(fd, F_SETLK, F_RDLCK, SEEK_SET, 0, 0);
    ranges("to the end", probe);
    lock(fd, F_SETLK, F_UNLCK, SEEK_SET, 0, 0);
    ranges("none", probe);

    // Own locks never conflict; F_GETLK finds nothing of the caller's.
    lock(fd, F_SETLK, F_WRLCK, SEEK_SET, 0, 10);
    try("own write lock again through another file", probe, F_SETLK, F_WRLCK, SEEK_SET, 0, 10);
    try("getlk of own lock", fd, F_GETLK, F_WRLCK, SEEK_SET, 0, 10);
    // Open file locks conflict with the process's locks and with other open files'.
    try("ofd getlk meets the process's lock", probe, F_OFD_GETLK, F_WRLCK, SEEK_SET, 5, 20);
    try("ofd setlk meets it", probe, F_OFD_SETLK, F_RDLCK, SEEK_SET, 5, 1);
    try("ofd setlk past it", probe, F_OFD_SETLK, F_RDLCK, SEEK_SET, 10, 5);
    try("ofd write lock beside it", probe, F_OFD_SETLK, F_WRLCK, SEEK_SET, 15, 5);
    try("process lock meets ofd", fd, F_SETLK, F_RDLCK, SEEK_SET, 12, 1);
    try("process getlk meets ofd", fd, F_GETLK, F_RDLCK, SEEK_SET, 11, 100);
    try("ofd getlk of unlock finds its own", probe, F_OFD_GETLK, F_UNLCK, SEEK_SET, 0, 0);
    try("ofd getlk of unlock on another file", fd, F_OFD_GETLK, F_UNLCK, SEEK_SET, 0, 0);
    try("ofd setlkw past every lock", probe, F_OFD_SETLKW, F_RDLCK, SEEK_SET, 30, 1);
    try("setlkw past every lock", fd, F_SETLKW, F_RDLCK, SEEK_SET, 40, 1);
    int third = open("/tmp/locked", O_RDWR);
    try("third file meets the read lock", third, F_OFD_SETLK, F_WRLCK, SEEK_SET, 12, 1);
    try("third file shares the read lock", third, F_OFD_SETLK, F_RDLCK, SEEK_SET, 12, 1);
    ranges("seen by the third file", third);
    close(third);
    ranges("after the third file closed", open("/tmp/locked", O_RDONLY));

    // A process's locks go when any descriptor of the file closes, except one opened for its path.
    int path = open("/tmp/locked", O_PATH);
    close(path);
    ranges("after closing a path descriptor", probe);
    int other = open("/tmp/locked", O_RDONLY);
    close(other);
    ranges("after closing another descriptor", probe);
    // An open file's locks go with its last descriptor.
    int copy = dup(probe);
    lock(probe, F_OFD_SETLK, F_UNLCK, SEEK_SET, 0, 0);
    lock(probe, F_OFD_SETLK, F_WRLCK, SEEK_SET, 0, 4);
    close(probe);
    ranges("ofd lock kept by a copy", fd);
    int second = dup2(copy, 50);
    close(copy);
    ranges("and by a second copy", fd);
    close(second);
    ranges("gone with the last", fd);
    probe = open("/tmp/locked", O_RDWR);

    // The range: where it is measured from, signs, overflow, and the checks in Linux's order.
    lseek(fd, 7, SEEK_SET);
    try("from the position", fd, F_SETLK, F_WRLCK, SEEK_CUR, 1, 2);
    try("from the end, backwards", fd, F_SETLK, F_WRLCK, SEEK_END, 0, -3);
    ranges("measured", probe);
    lock(fd, F_SETLK, F_UNLCK, SEEK_SET, 0, 0);
    try("negative length past the start", fd, F_SETLK, F_WRLCK, SEEK_SET, 2, -3);
    try("negative start", fd, F_SETLK, F_WRLCK, SEEK_CUR, -8, 1);
    try("start past the largest", fd, F_SETLK, F_WRLCK, SEEK_END, 0x7fffffffffffffffLL, 1);
    try("length past the largest", fd, F_SETLK, F_WRLCK, SEEK_SET, 10, 0x7fffffffffffffffLL);
    try("largest single byte", fd, F_SETLK, F_WRLCK, SEEK_SET, 0x7fffffffffffffffLL, 1);
    try("getlk of the largest byte", probe, F_OFD_GETLK, F_RDLCK, SEEK_SET, 0x7ffffffffffffff0LL, 0);
    try("bad whence", fd, F_SETLK, F_WRLCK, 3, 0, 1);
    try("bad whence and kind", fd, F_GETLK, 7, 3, 0, 1);
    try("bad kind", fd, F_SETLK, 7, SEEK_SET, 0, 1);
    try("bad kind and range", fd, F_SETLK, 7, SEEK_SET, -1, 1);
    try("getlk of unlock", fd, F_GETLK, F_UNLCK, SEEK_SET, 0, 1);
    try("ofd getlk bad kind", fd, F_OFD_GETLK, 9, SEEK_SET, 0, 1);
    struct flock with_pid = {.l_type = F_RDLCK, .l_whence = SEEK_SET, .l_len = 1, .l_pid = 5};
    show("ofd setlk with a pid", fcntl(fd, F_OFD_SETLK, &with_pid), &with_pid);
    show("ofd getlk with a pid", fcntl(fd, F_OFD_GETLK, &with_pid), &with_pid);
    show("setlk with a pid", fcntl(fd, F_SETLK, &with_pid), &with_pid);
    lock(fd, F_SETLK, F_UNLCK, SEEK_SET, 0, 0);
    int reading = open("/tmp/locked", O_RDONLY), writing = open("/tmp/locked", O_WRONLY);
    try("write lock, read only", reading, F_SETLK, F_WRLCK, SEEK_SET, 0, 1);
    try("read lock, write only", writing, F_SETLK, F_RDLCK, SEEK_SET, 0, 1);
    try("unlock, write only", writing, F_SETLK, F_UNLCK, SEEK_SET, 0, 1);
    try("getlk write, read only", reading, F_GETLK, F_WRLCK, SEEK_SET, 0, 1);
    try("ofd write lock, read only", reading, F_OFD_SETLK, F_WRLCK, SEEK_SET, 0, 1);
    int path_fd = open("/tmp/locked", O_PATH);
    try("path descriptor", path_fd, F_SETLK, F_RDLCK, SEEK_SET, 0, 1);
    try("path descriptor getlk", path_fd, F_GETLK, F_RDLCK, SEEK_SET, 0, 1);
    errno = 0;
    printf("bad pointer %d %s\n", fcntl(fd, F_SETLK, (void *)8), strerror(errno));
    errno = 0;
    printf("bad descriptor %d %s\n", fcntl(99, F_GETLK, (void *)8), strerror(errno));
    struct flock *shown = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    *shown = (struct flock){.l_type = F_WRLCK, .l_whence = SEEK_SET, .l_len = 1};
    lock(probe, F_OFD_SETLK, F_RDLCK, SEEK_SET, 0, 1);
    mprotect(shown, 4096, PROT_READ);
    errno = 0;
    printf("getlk into read only memory %d %s\n", fcntl(fd, F_GETLK, shown), strerror(errno));
    lock(probe, F_OFD_SETLK, F_UNLCK, SEEK_SET, 0, 0);

    // Other files take locks too.
    try("lock /dev/null", 0, F_SETLK, F_RDLCK, SEEK_SET, 0, 1);
    try("write lock on the output pipe", 1, F_SETLK, F_WRLCK, SEEK_SET, 0, 0);
    int comm = open("/proc/self/comm", O_RDWR);
    try("lock a proc file", comm, F_SETLK, F_WRLCK, SEEK_SET, 0, 0);
    int directory = open("/tmp", O_RDONLY);
    try("read lock a directory", directory, F_SETLK, F_RDLCK, SEEK_SET, 0, 0);
    int memory = memfd_create("m", 0);
    try("lock a memory file", memory, F_OFD_SETLK, F_WRLCK, SEEK_SET, 0, 0);
    return 0;
}
