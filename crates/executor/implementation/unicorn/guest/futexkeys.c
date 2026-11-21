// Futex keys: one file mapped twice, shared and private operations, mappings that cannot be read or
// written, futexes between processes through shared and private anonymous memory, and timeouts on
// both clocks.
#define _GNU_SOURCE
#include <errno.h>
#include <linux/futex.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static long fx(void *a, int op, uint32_t v, const void *t, void *a2, uint32_t v3) {
    return syscall(SYS_futex, a, op, v, t, a2, v3);
}
static void p(const char *what, long r) {
    printf("%s = %ld %s\n", what, r, r < 0 ? strerror(errno) : "");
    errno = 0;
}

static uint32_t *wait_at;
static int wait_op;
static volatile int waiting;
static void *waiter(void *arg) {
    (void)arg;
    waiting = 1;
    long r = fx(wait_at, wait_op, 0, NULL, NULL, 0);
    printf("waiter woke %ld\n", r);
    return NULL;
}

// Wakes at `wake_at` with `wake_op` while a thread waits at `at` with `op`; returns how many woke.
static long cross(uint32_t *at, int op, uint32_t *wake_at, int wake_op) {
    wait_at = at;
    wait_op = op;
    waiting = 0;
    pthread_t t;
    pthread_create(&t, NULL, waiter, NULL);
    while (!waiting) usleep(500);
    usleep(20000);
    long woken = fx(wake_at, wake_op, 1, NULL, NULL, 0);
    if (woken == 0) fx(at, op == FUTEX_WAIT ? FUTEX_WAKE : FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0);
    pthread_join(t, NULL);
    return woken;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int fd = memfd_create("f", 0);
    ftruncate(fd, 8192);
    uint32_t *a = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    uint32_t *b = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    p("shared key, one file at two addresses", cross(a + 1, FUTEX_WAIT, b + 1, FUTEX_WAKE));
    p("private key, one file at two addresses", cross(a + 1, FUTEX_WAIT_PRIVATE, b + 1, FUTEX_WAKE_PRIVATE));
    p("shared wait, private wake", cross(a + 1, FUTEX_WAIT, a + 1, FUTEX_WAKE_PRIVATE));
    uint32_t *c = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    p("private mapping of the file, shared ops at two addresses", cross(c + 1, FUTEX_WAIT, a + 1, FUTEX_WAKE));
    uint32_t *d = mmap(NULL, 8192, PROT_READ, MAP_PRIVATE, fd, 0);
    p("read-only private mapping of the file, shared ops", cross(d + 1, FUTEX_WAIT, a + 1, FUTEX_WAKE));
    uint32_t *rw = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    rw[0] = 0;
    mprotect(rw, 4096, PROT_READ);
    struct timespec two_ms = {0, 2000000};
    p("shared wait on a written page made read-only", fx(rw, FUTEX_WAIT, 0, &two_ms, NULL, 0));

    struct timespec short_wait = {0, 2000000};
    uint32_t *ro = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    p("wait on a read-only page", fx(ro, FUTEX_WAIT_PRIVATE, 0, &short_wait, NULL, 0));
    p("shared wait on a read-only page", fx(ro, FUTEX_WAIT, 0, &short_wait, NULL, 0));
    p("wake-op on a read-only page", fx(a, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1, ro, FUTEX_OP(FUTEX_OP_ADD, 1, FUTEX_OP_CMP_EQ, 0)));
    uint32_t *none = mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    p("private wake on PROT_NONE", fx(none, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0));
    p("shared wake on PROT_NONE", fx(none, FUTEX_WAKE, 1, NULL, NULL, 0));
    p("private wait on PROT_NONE", fx(none, FUTEX_WAIT_PRIVATE, 0, &short_wait, NULL, 0));
    munmap(none, 4096);
    p("shared wake unmapped", fx(none, FUTEX_WAKE, 1, NULL, NULL, 0));
    p("private wake unmapped", fx(none, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0));
    p("cmp requeue negative count", fx(a, FUTEX_CMP_REQUEUE_PRIVATE, 1, (void *)-1, b, 0));
    p("requeue to the same futex", fx(a, FUTEX_CMP_REQUEUE_PRIVATE, 1, (void *)1, a, 0));

    // Absolute waits that are already over, on each clock.
    struct timespec now;
    clock_gettime(CLOCK_REALTIME, &now);
    now.tv_sec -= 1;
    p("bitset realtime past", fx(a, FUTEX_WAIT_BITSET_PRIVATE | FUTEX_CLOCK_REALTIME, 0, &now, NULL, ~0u));
    clock_gettime(CLOCK_MONOTONIC, &now);
    now.tv_sec -= 1;
    p("bitset monotonic past", fx(a, FUTEX_WAIT_BITSET_PRIVATE, 0, &now, NULL, ~0u));
    p("wait with realtime flag", fx(a, FUTEX_WAIT_PRIVATE | FUTEX_CLOCK_REALTIME, 0, &short_wait, NULL, 0));
    struct timespec bad = {0, 1000000000};
    p("wait bad timeout", fx(a, FUTEX_WAIT_PRIVATE, 0, &bad, NULL, 0));
    p("wait bad timeout, value differs", fx(a, FUTEX_WAIT_PRIVATE, 5, &bad, NULL, 0));

    // Between processes.
    uint32_t *anon_shared = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    uint32_t *anon_private = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    pid_t child = fork();
    if (child == 0) {
        usleep(30000);
        long r1 = fx(anon_shared, FUTEX_WAKE, 1, NULL, NULL, 0);
        long r2 = fx(anon_private, FUTEX_WAKE, 1, NULL, NULL, 0);
        _exit(r1 * 10 + r2);
    }
    struct timespec long_wait = {2, 0};
    long r = fx(anon_shared, FUTEX_WAIT, 0, &long_wait, NULL, 0);
    p("wait on shared anonymous memory, woken by a child", r);
    int status;
    waitpid(child, &status, 0);
    printf("child woke shared %d private %d\n", WEXITSTATUS(status) / 10, WEXITSTATUS(status) % 10);

    // A waiter on a private anonymous page, woken through a child's wake at the same address: not.
    child = fork();
    if (child == 0) {
        _exit(fx(anon_private, FUTEX_WAKE, 1, NULL, NULL, 0));
    }
    struct timespec mid = {0, 50000000};
    p("wait on private anonymous memory", fx(anon_private, FUTEX_WAIT, 0, &mid, NULL, 0));
    waitpid(child, &status, 0);
    printf("child's wake %d\n", WEXITSTATUS(status));
    return 0;
}
