// Threads through the C library: creating and joining, locks, conditions, barriers, read-write locks,
// semaphores, thread-local values, thread ids, a robust lock whose owner ends, a priority inheriting
// lock passed between threads, signals aimed at one thread with per-thread masks and alternate stacks,
// and the first thread leaving before the others. Lines are printed in an order the threads agree on.

#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <semaphore.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#define WORKERS 4
#define ROUNDS 2000

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static long counter;
static __thread int local_value;

static void *count_up(void *arg) {
    local_value = (int)(long)arg;
    for (int i = 0; i < ROUNDS; i++) {
        pthread_mutex_lock(&lock);
        counter++;
        pthread_mutex_unlock(&lock);
        if (i % 500 == 0)
            sched_yield();
    }
    return (void *)(long)(local_value * 10);
}

static pthread_mutex_t turn_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t turn_changed = PTHREAD_COND_INITIALIZER;
static int turn;
static char order[64];
static int order_length;

static void *take_turns(void *arg) {
    int me = (int)(long)arg;
    for (int round = 0; round < 3; round++) {
        pthread_mutex_lock(&turn_lock);
        while (turn % 3 != me)
            pthread_cond_wait(&turn_changed, &turn_lock);
        order[order_length++] = 'a' + me;
        turn++;
        pthread_cond_broadcast(&turn_changed);
        pthread_mutex_unlock(&turn_lock);
    }
    return NULL;
}

static pthread_barrier_t barrier;
static int arrived[WORKERS];
static int all_seen[WORKERS];

static void *meet(void *arg) {
    int me = (int)(long)arg;
    arrived[me] = 1;
    pthread_barrier_wait(&barrier);
    int seen = 0;
    for (int i = 0; i < WORKERS; i++)
        seen += arrived[i];
    all_seen[me] = seen;
    return NULL;
}

static pthread_rwlock_t table_lock = PTHREAD_RWLOCK_INITIALIZER;
static int table[16];

static void *write_table(void *arg) {
    for (int round = 0; round < 200; round++) {
        pthread_rwlock_wrlock(&table_lock);
        for (int i = 0; i < 16; i++)
            table[i]++;
        pthread_rwlock_unlock(&table_lock);
    }
    (void)arg;
    return NULL;
}

static void *read_table(void *arg) {
    long torn = 0;
    for (int round = 0; round < 200; round++) {
        pthread_rwlock_rdlock(&table_lock);
        for (int i = 1; i < 16; i++)
            torn += table[i] != table[0];
        pthread_rwlock_unlock(&table_lock);
    }
    (void)arg;
    return (void *)torn;
}

static sem_t items, room;
static int ring[4], produced_sum, consumed_sum;

static void *produce(void *arg) {
    for (int i = 1; i <= 50; i++) {
        sem_wait(&room);
        ring[i % 4] = i;
        produced_sum += i;
        sem_post(&items);
    }
    (void)arg;
    return NULL;
}

static void *consume(void *arg) {
    for (int i = 1; i <= 50; i++) {
        sem_wait(&items);
        consumed_sum += ring[i % 4];
        sem_post(&room);
    }
    (void)arg;
    return NULL;
}

static pid_t thread_ids[2];

static void *report_ids(void *arg) {
    int me = (int)(long)arg;
    thread_ids[me] = gettid();
    return NULL;
}

static pthread_mutex_t robust;

static void *die_holding(void *arg) {
    pthread_mutex_lock(&robust);
    (void)arg;
    return NULL;
}

static pthread_mutex_t inheriting;
static volatile int holder_ready;

static void *hold_then_release(void *arg) {
    pthread_mutex_lock(&inheriting);
    holder_ready = 1;
    usleep(100000);
    pthread_mutex_unlock(&inheriting);
    (void)arg;
    return NULL;
}

static volatile pid_t handled_by;
static volatile int on_alternate;
static char alternate[65536];

static void record(int signal) {
    char here;
    handled_by = gettid();
    on_alternate = &here >= alternate && &here < alternate + sizeof alternate;
    (void)signal;
}

static volatile int waiting_thread_ready;
static pid_t waiting_thread_id;

static void *wait_for_signal(void *arg) {
    stack_t stack = {.ss_sp = alternate, .ss_size = sizeof alternate};
    sigaltstack(&stack, NULL);
    sigset_t unblock;
    sigemptyset(&unblock);
    sigaddset(&unblock, SIGUSR1);
    sigaddset(&unblock, SIGUSR2);
    pthread_sigmask(SIG_UNBLOCK, &unblock, NULL);
    waiting_thread_id = gettid();
    waiting_thread_ready = 1;
    while (handled_by == 0)
        usleep(1000);
    (void)arg;
    return NULL;
}

static void *main_left(void *arg) {
    usleep(50000);
    printf("after the first thread left: getpid same %d\n", getpid() == (pid_t)(long)arg);
    fflush(stdout);
    return NULL;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    pthread_t threads[WORKERS];
    for (long i = 0; i < WORKERS; i++)
        pthread_create(&threads[i], NULL, count_up, (void *)(i + 1));
    long results = 0;
    for (int i = 0; i < WORKERS; i++) {
        void *result;
        pthread_join(threads[i], &result);
        results += (long)result;
    }
    printf("counter %ld results %ld\n", counter, results);

    for (long i = 0; i < 3; i++)
        pthread_create(&threads[i], NULL, take_turns, (void *)i);
    for (int i = 0; i < 3; i++)
        pthread_join(threads[i], NULL);
    printf("turns %s\n", order);

    pthread_barrier_init(&barrier, NULL, WORKERS);
    for (long i = 0; i < WORKERS; i++)
        pthread_create(&threads[i], NULL, meet, (void *)i);
    for (int i = 0; i < WORKERS; i++)
        pthread_join(threads[i], NULL);
    printf("barrier saw %d %d %d %d\n", all_seen[0], all_seen[1], all_seen[2], all_seen[3]);

    pthread_create(&threads[0], NULL, write_table, NULL);
    pthread_create(&threads[1], NULL, read_table, NULL);
    pthread_create(&threads[2], NULL, read_table, NULL);
    void *torn1, *torn2;
    pthread_join(threads[0], NULL);
    pthread_join(threads[1], &torn1);
    pthread_join(threads[2], &torn2);
    printf("rwlock table %d torn %ld\n", table[5], (long)torn1 + (long)torn2);

    sem_init(&items, 0, 0);
    sem_init(&room, 0, 4);
    pthread_create(&threads[0], NULL, produce, NULL);
    pthread_create(&threads[1], NULL, consume, NULL);
    pthread_join(threads[0], NULL);
    pthread_join(threads[1], NULL);
    printf("semaphores produced %d consumed %d\n", produced_sum, consumed_sum);

    pthread_create(&threads[0], NULL, report_ids, (void *)0);
    pthread_create(&threads[1], NULL, report_ids, (void *)1);
    pthread_join(threads[0], NULL);
    pthread_join(threads[1], NULL);
    printf("ids: main is pid %d, threads differ %d, not pid %d %d\n", gettid() == getpid(),
           thread_ids[0] != thread_ids[1], thread_ids[0] != getpid(), thread_ids[1] != getpid());

    pthread_mutexattr_t attributes;
    pthread_mutexattr_init(&attributes);
    pthread_mutexattr_setrobust(&attributes, PTHREAD_MUTEX_ROBUST);
    pthread_mutex_init(&robust, &attributes);
    pthread_create(&threads[0], NULL, die_holding, NULL);
    pthread_join(threads[0], NULL);
    int r = pthread_mutex_lock(&robust);
    printf("robust lock after its owner ended: %s\n", r == EOWNERDEAD ? "EOWNERDEAD" : strerror(r));
    pthread_mutex_consistent(&robust);
    pthread_mutex_unlock(&robust);

    // A robust lock in memory shared with a child that ends holding it.
    pthread_mutex_t *shared = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    pthread_mutexattr_setpshared(&attributes, PTHREAD_PROCESS_SHARED);
    pthread_mutex_init(shared, &attributes);
    pid_t child = fork();
    if (child == 0) {
        pthread_mutex_lock(shared);
        _exit(3);
    }
    int status;
    waitpid(child, &status, 0);
    r = pthread_mutex_lock(shared);
    printf("shared robust lock after the child ended: %s, child status %d\n",
           r == EOWNERDEAD ? "EOWNERDEAD" : strerror(r), WEXITSTATUS(status));

    pthread_mutexattr_t inherit;
    pthread_mutexattr_init(&inherit);
    pthread_mutexattr_setprotocol(&inherit, PTHREAD_PRIO_INHERIT);
    r = pthread_mutex_init(&inheriting, &inherit);
    pthread_create(&threads[0], NULL, hold_then_release, NULL);
    while (!holder_ready)
        usleep(1000);
    r = pthread_mutex_trylock(&inheriting);
    printf("inheriting trylock while held: %s\n", strerror(r));
    r = pthread_mutex_lock(&inheriting);
    printf("inheriting lock after the holder let go: %d\n", r);
    pthread_mutex_unlock(&inheriting);
    pthread_join(threads[0], NULL);

    // SIGUSR1 to the process goes to the one thread that does not block it, on its alternate stack.
    struct sigaction action = {.sa_handler = record, .sa_flags = SA_ONSTACK};
    sigaction(SIGUSR1, &action, NULL);
    sigaction(SIGUSR2, &action, NULL);
    sigset_t block;
    sigemptyset(&block);
    sigaddset(&block, SIGUSR1);
    sigaddset(&block, SIGUSR2);
    pthread_sigmask(SIG_BLOCK, &block, NULL);
    pthread_create(&threads[0], NULL, wait_for_signal, NULL);
    while (!waiting_thread_ready)
        usleep(1000);
    kill(getpid(), SIGUSR1);
    pthread_join(threads[0], NULL);
    printf("process signal handled by the unblocking thread %d on its alternate stack %d\n",
           handled_by == waiting_thread_id, on_alternate);
    handled_by = 0;
    waiting_thread_ready = 0;
    pthread_create(&threads[0], NULL, wait_for_signal, NULL);
    while (!waiting_thread_ready)
        usleep(1000);
    syscall(SYS_tgkill, getpid(), waiting_thread_id, SIGUSR2);
    pthread_join(threads[0], NULL);
    sigset_t pending;
    sigpending(&pending);
    printf("thread signal handled by its thread %d, main has nothing pending %d\n",
           handled_by == waiting_thread_id, !sigismember(&pending, SIGUSR1) && !sigismember(&pending, SIGUSR2));

    // The first thread leaves; the process goes on until its last thread ends.
    pthread_create(&threads[0], NULL, main_left, (void *)(long)getpid());
    (void)argc;
    (void)argv;
    pthread_exit(NULL);
}
