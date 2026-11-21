// Native asynchronous poll requests (IOCB_CMD_POLL): ready at once or waiting for their file's
// events, their checks, cancels, event descriptors and a wait for another process's write.
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
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define R(call)                                                                                    \
    do {                                                                                           \
        long r_ = (call);                                                                          \
        printf("%-60s %ld %s\n", #call, r_, r_ < 0 ? strerror(errno) : "");                         \
    } while (0)

static aio_context_t ctx;
static struct iocb requests[8];
static struct iocb *list[8];

static long submit(int first, long n) { return syscall(SYS_io_submit, ctx, n, &list[first]); }
static long cancel(int which, struct io_event *result) {
    return syscall(SYS_io_cancel, ctx, &requests[which], result);
}

static void poll_request(int which, int fd, unsigned long long events) {
    memset(&requests[which], 0, sizeof requests[which]);
    requests[which].aio_data = 0x100 + which;
    requests[which].aio_lio_opcode = IOCB_CMD_POLL;
    requests[which].aio_fildes = fd;
    requests[which].aio_buf = events;
}

// Takes the events that came within `wait` milliseconds (-1: waits for at least one).
static void events(long wait) {
    struct io_event got[8];
    struct timespec timeout = {0, wait * 1000000};
    long n = syscall(SYS_io_getevents, ctx, wait < 0 ? 1 : 0, 8, got, wait < 0 ? NULL : &timeout);
    printf("events %ld\n", n);
    for (long i = 0; i < n; i++)
        printf("  data %llx request %ld res %llx\n", (unsigned long long)got[i].data,
               (long)((struct iocb *)(uintptr_t)got[i].obj - requests), (long long)got[i].res);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    for (int i = 0; i < 8; i++)
        list[i] = &requests[i];
    R(syscall(SYS_io_setup, 8, &ctx));
    int pipes[2], other[2], hung[2];
    pipe(pipes);
    pipe(other);
    pipe(hung);

    // Waiting for a read end, at once for a write end and a file.
    int file = open("/tmp/aiopoll", O_RDWR | O_CREAT | O_TRUNC, 0644);
    poll_request(0, pipes[0], POLLIN);
    poll_request(1, pipes[1], POLLOUT | POLLWRNORM);
    poll_request(2, file, POLLIN | POLLOUT);
    R(submit(0, 3));
    events(0);
    write(pipes[1], "x", 1);
    events(0);

    // The checks.
    poll_request(3, pipes[0], 0x10000);
    R(submit(3, 1));
    poll_request(3, pipes[0], POLLIN);
    requests[3].aio_offset = 1;
    R(submit(3, 1));
    poll_request(3, pipes[0], POLLIN);
    requests[3].aio_nbytes = 1;
    R(submit(3, 1));
    poll_request(3, pipes[0], POLLIN);
    requests[3].aio_rw_flags = 1;
    R(submit(3, 1));
    poll_request(3, 99, POLLIN);
    R(submit(3, 1));

    // A cancel posts the event with no events (Linux's later, from its work queue).
    struct io_event result;
    poll_request(4, other[0], POLLIN);
    R(submit(4, 1));
    R(cancel(4, &result));
    R(cancel(4, &result));
    events(-1);

    // A hang-up comes to every poll; one waits while another process writes, and counts an event
    // descriptor.
    poll_request(5, hung[0], POLLIN);
    R(submit(5, 1));
    close(hung[1]);
    events(-1);
    int counter = eventfd(0, 0);
    poll_request(6, other[0], POLLIN | POLLRDNORM);
    requests[6].aio_flags = IOCB_FLAG_RESFD;
    requests[6].aio_resfd = counter;
    R(submit(6, 1));
    pid_t child = fork();
    if (child == 0) {
        usleep(50000);
        write(other[1], "y", 1);
        _exit(0);
    }
    events(-1);
    uint64_t count = 0;
    read(counter, &count, 8);
    printf("eventfd %llu\n", (unsigned long long)count);
    waitpid(child, NULL, 0);

    // An event descriptor polled, and a context destroyed with a poll waiting.
    poll_request(7, counter, POLLIN);
    R(submit(7, 1));
    events(0);
    uint64_t one = 1;
    write(counter, &one, 8);
    events(0);
    poll_request(0, other[0], POLLPRI);
    R(submit(0, 1));
    R(syscall(SYS_io_destroy, ctx));
    unlink("/tmp/aiopoll");
    return 0;
}
