// The futex calls made directly: every operation's checks and answers, waits ended by wakes, time-outs
// on both clocks and signals, requeues, wake-op, the priority inheriting locks between threads, the
// futex2 calls, and what a thread's end does to its thread id address.

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

#ifndef SYS_futex_waitv
#define SYS_futex_waitv 449
#endif
#ifndef SYS_futex_wake
#define SYS_futex_wake 454
#define SYS_futex_wait 455
#define SYS_futex_requeue 456
#endif

struct waitv {
    uint64_t val, uaddr;
    uint32_t flags, reserved;
};

static long fx(uint32_t *address, int op, uint32_t value, const void *timeout, uint32_t *address2, uint32_t value3) {
    return syscall(SYS_futex, address, op, value, timeout, address2, value3);
}

static void p(const char *what, long r) {
    printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0);
}

static uint32_t word, word2, word3;
static volatile int started;

static long gap_ms(struct timespec *a, struct timespec *b) {
    return (b->tv_sec - a->tv_sec) * 1000 + (b->tv_nsec - a->tv_nsec) / 1000000;
}

static void *waiter(void *arg) {
    __atomic_add_fetch(&started, 1, __ATOMIC_SEQ_CST);
    long r = fx(&word, FUTEX_WAIT_BITSET_PRIVATE, 0, NULL, NULL, (uint32_t)(uintptr_t)arg);
    return (void *)r;
}

static void wait_for_waiters(int count) {
    while (__atomic_load_n(&started, __ATOMIC_SEQ_CST) < count)
        usleep(1000);
    usleep(50000);
}

static uint32_t pi_lock;
static volatile int pi_state;

static void *pi_holder(void *arg) {
    p("lock pi free", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("owner is me %d\n", (pi_lock & FUTEX_TID_MASK) == (uint32_t)gettid());
    pi_state = 1;
    while (pi_state != 2)
        usleep(1000);
    usleep(50000);
    printf("waiters bit set %d\n", (pi_lock & FUTEX_WAITERS) != 0);
    long r = fx(&pi_lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0);
    (void)arg;
    return (void *)r;
}

static void handler(int signal) { (void)signal; }

static volatile int vector_waiting;

// Wakes as many waiters of `word` as it is given, once the vector wait has begun.
static void *vector_waker(void *arg) {
    while (!vector_waiting)
        usleep(1000);
    usleep(50000);
    return (void *)fx(&word, FUTEX_WAKE_PRIVATE, (uint32_t)(uintptr_t)arg, NULL, NULL, 0);
}

static uint32_t tid_word;

static int clear_me(void *arg) {
    (void)arg;
    syscall(SYS_exit, 0);
    return 0;
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    struct timespec zero = {0, 0}, bad = {0, 2000000000}, short_wait = {0, 20000000};
    struct timespec before, after;

    // Checks.
    p("wait unaligned", fx((uint32_t *)((char *)&word + 1), FUTEX_WAIT_PRIVATE, 0, NULL, NULL, 0));
    p("wait other value", fx(&word, FUTEX_WAIT_PRIVATE, 1, NULL, NULL, 0));
    p("wait bad time", fx(&word, FUTEX_WAIT_PRIVATE, 0, &bad, NULL, 0));
    p("wait no time left", fx(&word, FUTEX_WAIT_PRIVATE, 0, &zero, NULL, 0));
    p("wait bitset zero", fx(&word, FUTEX_WAIT_BITSET_PRIVATE, 0, NULL, NULL, 0));
    p("wait realtime flag", fx(&word, FUTEX_WAIT | FUTEX_CLOCK_REALTIME, 0, &zero, NULL, 0));
    p("wait null address", fx(NULL, FUTEX_WAIT_PRIVATE, 0, NULL, NULL, 0));
    p("wait shared unmapped", fx((uint32_t *)0x1000, FUTEX_WAIT, 0, NULL, NULL, 0));
    p("wake nobody", fx(&word, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0));
    p("wake shared unmapped", fx((uint32_t *)0x1000, FUTEX_WAKE, 1, NULL, NULL, 0));
    p("wake private unmapped", fx((uint32_t *)0x1000, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0));
    p("wake bitset zero", fx(&word, FUTEX_WAKE_BITSET_PRIVATE, 1, NULL, NULL, 0));
    p("fd", fx(&word, FUTEX_FD, 0, NULL, NULL, 0));
    p("unknown", fx(&word, 14, 0, NULL, NULL, 0));
    p("requeue negative", fx(&word, FUTEX_REQUEUE_PRIVATE, 1, (void *)-1L, &word2, 0));
    p("cmp requeue other", fx(&word, FUTEX_CMP_REQUEUE_PRIVATE, 1, (void *)1L, &word2, 5));
    p("cmp requeue nobody", fx(&word, FUTEX_CMP_REQUEUE_PRIVATE, 1, (void *)1L, &word2, 0));
    word3 = 5;
    p("wake op add", fx(&word, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1L, &word3, FUTEX_OP(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 5)));
    printf("word3 %u\n", word3);
    p("wake op shift", fx(&word, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1L, &word3, FUTEX_OP(FUTEX_OP_OR | 8, 4, FUTEX_OP_CMP_GT, 100)));
    printf("word3 %u\n", word3);
    p("wake op bad op", fx(&word, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1L, &word3, 7u << 28));
    p("wake op bad compare", fx(&word, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1L, &word3, 9u << 24));
    printf("word3 %u\n", word3);
    const uint32_t *read_only = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    p("wake op read only", fx(&word, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1L, (uint32_t *)read_only, 0));
    p("wait read only", fx((uint32_t *)read_only, FUTEX_WAIT_PRIVATE, 0, &zero, NULL, 0));

    // Time-outs on both clocks.
    clock_gettime(CLOCK_MONOTONIC, &before);
    p("wait relative", fx(&word, FUTEX_WAIT_PRIVATE, 0, &short_wait, NULL, 0));
    clock_gettime(CLOCK_MONOTONIC, &after);
    printf("waited at least 20 ms %d\n", gap_ms(&before, &after) >= 20);
    struct timespec until;
    clock_gettime(CLOCK_MONOTONIC, &until);
    until.tv_nsec += 20000000;
    if (until.tv_nsec >= 1000000000) {
        until.tv_sec++;
        until.tv_nsec -= 1000000000;
    }
    p("wait bitset monotonic", fx(&word, FUTEX_WAIT_BITSET_PRIVATE, 0, &until, NULL, FUTEX_BITSET_MATCH_ANY));
    clock_gettime(CLOCK_REALTIME, &until);
    until.tv_nsec += 20000000;
    if (until.tv_nsec >= 1000000000) {
        until.tv_sec++;
        until.tv_nsec -= 1000000000;
    }
    p("wait bitset realtime", fx(&word, FUTEX_WAIT_BITSET_PRIVATE | FUTEX_CLOCK_REALTIME, 0, &until, NULL, FUTEX_BITSET_MATCH_ANY));
    p("wait bitset past", fx(&word, FUTEX_WAIT_BITSET_PRIVATE, 0, &zero, NULL, FUTEX_BITSET_MATCH_ANY));

    // A signal with a handler ends a wait with EINTR.
    struct sigaction action = {.sa_handler = handler};
    sigaction(SIGALRM, &action, NULL);
    struct itimerval timer = {.it_value = {0, 20000}};
    (void)timer;
    pid_t self = getpid();
    pid_t killer = fork();
    if (killer == 0) {
        usleep(50000);
        kill(self, SIGALRM);
        _exit(0);
    }
    p("wait interrupted", fx(&word, FUTEX_WAIT_PRIVATE, 0, NULL, NULL, 0));
    waitpid(killer, NULL, 0);

    // Wakes by bits, and wakes that count.
    pthread_t threads[4];
    started = 0;
    pthread_create(&threads[0], NULL, waiter, (void *)(uintptr_t)1);
    pthread_create(&threads[1], NULL, waiter, (void *)(uintptr_t)2);
    pthread_create(&threads[2], NULL, waiter, (void *)(uintptr_t)3);
    wait_for_waiters(3);
    p("wake bits 4", fx(&word, FUTEX_WAKE_BITSET_PRIVATE, 10, NULL, NULL, 4));
    p("wake bits 2", fx(&word, FUTEX_WAKE_BITSET_PRIVATE, 10, NULL, NULL, 2));
    p("wake zero wakes one", fx(&word, FUTEX_WAKE_PRIVATE, 0, NULL, NULL, 0));
    p("wake the rest", fx(&word, FUTEX_WAKE_PRIVATE, 10, NULL, NULL, 0));
    for (int i = 0; i < 3; i++) {
        void *r;
        pthread_join(threads[i], &r);
        printf("waiter %d ended %ld\n", i, (long)r);
    }

    // Requeue: one woken, two moved, then woken from the second futex.
    started = 0;
    for (int i = 0; i < 3; i++)
        pthread_create(&threads[i], NULL, waiter, (void *)(uintptr_t)FUTEX_BITSET_MATCH_ANY);
    wait_for_waiters(3);
    p("cmp requeue", fx(&word, FUTEX_CMP_REQUEUE_PRIVATE, 1, (void *)2L, &word2, 0));
    usleep(20000);
    p("wake first after requeue", fx(&word, FUTEX_WAKE_PRIVATE, 10, NULL, NULL, 0));
    p("wake second after requeue", fx(&word2, FUTEX_WAKE_PRIVATE, 10, NULL, NULL, 0));
    for (int i = 0; i < 3; i++)
        pthread_join(threads[i], NULL);

    // Wake-op: changes the second word and wakes on both.
    started = 0;
    pthread_create(&threads[0], NULL, waiter, (void *)(uintptr_t)FUTEX_BITSET_MATCH_ANY);
    wait_for_waiters(1);
    word3 = 0;
    p("wake op both", fx(&word, FUTEX_WAKE_OP_PRIVATE, 1, (void *)1L, &word3, FUTEX_OP(FUTEX_OP_SET, 1, FUTEX_OP_CMP_EQ, 0)));
    printf("word3 %u\n", word3);
    pthread_join(threads[0], NULL);

    // Priority inheriting locks, alone and between two threads.
    pi_lock = 0;
    p("lock pi", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("pi word is my tid %d\n", pi_lock == (uint32_t)gettid());
    p("lock pi again", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("trylock pi held by me", fx(&pi_lock, FUTEX_TRYLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("unlock pi", fx(&pi_lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("pi word %u\n", pi_lock);
    p("unlock pi not mine", fx(&pi_lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    pi_lock = 0x3ffffff0;
    p("lock pi gone owner", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("pi word waiters %d\n", (pi_lock & FUTEX_WAITERS) != 0);
    pi_lock = FUTEX_OWNER_DIED;
    p("lock pi owner died", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("pi word keeps owner died %d mine %d\n", (pi_lock & FUTEX_OWNER_DIED) != 0, (pi_lock & FUTEX_TID_MASK) == (uint32_t)gettid());
    p("lock pi2 realtime past", fx(&word2, FUTEX_LOCK_PI2_PRIVATE | FUTEX_CLOCK_REALTIME, 0, &zero, NULL, 0));
    p("unlock pi2", fx(&word2, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    pi_lock = 0;
    pi_state = 0;
    pthread_create(&threads[0], NULL, pi_holder, NULL);
    while (pi_state != 1)
        usleep(1000);
    p("trylock pi held", fx(&pi_lock, FUTEX_TRYLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    clock_gettime(CLOCK_REALTIME, &until);
    until.tv_nsec += 10000000;
    if (until.tv_nsec >= 1000000000) {
        until.tv_sec++;
        until.tv_nsec -= 1000000000;
    }
    p("lock pi timed out", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, &until, NULL, 0));
    p("plain wait on pi", fx(&pi_lock, FUTEX_WAIT_PRIVATE, pi_lock, &zero, NULL, 0));
    pi_state = 2;
    p("lock pi waits", fx(&pi_lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("handed over: mine %d waiters bit %d\n", (pi_lock & FUTEX_TID_MASK) == (uint32_t)gettid(), (pi_lock & FUTEX_WAITERS) != 0);
    p("unlock handed", fx(&pi_lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("pi word %u\n", pi_lock);
    void *unlocked;
    pthread_join(threads[0], &unlocked);
    printf("the holder's unlock with a waiter = %ld\n", (long)unlocked);

    // The futex2 calls.
    struct waitv vector[3] = {
        {.val = 0, .uaddr = (uintptr_t)&word, .flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE},
        {.val = 0, .uaddr = (uintptr_t)&word2, .flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE},
        {.val = 7, .uaddr = (uintptr_t)&word3, .flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE},
    };
    word3 = 1;
    p("waitv flags", syscall(SYS_futex_waitv, vector, 2, 1, NULL, CLOCK_MONOTONIC));
    p("waitv none", syscall(SYS_futex_waitv, vector, 0, 0, NULL, CLOCK_MONOTONIC));
    p("waitv too many", syscall(SYS_futex_waitv, vector, 129, 0, NULL, CLOCK_MONOTONIC));
    p("waitv other value", syscall(SYS_futex_waitv, vector, 3, 0, NULL, CLOCK_MONOTONIC));
    p("waitv bad clock", syscall(SYS_futex_waitv, vector, 2, 0, &zero, CLOCK_BOOTTIME));
    p("waitv past", syscall(SYS_futex_waitv, vector, 2, 0, &zero, CLOCK_MONOTONIC));
    vector[1].flags = FUTEX2_SIZE_U64;
    p("waitv 64 bit", syscall(SYS_futex_waitv, vector, 2, 0, &zero, CLOCK_MONOTONIC));
    vector[1].flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE;
    vector[1].reserved = 1;
    p("waitv reserved", syscall(SYS_futex_waitv, vector, 2, 0, &zero, CLOCK_MONOTONIC));
    vector[1].reserved = 0;
    vector[1].val = 1ull << 32;
    p("waitv wide value", syscall(SYS_futex_waitv, vector, 2, 0, &zero, CLOCK_MONOTONIC));
    vector[1].val = 0;
    p("futex_wake strict zero", syscall(SYS_futex_wake, &word, 0xffffffffUL, 0, FUTEX2_SIZE_U32 | FUTEX2_PRIVATE));
    p("futex_wake wide mask", syscall(SYS_futex_wake, &word, 1UL << 32, 1, FUTEX2_SIZE_U32 | FUTEX2_PRIVATE));
    p("futex_wake 8 bit", syscall(SYS_futex_wake, &word, 1UL, 1, FUTEX2_SIZE_U8));
    p("futex_wait past", syscall(SYS_futex_wait, &word, 0UL, 0xffffffffUL, FUTEX2_SIZE_U32 | FUTEX2_PRIVATE, &zero, CLOCK_MONOTONIC));
    p("futex_wait other", syscall(SYS_futex_wait, &word, 3UL, 0xffffffffUL, FUTEX2_SIZE_U32 | FUTEX2_PRIVATE, NULL, CLOCK_MONOTONIC));
    p("futex_wait mask zero", syscall(SYS_futex_wait, &word, 0UL, 0UL, FUTEX2_SIZE_U32 | FUTEX2_PRIVATE, NULL, CLOCK_MONOTONIC));
    p("futex_requeue nobody", syscall(SYS_futex_requeue, vector, 0, 1, 1));
    p("futex_requeue flags", syscall(SYS_futex_requeue, vector, 1, 1, 1));
    started = 0;
    pthread_create(&threads[0], NULL, waiter, (void *)(uintptr_t)FUTEX_BITSET_MATCH_ANY);
    wait_for_waiters(1);
    p("futex_requeue moves", syscall(SYS_futex_requeue, vector, 0, 0, 1));
    p("futex_wake second", syscall(SYS_futex_wake, &word2, 0xffffffffUL, 1, FUTEX2_SIZE_U32 | FUTEX2_PRIVATE));
    pthread_join(threads[0], NULL);

    // A vector naming one futex twice: a wake takes its entries in order, and the call answers the
    // highest one taken.
    struct waitv twice[3] = {
        {.val = 0, .uaddr = (uintptr_t)&word, .flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE},
        {.val = 0, .uaddr = (uintptr_t)&word, .flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE},
        {.val = 0, .uaddr = (uintptr_t)&word2, .flags = FUTEX2_SIZE_U32 | FUTEX2_PRIVATE},
    };
    for (uintptr_t count = 1; count <= 2; count++) {
        vector_waiting = 0;
        pthread_create(&threads[0], NULL, vector_waker, (void *)count);
        clock_gettime(CLOCK_MONOTONIC, &until);
        until.tv_sec += 5;
        vector_waiting = 1;
        p("waitv one futex twice", syscall(SYS_futex_waitv, twice, 3, 0, &until, CLOCK_MONOTONIC));
        void *woken;
        pthread_join(threads[0], &woken);
        printf("its waker woke %ld\n", (long)woken);
    }

    // A thread's end clears and wakes its thread id address.
    char *stack = mmap(NULL, 65536, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    int flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM |
                CLONE_PARENT_SETTID | CLONE_CHILD_CLEARTID;
    long child = clone(clear_me, stack + 65536, flags, NULL, &tid_word, NULL, &tid_word);
    printf("clone a thread %d\n", child > 0);
    while (1) {
        uint32_t seen = __atomic_load_n(&tid_word, __ATOMIC_SEQ_CST);
        if (seen == 0)
            break;
        fx(&tid_word, FUTEX_WAIT, seen, NULL, NULL, 0);
    }
    printf("thread id address cleared %u\n", tid_word);
    return 0;
}
