// An eventfd one below its overflow, counted once more by a completed asynchronous write: it
// reaches ULLONG_MAX, which poll shows as POLLERR and a read takes.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/aio_abi.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/syscall.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int evfd = eventfd(0, EFD_NONBLOCK);
    int fd = open("/tmp/eventfdoverflow", O_CREAT | O_RDWR | O_TRUNC, 0600);
    aio_context_t ctx = 0;
    printf("io_setup %ld\n", syscall(SYS_io_setup, 1, &ctx));
    uint64_t almost = UINT64_MAX - 1;
    printf("write %zd\n", write(evfd, &almost, 8));
    static char buffer[64];
    struct iocb cb = {.aio_lio_opcode = IOCB_CMD_PWRITE, .aio_fildes = fd, .aio_buf = (uintptr_t)buffer,
                      .aio_nbytes = sizeof buffer, .aio_flags = IOCB_FLAG_RESFD, .aio_resfd = evfd};
    struct iocb *list[1] = {&cb};
    printf("io_submit %ld\n", syscall(SYS_io_submit, ctx, 1, list));
    struct io_event event;
    printf("io_getevents %ld\n", syscall(SYS_io_getevents, ctx, 1, 1, &event, NULL));
    struct pollfd p = {.fd = evfd, .events = POLLIN | POLLOUT};
    printf("poll %d revents %x\n", poll(&p, 1, 0), p.revents);
    uint64_t one = 1;
    errno = 0;
    printf("write one more %zd %s\n", write(evfd, &one, 8), strerror(errno));
    uint64_t count = 0;
    printf("read %zd count is max %d\n", read(evfd, &count, 8), count == UINT64_MAX);
    p.revents = 0;
    printf("poll after %d revents %x\n", poll(&p, 1, 0), p.revents);
    return 0;
}
