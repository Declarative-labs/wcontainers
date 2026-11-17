// The priority inheriting futex calls: their argument checks, waiters on a condition requeued onto
// a held lock and handed it in turn, a free lock taken straight from the requeue (as glibc's
// priority inheriting condition variables use them), and a lock whose owner ends while another
// waits for it.
#define _GNU_SOURCE
#include <errno.h>
#include <linux/futex.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

static uint32_t cond, lock;
static volatile int started, phase;

static long fx(uint32_t *a, int op, uint32_t v, const void *t, uint32_t *a2, uint32_t v3) {
    return syscall(SYS_futex, a, op, v, t, a2, v3);
}

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

static void *waiter(void *arg) {
    long id = (long)arg;
    __atomic_add_fetch(&started, 1, __ATOMIC_SEQ_CST);
    long r = fx(&cond, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 0, NULL, &lock, 0);
    int err = r < 0 ? errno : 0;
    int mine = (lock & FUTEX_TID_MASK) == (uint32_t)gettid();
    printf("waiter %ld woke r=%ld errno %d holds %d waiters %d\n", id, r, err, mine, (lock & FUTEX_WAITERS) != 0);
    if (mine) {
        p("waiter unlock", fx(&lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    }
    return NULL;
}

// Takes the lock, and ends holding it once the main thread waits for it (a bare exit, so it is
// never joined).
static void *holder(void *arg) {
    (void)arg;
    p("holder lock", fx(&lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    phase = 1;
    while (phase != 2)
        usleep(1000);
    usleep(50000);
    syscall(SYS_exit, 0);
    return NULL;
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    struct timespec zero = {0, 0};
    uint32_t *odd = (uint32_t *)((char *)&lock + 2);
    p("lock pi unaligned", fx(odd, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("lock pi2 unaligned", fx(odd, FUTEX_LOCK_PI2_PRIVATE, 0, NULL, NULL, 0));
    p("trylock pi unaligned", fx(odd, FUTEX_TRYLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("unlock pi unaligned", fx(odd, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("lock pi null", fx(NULL, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("unlock pi null", fx(NULL, FUTEX_UNLOCK_PI, 0, NULL, NULL, 0));
    p("unlock pi free", fx(&lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("wait rq unaligned and null", fx(odd, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 0, NULL, NULL, 0));
    p("wait rq null and unaligned", fx(NULL, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 0, NULL, odd, 0));
    p("wait rq other value same", fx(&cond, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 5, NULL, &cond, 0));
    p("cmp rq pi null same", fx(NULL, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)1L, NULL, 0));
    p("cmp rq pi negative", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)-1L, &lock, 0));
    p("cmp rq pi wake 0", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 0, (void *)1L, &lock, 0));
    p("cmp rq pi unaligned", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)1L, odd, 0));
    p("wait rq same", fx(&cond, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 0, NULL, &cond, 0));
    p("wait rq other value", fx(&cond, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 5, NULL, &lock, 0));
    p("wait rq past", fx(&cond, FUTEX_WAIT_REQUEUE_PI_PRIVATE, 0, &zero, &lock, 0));
    p("cmp rq pi wake 2", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 2, (void *)1L, &lock, 0));
    p("cmp rq pi nobody", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)1L, &lock, 0));
    p("cmp rq pi same", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)1L, &cond, 0));
    p("cmp rq pi other", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)1L, &lock, 9));

    pthread_t t[3];
    for (long i = 0; i < 3; i++) {
        pthread_create(&t[i], NULL, waiter, (void *)i);
        while (started != i + 1)
            usleep(1000);
        usleep(30000);
    }
    // Main holds the lock, so every waiter is requeued onto it.
    p("lock pi main", fx(&lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    p("plain wake on cond", fx(&cond, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0));
    p("plain requeue on cond", fx(&cond, FUTEX_CMP_REQUEUE_PRIVATE, 1, (void *)1L, &lock, 0));
    p("cmp rq pi all", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)10L, &lock, 0));
    printf("lock waiters bit %d mine %d\n", (lock & FUTEX_WAITERS) != 0, (lock & FUTEX_TID_MASK) == (uint32_t)gettid());
    p("plain wake on lock", fx(&lock, FUTEX_WAKE_PRIVATE, 1, NULL, NULL, 0));
    usleep(30000);
    p("unlock pi main", fx(&lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    for (int i = 0; i < 3; i++)
        pthread_join(t[i], NULL);
    printf("lock at end %u\n", lock);

    // Free lock: the first waiter takes it straight from the requeue.
    started = 0;
    lock = 0;
    pthread_create(&t[0], NULL, waiter, (void *)10L);
    while (started != 1)
        usleep(1000);
    usleep(30000);
    p("cmp rq pi free", fx(&cond, FUTEX_CMP_REQUEUE_PI_PRIVATE, 1, (void *)0L, &lock, 0));
    pthread_join(t[0], NULL);
    printf("lock at end %u\n", lock);

    // The owner ends holding the lock: it goes to the waiter.
    lock = 0;
    phase = 0;
    pthread_create(&t[0], NULL, holder, NULL);
    while (phase != 1)
        usleep(1000);
    phase = 2;
    p("lock pi from an ended owner", fx(&lock, FUTEX_LOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("mine %d waiters %d owner died %d\n", (lock & FUTEX_TID_MASK) == (uint32_t)gettid(),
           (lock & FUTEX_WAITERS) != 0, (lock & FUTEX_OWNER_DIED) != 0);
    p("unlock it", fx(&lock, FUTEX_UNLOCK_PI_PRIVATE, 0, NULL, NULL, 0));
    printf("lock at end %u\n", lock);
    return 0;
}
