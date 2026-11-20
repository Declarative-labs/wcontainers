// Writes of many backend messages each: one write, a pwrite, a writev of three parts and three
// asynchronous writes submitted together, then the file read back. Prints each result and a sum of
// the file's bytes by position.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/aio_abi.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <unistd.h>

enum { LARGE = (1 << 20) + 12345, PART = 300 << 10, PARTS = 3 };

static void result(const char *what, long n) {
    if (n < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
    } else {
        printf("%s: %ld\n", what, n);
    }
}

static unsigned char *pattern(size_t length, unsigned seed) {
    unsigned char *bytes = malloc(length);
    for (size_t i = 0; i < length; i++) bytes[i] = (unsigned char)(i * 7 + seed);
    return bytes;
}

int main(void) {
    int fd = open("large", O_CREAT | O_RDWR | O_TRUNC, 0644);
    result("open", fd);
    unsigned char *first = pattern(LARGE, 1);
    result("write", write(fd, first, LARGE));
    result("pwrite", pwrite(fd, pattern(LARGE, 2), LARGE, 3 * LARGE));
    struct iovec parts[PARTS];
    for (int i = 0; i < PARTS; i++) {
        parts[i].iov_base = pattern(PART + i, 3 + i);
        parts[i].iov_len = PART + i;
    }
    result("writev", writev(fd, parts, PARTS));

    aio_context_t ctx = 0;
    result("io_setup", syscall(SYS_io_setup, PARTS, &ctx));
    struct iocb iocbs[PARTS];
    struct iocb *list[PARTS];
    for (int i = 0; i < PARTS; i++) {
        memset(&iocbs[i], 0, sizeof iocbs[i]);
        iocbs[i].aio_lio_opcode = IOCB_CMD_PWRITE;
        iocbs[i].aio_fildes = fd;
        iocbs[i].aio_buf = (uintptr_t)pattern(PART, 9 + i);
        iocbs[i].aio_nbytes = PART;
        iocbs[i].aio_offset = 5LL * LARGE + (long long)i * PART;
        list[i] = &iocbs[i];
    }
    result("io_submit", syscall(SYS_io_submit, ctx, PARTS, list));
    struct io_event got[PARTS];
    long n = syscall(SYS_io_getevents, ctx, PARTS, PARTS, got, NULL);
    result("io_getevents", n);
    for (long i = 0; i < n; i++) {
        printf("event %ld: %lld\n", (long)((struct iocb *)(uintptr_t)got[i].obj - iocbs),
               (long long)got[i].res);
    }

    off_t size = lseek(fd, 0, SEEK_END);
    result("size", size);
    unsigned char *back = malloc(size);
    result("pread", pread(fd, back, size, 0));
    unsigned long long sum = 0;
    for (off_t i = 0; i < size; i++) sum += (unsigned long long)back[i] * (unsigned long long)(i % 251 + 1);
    printf("sum %llu, first part same %d\n", sum, memcmp(back, first, LARGE) == 0);
    return 0;
}
