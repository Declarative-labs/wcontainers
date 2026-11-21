// Threads of one process whose output does not depend on how they are scheduled: turns taken in
// order through a mutex and a condition variable, a contended counter, a thread that spins until
// another sets a flag, a timer signal sent to one chosen thread, and the process ended by one
// thread while another waits in a system call and the first waits to join it.
#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#define WORKERS 4
#define ROUNDS 50
#define ADDS 20000

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t changed = PTHREAD_COND_INITIALIZER;
static unsigned turn;
static char order[WORKERS * ROUNDS + 1];
static long total;

static void *worker(void *argument) {
    unsigned me = (unsigned)(uintptr_t)argument;
    for (int round = 0; round < ROUNDS; round++) {
        pthread_mutex_lock(&lock);
        while (turn % WORKERS != me)
            pthread_cond_wait(&changed, &lock);
        order[turn++] = (char)('0' + me);
        pthread_cond_broadcast(&changed);
        pthread_mutex_unlock(&lock);
    }
    for (int i = 0; i < ADDS; i++) {
        pthread_mutex_lock(&lock);
        total++;
        pthread_mutex_unlock(&lock);
    }
    return (void *)(uintptr_t)(me * 10 + 1);
}

static atomic_int go;

// Never makes a system call until another thread sets the flag.
static void *spinner(void *argument) {
    (void)argument;
    unsigned long spins = 0;
    while (!atomic_load(&go))
        spins++;
    return (void *)(uintptr_t)(spins > 0 ? 1 : 2);
}

static volatile pid_t hit;
static pid_t chosen_id;
static pthread_mutex_t ready_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ready = PTHREAD_COND_INITIALIZER;

static void on_timer(int number) {
    (void)number;
    hit = (pid_t)syscall(SYS_gettid);
}

// Waits with the timer's signal blocked but while suspended, so it can land only here or in
// threads that leave it unblocked (the others do).
static void *chosen(void *argument) {
    (void)argument;
    sigset_t blocked, none;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGUSR1);
    pthread_sigmask(SIG_BLOCK, &blocked, NULL);
    pthread_mutex_lock(&ready_lock);
    chosen_id = (pid_t)syscall(SYS_gettid);
    pthread_cond_signal(&ready);
    pthread_mutex_unlock(&ready_lock);
    sigemptyset(&none);
    while (!hit)
        sigsuspend(&none);
    return (void *)(uintptr_t)(hit == chosen_id);
}

static int pipe_ends[2];

static void *reader(void *argument) {
    (void)argument;
    char byte;
    ssize_t got = read(pipe_ends[0], &byte, 1);
    printf("reader returned %zd\n", got);
    return NULL;
}

static void *ender(void *argument) {
    (void)argument;
    struct timespec pause = {0, 20 * 1000 * 1000};
    nanosleep(&pause, NULL);
    printf("ending the process from another thread\n");
    exit(3);
}

int main(void) {
    pthread_t workers[WORKERS];
    for (unsigned i = 0; i < WORKERS; i++)
        pthread_create(&workers[i], NULL, worker, (void *)(uintptr_t)i);
    unsigned long sum = 0;
    for (unsigned i = 0; i < WORKERS; i++) {
        void *result;
        pthread_join(workers[i], &result);
        sum += (uintptr_t)result;
    }
    int in_order = 1;
    for (unsigned i = 0; i < WORKERS * ROUNDS; i++)
        in_order &= order[i] == (char)('0' + i % WORKERS);
    printf("turns %u in order %d; total %ld; joined %lu\n", turn, in_order, total, sum);

    pthread_t spin;
    pthread_create(&spin, NULL, spinner, NULL);
    struct timespec pause = {0, 5 * 1000 * 1000};
    nanosleep(&pause, NULL);
    atomic_store(&go, 1);
    void *spun;
    pthread_join(spin, &spun);
    printf("spinner ended after the flag: %d\n", (int)(uintptr_t)spun);

    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = on_timer;
    sigaction(SIGUSR1, &action, NULL);
    pthread_t target;
    pthread_mutex_lock(&ready_lock);
    pthread_create(&target, NULL, chosen, NULL);
    while (!chosen_id)
        pthread_cond_wait(&ready, &ready_lock);
    pthread_mutex_unlock(&ready_lock);
    struct sigevent event;
    memset(&event, 0, sizeof event);
    event.sigev_notify = SIGEV_THREAD_ID;
    event.sigev_signo = SIGUSR1;
    event.sigev_notify_thread_id = chosen_id;
    timer_t timer;
    if (timer_create(CLOCK_MONOTONIC, &event, &timer) != 0) {
        perror("timer_create");
        return 1;
    }
    struct itimerspec when = {.it_value = {0, 10 * 1000 * 1000}};
    timer_settime(timer, 0, &when, NULL);
    // Busy while the timer runs out, so the signal finds this thread running with it unblocked.
    while (!hit)
        atomic_fetch_add(&go, 1);
    void *landed;
    pthread_join(target, &landed);
    printf("timer signal on the chosen thread: %d\n", (int)(uintptr_t)landed);

    if (pipe(pipe_ends) != 0)
        return 1;
    pthread_t waiting, ending;
    pthread_create(&waiting, NULL, reader, NULL);
    pthread_create(&ending, NULL, ender, NULL);
    pthread_join(waiting, NULL);
    printf("never reached\n");
    return 0;
}
