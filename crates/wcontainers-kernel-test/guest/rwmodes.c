// Which files take reads and writes that may not wait (RWF_NOWAIT), and every fallocate mode up to
// 255 on a tmpfs file.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <unistd.h>

static void nowait(const char *what, int fd) {
    char buf[256] = "0123456789";
    struct iovec iov = {buf, 10};
    long r = syscall(SYS_preadv2, fd, &iov, 1, 0L, 0L, (long)RWF_NOWAIT);
    printf("%s: preadv2 %ld errno %d", what, r, r < 0 ? errno : 0);
    r = syscall(SYS_preadv2, fd, &iov, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" current %ld errno %d", r, r < 0 ? errno : 0);
    r = syscall(SYS_pwritev2, fd, &iov, 1, -1L, 0L, (long)RWF_NOWAIT);
    printf(" pwritev2 %ld errno %d\n", r, r < 0 ? errno : 0);
}

int main(void) {
    mkdir("/dev/shm/w", 0777);
    int fd = open("/dev/shm/w/f", O_RDWR | O_CREAT, 0644);
    nowait("empty shm", fd);
    write(fd, "hello world", 11);
    nowait("written shm", fd);
    nowait("shm read only", open("/dev/shm/w/f", O_RDONLY));
    fd = open("/tmp/f", O_RDWR | O_CREAT, 0644);
    nowait("root tmp", fd);
    nowait("dev null", open("/dev/null", O_RDWR));
    nowait("dev zero", open("/dev/zero", O_RDWR));
    nowait("dev urandom", open("/dev/urandom", O_RDONLY));
    int p[2];
    pipe(p);
    write(p[1], "x", 1);
    nowait("pipe reader", p[0]);
    nowait("pipe writer", p[1]);
    nowait("memfd", memfd_create("m", 0));
    nowait("proc comm", open("/proc/self/comm", O_RDWR));
    fd = open("/dev/shm/w/g", O_RDWR | O_CREAT, 0644);
    for (int mode = 0; mode < 256; mode++) {
        long r = fallocate(fd, mode, 0, 4096);
        printf("fallocate %d: %ld errno %d\n", mode, r, r < 0 ? errno : 0);
    }
    return 0;
}
