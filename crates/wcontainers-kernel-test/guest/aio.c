// Native asynchronous I/O through the raw calls: contexts, reads, writes and syncs on files and
// pipes, their events in the ring and through io_getevents, and the checks on each.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/aio_abi.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/eventfd.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define R(call)                                                                                    \
    do {                                                                                           \
        long r_ = (call);                                                                          \
        printf("%-60s %ld %s\n", #call, r_, r_ < 0 ? strerror(errno) : "");                         \
    } while (0)

static long setup(unsigned n, aio_context_t *ctx) { return syscall(SYS_io_setup, n, ctx); }
static long destroy(aio_context_t ctx) { return syscall(SYS_io_destroy, ctx); }
static long submit(aio_context_t ctx, long n, struct iocb **iocbs) {
    return syscall(SYS_io_submit, ctx, n, iocbs);
}
static long getevents(aio_context_t ctx, long min, long n, struct io_event *events,
                      struct timespec *timeout) {
    return syscall(SYS_io_getevents, ctx, min, n, events, timeout);
}
static long cancel(aio_context_t ctx, struct iocb *iocb, struct io_event *result) {
    return syscall(SYS_io_cancel, ctx, iocb, result);
}

struct aio_ring {
    unsigned id, nr, head, tail, magic, compat_features, incompat_features, header_length;
};

static struct iocb make(int opcode, int fd, void *buffer, size_t length, long long offset) {
    struct iocb iocb;
    memset(&iocb, 0, sizeof iocb);
    iocb.aio_data = 0x1000 + opcode;
    iocb.aio_key = 77;
    iocb.aio_lio_opcode = opcode;
    iocb.aio_fildes = fd;
    iocb.aio_buf = (uintptr_t)buffer;
    iocb.aio_nbytes = length;
    iocb.aio_offset = offset;
    return iocb;
}

// Waits for `count` events (syncs finish later in Linux's work queue), then shows them in the
// order of their requests.
static void events(aio_context_t ctx, struct iocb *first, int count) {
    struct io_event got[8];
    long n = getevents(ctx, count, 8, got, NULL);
    printf("events %ld (wanted %d)\n", n, count);
    for (long i = 0; i < n; i++)
        for (long j = i + 1; j < n; j++)
            if (got[j].obj < got[i].obj) {
                struct io_event swap = got[i];
                got[i] = got[j];
                got[j] = swap;
            }
    for (long i = 0; i < n; i++)
        printf("  data %llx request %ld res %lld res2 %lld\n", (unsigned long long)got[i].data,
               (long)((struct iocb *)(uintptr_t)got[i].obj - first), (long long)got[i].res,
               (long long)got[i].res2);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    aio_context_t ctx = 0, other = 5;
    R(setup(0, &ctx));
    R(setup(4, &other));
    R(setup(4, (aio_context_t *)8));
    R(setup(4, &ctx));
    char line[32] = {0};
    FILE *nr = fopen("/proc/sys/fs/aio-nr", "r");
    fgets(line, sizeof line, nr);
    fclose(nr);
    // The count is the whole machine's, other containers' contexts too.
    printf("aio-nr counts this context %d\n", atol(line) >= 4);
    struct aio_ring *ring = (struct aio_ring *)ctx;
    printf("ring id %u head %u tail %u magic %x compat %u incompat %u header %u\n", ring->id,
           ring->head, ring->tail, ring->magic, ring->compat_features, ring->incompat_features,
           ring->header_length);

    int fd = open("/tmp/aiofile", O_RDWR | O_CREAT | O_TRUNC, 0644);
    char text[] = "asynchronous", back[32] = {0}, more[8] = {0};
    struct iocb requests[8];
    struct iocb *list[8];
    for (int i = 0; i < 8; i++)
        list[i] = &requests[i];
    requests[0] = make(IOCB_CMD_PWRITE, fd, text, 12, 4);
    requests[1] = make(IOCB_CMD_PREAD, fd, back, 32, 0);
    struct iovec parts[2] = {{back, 3}, {more, 5}};
    requests[2] = make(IOCB_CMD_PREADV, fd, parts, 2, 6);
    requests[3] = make(IOCB_CMD_FSYNC, fd, NULL, 0, 0);
    requests[4] = make(IOCB_CMD_FDSYNC, fd, NULL, 0, 0);
    R(submit(ctx, 5, list));
    printf("key %u back %.3s more %.5s\n", requests[0].aio_key, back, more);
    events(ctx, requests, 5);
    printf("head %u tail %u\n", ring->head, ring->tail);

    // The checks, each refusing its request (the first refused ends the call).
    int pipes[2];
    pipe(pipes);
    int reader = open("/tmp/aiofile", O_RDONLY);
    struct { int opcode, fd; long long offset; } bad[] = {
        {IOCB_CMD_PREAD, 99, 0},         {IOCB_CMD_PWRITE, 0, 0},      {IOCB_CMD_PWRITE, 1000, 0},
        {IOCB_CMD_FSYNC, 0, 0},          {IOCB_CMD_PREAD, 0, -1},      {42, 0, 0},
    };
    for (unsigned i = 0; i < sizeof bad / sizeof bad[0]; i++) {
        int target = bad[i].fd == 0 ? reader : bad[i].fd == 1000 ? pipes[0] : bad[i].fd;
        requests[0] = make(bad[i].opcode, target, back, bad[i].opcode == IOCB_CMD_FSYNC ? 0 : 4,
                           bad[i].offset);
        printf("case %u: ", i);
        R(submit(ctx, 1, list));
    }
    requests[0] = make(IOCB_CMD_FSYNC, pipes[1], NULL, 0, 0);
    R(submit(ctx, 1, list));
    requests[0] = make(IOCB_CMD_FSYNC, fd, back, 0, 0);
    R(submit(ctx, 1, list));
    requests[0] = make(IOCB_CMD_PREAD, fd, back, 4, 0);
    requests[0].aio_reserved2 = 1;
    R(submit(ctx, 1, list));
    requests[0] = make(IOCB_CMD_PREAD, fd, back, 4, 0);
    requests[0].aio_flags = IOCB_FLAG_RESFD;
    requests[0].aio_resfd = fd;
    R(submit(ctx, 1, list));
    R(submit(ctx, 0, list));
    R(submit(ctx, -1, list));
    R(submit(ctx + 4096, 1, list));
    R(submit(ctx, 1, (struct iocb **)8));
    struct iocb *broken[2] = {&requests[1], (struct iocb *)8};
    requests[1] = make(IOCB_CMD_PREAD, fd, back, 4, 0);
    R(submit(ctx, 2, broken));
    events(ctx, requests, 1);

    // A pipe ignores the offset; a bad buffer and an event descriptor's count.
    write(pipes[1], "piped", 5);
    int counter = eventfd(0, 0);
    requests[0] = make(IOCB_CMD_PREAD, pipes[0], back, 32, 12345);
    requests[0].aio_flags = IOCB_FLAG_RESFD;
    requests[0].aio_resfd = counter;
    requests[1] = make(IOCB_CMD_PREAD, fd, (void *)8, 4, 0);
    R(submit(ctx, 2, list));
    uint64_t count = 0;
    read(counter, &count, 8);
    printf("pipe read %.5s eventfd %llu\n", back, (unsigned long long)count);
    events(ctx, requests, 2);

    // A request's flags: one that may not wait on an empty pipe, on a file that cannot take it,
    // an unknown one, and appending.
    int empty[2];
    pipe(empty);
    requests[0] = make(IOCB_CMD_PREAD, empty[0], back, 4, 0);
    requests[0].aio_rw_flags = RWF_NOWAIT;
    R(submit(ctx, 1, list));
    events(ctx, requests, 1);
    requests[0] = make(IOCB_CMD_PREAD, fd, back, 4, 0);
    requests[0].aio_rw_flags = RWF_NOWAIT;
    R(submit(ctx, 1, list));
    requests[0].aio_rw_flags = 0x80000000;
    R(submit(ctx, 1, list));
    requests[0] = make(IOCB_CMD_PWRITE, reader, back, 4, 0);
    requests[0].aio_rw_flags = RWF_NOWAIT;
    R(submit(ctx, 1, list));
    requests[0] = make(IOCB_CMD_PWRITE, fd, "tail", 4, 0);
    requests[0].aio_rw_flags = RWF_APPEND;
    R(submit(ctx, 1, list));
    events(ctx, requests, 1);
    printf("size after append %ld\n", (long)lseek(fd, 0, SEEK_END));

    // io_getevents' checks and waits.
    struct io_event got[4];
    struct timespec short_wait = {0, 10000000};
    R(getevents(ctx, 2, 1, got, NULL));
    R(getevents(ctx, -1, 1, got, NULL));
    R(getevents(ctx + 4096, 0, 1, got, NULL));
    R(getevents(ctx, 1, 1, got, &short_wait));
    R(getevents(ctx, 1, 1, got, (struct timespec *)8));
    sigset_t mask;
    sigemptyset(&mask);
    struct { const sigset_t *mask; size_t size; } usig = {&mask, 8}, wrong = {&mask, 4};
    R(syscall(SYS_io_pgetevents, ctx, 1, 1, got, &short_wait, &usig));
    R(syscall(SYS_io_pgetevents, ctx, 1, 1, got, &short_wait, &wrong));
    R(syscall(SYS_io_pgetevents, ctx, 1, 1, got, &short_wait, (void *)8));
    requests[0] = make(IOCB_CMD_PREAD, fd, back, 4, 0);
    R(submit(ctx, 1, list));
    R(syscall(SYS_io_pgetevents, ctx, 1, 4, got, NULL, NULL));
    R(cancel(ctx, &requests[0], got));
    requests[0].aio_key = 3;
    R(cancel(ctx, &requests[0], got));
    R(cancel(ctx, (struct iocb *)8, got));

    // A child has no contexts.
    pid_t child = fork();
    if (child == 0) {
        requests[0] = make(IOCB_CMD_PREAD, fd, back, 4, 0);
        printf("child: ");
        R(submit(ctx, 1, list));
        _exit(0);
    }
    waitpid(child, NULL, 0);
    R(destroy(ctx));
    R(destroy(ctx));
    R(submit(ctx, 1, list));
    aio_context_t again = 0;
    R(setup(1, &again));
    R(destroy(again));
    unlink("/tmp/aiofile");
    return 0;
}
