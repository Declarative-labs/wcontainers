// Scheduling between threads on one CPU: nice values and policies set on one thread by its number,
// the scheduling calls on threads, threads taking turns with sched_yield, and two threads that only
// compute sharing the CPU by their weights.

#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

static int gettid_(void) { return (int)syscall(SYS_gettid); }

static void show(const char *what, long result) {
    if (result < 0) {
        printf("%s: errno %d\n", what, errno);
    } else {
        printf("%s: %ld\n", what, result);
    }
    errno = 0;
}

// The nice value stat shows for a thread.
static long stat_nice(int tid) {
    char path[64], text[1024];
    snprintf(path, sizeof path, "/proc/self/task/%d/stat", tid);
    FILE *file = fopen(path, "r");
    if (!file) {
        return -100;
    }
    size_t got = fread(text, 1, sizeof text - 1, file);
    fclose(file);
    text[got] = 0;
    char *at = strrchr(text, ')');
    long fields[17];
    char state;
    sscanf(at + 2, "%c %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld", &state,
           &fields[0], &fields[1], &fields[2], &fields[3], &fields[4], &fields[5], &fields[6],
           &fields[7], &fields[8], &fields[9], &fields[10], &fields[11], &fields[12], &fields[13],
           &fields[14], &fields[15]);
    return fields[15];
}

static pthread_barrier_t barrier;
static volatile int sleeper_tid;

static void *sleeper(void *argument) {
    (void)argument;
    sleeper_tid = gettid_();
    pthread_barrier_wait(&barrier);
    pthread_barrier_wait(&barrier);
    return NULL;
}

// Turns taken with sched_yield: each thread notes itself and yields.
static char order[64];
static atomic_int next;
static atomic_int ready;

static void *yielder(void *argument) {
    char me = (char)(long)argument;
    atomic_fetch_add(&ready, 1);
    while (atomic_load(&ready) < 2) {
        sched_yield();
    }
    for (int round = 0; round < 10; round++) {
        int at = atomic_fetch_add(&next, 1);
        order[at] = me;
        sched_yield();
    }
    return NULL;
}

// Threads that only compute until told to stop; each reports its CPU time.
static atomic_int stop;
static atomic_int computing;

static void *compute(void *argument) {
    int nice_value = (int)(long)argument;
    setpriority(PRIO_PROCESS, gettid_(), nice_value);
    atomic_fetch_add(&computing, 1);
    volatile unsigned long count = 0;
    while (!atomic_load(&stop)) {
        count++;
    }
    struct timespec used;
    clock_gettime(CLOCK_THREAD_CPUTIME_ID, &used);
    return (void *)(long)(used.tv_sec * 1000000000L + used.tv_nsec);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int main_tid = gettid_();
    pthread_barrier_init(&barrier, NULL, 2);
    pthread_t thread;
    pthread_create(&thread, NULL, sleeper, NULL);
    pthread_barrier_wait(&barrier);
    int tid = sleeper_tid;

    // A nice value set by a thread's number is that thread's alone.
    show("setpriority thread", setpriority(PRIO_PROCESS, tid, 7));
    show("getpriority thread", syscall(SYS_getpriority, PRIO_PROCESS, tid));
    show("getpriority main", syscall(SYS_getpriority, PRIO_PROCESS, main_tid));
    show("getpriority process", syscall(SYS_getpriority, PRIO_PROCESS, 0));
    printf("stat nice thread %ld main %ld\n", stat_nice(tid), stat_nice(main_tid));
    show("lower again", setpriority(PRIO_PROCESS, tid, 3));
    show("getpriority group", syscall(SYS_getpriority, PRIO_PGRP, 0));
    show("getpriority user", syscall(SYS_getpriority, PRIO_USER, 0));

    // Policies by thread (musl answers the policy calls ENOSYS itself, so they are made raw).
    struct sched_param param = {0};
    show("batch", syscall(SYS_sched_setscheduler, tid, SCHED_BATCH, &param));
    show("thread policy", syscall(SYS_sched_getscheduler, tid));
    show("main policy", syscall(SYS_sched_getscheduler, main_tid));
    show("idle", syscall(SYS_sched_setscheduler, tid, SCHED_IDLE, &param));
    show("thread policy", syscall(SYS_sched_getscheduler, tid));
    param.sched_priority = 1;
    show("fifo", syscall(SYS_sched_setscheduler, tid, SCHED_FIFO, &param));
    show("rr", syscall(SYS_sched_setscheduler, tid, SCHED_RR, &param));
    param.sched_priority = 0;
    show("back to other", syscall(SYS_sched_setscheduler, tid, SCHED_OTHER, &param));
    show("thread policy", syscall(SYS_sched_getscheduler, tid));
    show("getparam", syscall(SYS_sched_getparam, tid, &param));
    struct timespec interval;
    show("rr interval", syscall(SYS_sched_rr_get_interval, tid, &interval));
    // The slice is the base slice scaled by the CPU count, which differs: the interval must be the
    // thread's slice in whole milliseconds.
    struct {
        uint32_t size, policy;
        uint64_t flags;
        int32_t nice;
        uint32_t priority;
        uint64_t runtime, deadline, period;
        uint32_t util_min, util_max;
    } attr;
    show("getattr", syscall(SYS_sched_getattr, tid, &attr, sizeof attr, 0));
    printf("interval is the slice in whole ms: %d\n",
           interval.tv_sec == 0 && interval.tv_nsec == (long)(attr.runtime / 1000000 * 1000000));
    cpu_set_t set;
    CPU_ZERO(&set);
    show("getaffinity", sched_getaffinity(tid, sizeof set, &set));
    printf("cpus %d\n", CPU_COUNT(&set));
    CPU_ZERO(&set);
    CPU_SET(0, &set);
    show("setaffinity", sched_setaffinity(tid, sizeof set, &set));
    show("not a thread", syscall(SYS_sched_getscheduler, main_tid + 1000));
    pthread_barrier_wait(&barrier);
    pthread_join(thread, NULL);

    // Two threads that yield to each other take turns.
    pthread_t a, b;
    pthread_create(&a, NULL, yielder, (void *)'a');
    pthread_create(&b, NULL, yielder, (void *)'b');
    pthread_join(a, NULL);
    pthread_join(b, NULL);
    int changes = 0;
    for (int at = 1; at < 20; at++) {
        changes += order[at] != order[at - 1];
    }
    printf("yield turns: at least 15 changes of 19 %d\n", changes >= 15);

    // The CPU shared by weight: nice 0 against nice 10 (1024 against 110) gives the first about
    // 90 per cent.
    pthread_t heavy, light;
    pthread_create(&heavy, NULL, compute, (void *)0L);
    pthread_create(&light, NULL, compute, (void *)10L);
    while (atomic_load(&computing) < 2) {
        sched_yield();
    }
    struct timespec pause = {0, 400 * 1000 * 1000};
    nanosleep(&pause, NULL);
    atomic_store(&stop, 1);
    void *heavy_time, *light_time;
    pthread_join(heavy, &heavy_time);
    pthread_join(light, &light_time);
    long share = (long)heavy_time * 100 / ((long)heavy_time + (long)light_time);
    printf("weighted share between 80 and 97 per cent: %d\n", share >= 80 && share <= 97);
    if (share < 80 || share > 97) {
        printf("share %ld\n", share);
    }
    return 0;
}
