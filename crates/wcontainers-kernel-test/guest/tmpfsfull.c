// tmpfs's limits: Docker's 64 MiB /dev/shm filled by writes, fallocate and links, and what statfs
// reports as pages and inodes are taken and given back. Inode totals depend on the machine's memory,
// so only the inodes in use are printed.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/xattr.h>
#include <unistd.h>

#define R(expr)                                                                                    \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long r_ = (long)(expr);                                                                    \
        printf("%-64s = %ld (%s)\n", #expr, r_, r_ < 0 ? strerror(errno) : "-");                 \
    } while (0)

static void space(const char *what, const char *path) {
    struct statfs st;
    if (statfs(path, &st) != 0) {
        printf("%s: statfs %s\n", what, strerror(errno));
        return;
    }
    printf("%-28s %s: type %lx blocks %ld bfree %ld bavail %ld inodes used %ld%s\n", what, path,
           (long)st.f_type, (long)st.f_blocks, (long)st.f_bfree, (long)st.f_bavail,
           (long)(st.f_files - st.f_ffree), st.f_files == 0 ? " (no inode limit)" : "");
}

static void fspace(const char *what, int fd) {
    struct statfs st;
    if (fstatfs(fd, &st) != 0) {
        printf("%s: fstatfs %s\n", what, strerror(errno));
        return;
    }
    printf("%-28s fd: type %lx blocks %ld bfree %ld bavail %ld files %ld ffree %ld\n", what,
           (long)st.f_type, (long)st.f_blocks, (long)st.f_bfree, (long)st.f_bavail,
           (long)st.f_files, (long)st.f_ffree);
}

static void blocks(const char *path) {
    struct stat st;
    if (lstat(path, &st) == 0) {
        printf("%s: size %lld blocks %lld\n", path, (long long)st.st_size, (long long)st.st_blocks);
    } else {
        printf("%s: %s\n", path, strerror(errno));
    }
}

int main(void) {
    static char chunk[1 << 20];
    memset(chunk, 'x', sizeof(chunk));
    space("start", "/dev/shm");
    space("start", "/dev");
    space("start", "/proc");
    space("start", "/dev/pts");
    fspace("stdout", 1);
    int memfd = syscall(SYS_memfd_create, "m", 0);
    write(memfd, "abc", 3);
    fspace("memfd", memfd);

    R(mkdir("/dev/shm/t", 0755));
    space("after mkdir", "/dev/shm");
    int fd = open("/dev/shm/t/big", O_CREAT | O_RDWR, 0644);
    long total = 0;
    for (int i = 0; i < 80; i++) {
        errno = 0;
        long n = write(fd, chunk, sizeof(chunk));
        if (n != (long)sizeof(chunk)) {
            printf("write %d gave %ld (%s) after %ld\n", i, n, n < 0 ? strerror(errno) : "-", total);
            if (n <= 0) {
                break;
            }
        }
        total += n;
    }
    printf("total %ld\n", total);
    space("full", "/dev/shm");
    blocks("/dev/shm/t/big");
    R(write(fd, "y", 1));
    R(pwrite(fd, "y", 1, 0));
    R(pwrite(fd, "y", 1, 100L << 20));
    R(mkdir("/dev/shm/t/d", 0755));
    R(symlink("short", "/dev/shm/t/short"));
    char target[300];
    memset(target, 'l', sizeof(target) - 1);
    target[sizeof(target) - 1] = 0;
    R(symlink(target, "/dev/shm/t/long"));
    R(setxattr("/dev/shm/t/big", "user.a", "value", 5, 0));
    R(fallocate(fd, 0, 0, 200L << 20));
    R(fallocate(fd, 0, 64L << 20, 4096));
    R(ftruncate(fd, 60L << 20));
    space("truncated to 60M", "/dev/shm");
    // A request larger than what is left fails and keeps nothing it took.
    R(fallocate(fd, 0, 60L << 20, 8L << 20));
    space("after failed fallocate", "/dev/shm");
    blocks("/dev/shm/t/big");
    R(fallocate(fd, FALLOC_FL_KEEP_SIZE, 60L << 20, 2L << 20));
    space("after fallocate 2M", "/dev/shm");
    blocks("/dev/shm/t/big");
    // A write past what is left is cut short at the last page that fits.
    R(lseek(fd, 0, SEEK_END));
    errno = 0;
    long n = write(fd, chunk, sizeof(chunk));
    printf("write 1M near full = %ld (%s)\n", n, n < 0 ? strerror(errno) : "-");
    n = write(fd, chunk, sizeof(chunk));
    printf("write 1M near full = %ld (%s)\n", n, n < 0 ? strerror(errno) : "-");
    n = write(fd, chunk, sizeof(chunk));
    printf("write 1M near full = %ld (%s)\n", n, n < 0 ? strerror(errno) : "-");
    space("near full", "/dev/shm");
    R(fallocate(fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE, 0, 10L << 20));
    space("after punching 10M", "/dev/shm");
    blocks("/dev/shm/t/big");
    // Long link targets take a page; links and extended attributes take inode space.
    R(symlink(target, "/dev/shm/t/long"));
    space("after long link", "/dev/shm");
    blocks("/dev/shm/t/long");
    blocks("/dev/shm/t/short");
    R(symlink("short", "/dev/shm/t/short"));
    space("after short link", "/dev/shm");
    R(link("/dev/shm/t/big", "/dev/shm/t/big2"));
    R(link("/dev/shm/t/big", "/dev/shm/t/big3"));
    space("after two hard links", "/dev/shm");
    R(unlink("/dev/shm/t/big3"));
    space("after unlinking one", "/dev/shm");
    char value[3000];
    memset(value, 'v', sizeof(value));
    R(setxattr("/dev/shm/t/short", "user.big", value, sizeof(value), XATTR_CREATE));
    space("after a 3000 byte xattr", "/dev/shm");
    R(setxattr("/dev/shm/t/short", "user.big", value, 10, XATTR_REPLACE));
    space("after shrinking it", "/dev/shm");
    R(setxattr("/dev/shm/t/short", "user.big", value, 10, XATTR_CREATE));
    space("after a refused create", "/dev/shm");
    R(removexattr("/dev/shm/t/short", "user.big"));
    space("after removing it", "/dev/shm");
    // Files gone give their pages and inodes back once nothing holds them.
    R(unlink("/dev/shm/t/big"));
    R(unlink("/dev/shm/t/big2"));
    space("unlinked, still open", "/dev/shm");
    R(close(fd));
    space("closed", "/dev/shm");
    R(unlink("/dev/shm/t/long"));
    R(unlink("/dev/shm/t/short"));
    R(rmdir("/dev/shm/t/d"));
    space("emptied", "/dev/shm");
    // /dev has its own 64 MiB.
    int devfile = open("/dev/big", O_CREAT | O_RDWR, 0644);
    R(fallocate(devfile, 0, 0, 65L << 20));
    R(fallocate(devfile, 0, 0, 10L << 20));
    space("dev after 10M", "/dev");
    space("shm unchanged", "/dev/shm");
    R(unlink("/dev/big"));
    R(close(devfile));
    space("dev emptied", "/dev");
    return 0;
}
