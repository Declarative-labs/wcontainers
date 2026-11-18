// The extended scheduling calls (sched_setattr, sched_getattr), sched_rr_get_interval, and mincore's
// values. The default time slice depends on the machine's CPU count (one here), so slices are shown
// relative to it.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

struct attr {
    uint32_t size, policy;
    uint64_t flags;
    int32_t nice;
    uint32_t priority;
    uint64_t runtime, deadline, period;
    uint32_t util_min, util_max;
    unsigned char tail[64];
};

static uint64_t base_slice;

static void show(const char *what, long result) {
    printf("%s %ld %d\n", what, result, result < 0 ? errno : 0);
}

static void get(const char *what, unsigned size) {
    struct attr attr;
    memset(&attr, 0xee, sizeof attr);
    long result = syscall(SYS_sched_getattr, 0, &attr, size, 0);
    if (!base_slice && result == 0) base_slice = attr.runtime;
    char runtime[32];
    if (attr.runtime == base_slice)
        snprintf(runtime, sizeof runtime, "base");
    else
        snprintf(runtime, sizeof runtime, "%llu", (unsigned long long)attr.runtime);
    printf("%s %ld %d: size %u policy %u flags %llx nice %d priority %u runtime %s deadline %llu period %llu "
           "util %u %u tail %02x\n",
           what, result, result < 0 ? errno : 0, attr.size, attr.policy, (unsigned long long)attr.flags,
           attr.nice, attr.priority, runtime, (unsigned long long)attr.deadline,
           (unsigned long long)attr.period, attr.util_min, attr.util_max, attr.tail[0]);
}

static long set(uint32_t size, uint32_t policy, uint64_t flags, int nice, uint32_t priority, uint64_t runtime,
                uint64_t deadline, uint64_t period, uint32_t util_min, uint32_t util_max) {
    struct attr attr = {size, policy, flags, nice, priority, runtime, deadline, period, util_min, util_max, {0}};
    return syscall(SYS_sched_setattr, 0, &attr, 0);
}

// The scheduling fields of /proc/self/stat: priority, nice, threads, exit signal, real-time priority
// and policy (the CPU number, field 39, differs between machines).
static void stat_fields(const char *what) {
    char line[1024] = {0};
    int fd = open("/proc/self/stat", O_RDONLY);
    read(fd, line, sizeof line - 1);
    close(fd);
    char *at = strrchr(line, ')') + 2;
    int field = 3;
    printf("%s stat:", what);
    for (char *word = strtok(at, " \n"); word; word = strtok(NULL, " \n"), field++) {
        if ((field >= 18 && field <= 20) || field == 38 || field == 40 || field == 41) {
            printf(" %s", word);
        }
    }
    printf(" (%d fields)\n", field - 1);
}

int main(void) {
    get("get", 56);
    get("get small", 48);
    get("get large", 120);
    get("get too small", 47);
    get("get page", 4097);
    show("get bad pid", syscall(SYS_sched_getattr, -1, NULL, 56, 0));
    struct attr attr;
    show("get flags", syscall(SYS_sched_getattr, 0, &attr, 56, 1));
    show("get missing", syscall(SYS_sched_getattr, 99999, &attr, 56, 0));
    show("get null", syscall(SYS_sched_getattr, 0, NULL, 56, 0));
    show("get fault", syscall(SYS_sched_getattr, 0, (void *)8, 56, 0));

    show("set null", syscall(SYS_sched_setattr, 0, NULL, 0));
    show("set flags", syscall(SYS_sched_setattr, 0, &attr, 1));
    show("set fault", syscall(SYS_sched_setattr, 0, (void *)8, 0));
    memset(&attr, 0, sizeof attr);
    attr.size = 47;
    show("set size 47", syscall(SYS_sched_setattr, 0, &attr, 0));
    printf("size now %u\n", attr.size);
    attr.size = 80;
    attr.tail[0] = 1;
    show("set size 80 tail", syscall(SYS_sched_setattr, 0, &attr, 0));
    printf("size now %u\n", attr.size);
    attr.size = 0;
    show("set size 0", syscall(SYS_sched_setattr, 0, &attr, 0));
    show("set other", set(56, SCHED_OTHER, 0, 0, 0, 0, 0, 0, 0, 0));
    show("set nice 5", set(56, SCHED_OTHER, 0, 5, 0, 0, 0, 0, 0, 0));
    get("after nice 5", 56);
    show("getpriority", getpriority(PRIO_PROCESS, 0));
    show("set nice 2", set(56, SCHED_OTHER, 0, 2, 0, 0, 0, 0, 0, 0));
    show("set nice 30", set(56, SCHED_OTHER, 0, 30, 0, 0, 0, 0, 0, 0));
    get("after nice 30", 56);
    stat_fields("start");
    show("set batch", set(56, SCHED_BATCH, 0, 19, 0, 0, 0, 0, 0, 0));
    stat_fields("batch");
    get("after batch", 56);
    show("set fifo", set(56, SCHED_FIFO, 0, 0, 1, 0, 0, 0, 0, 0));
    show("set fifo 0", set(56, SCHED_FIFO, 0, 0, 0, 0, 0, 0, 0, 0));
    show("set other priority", set(56, SCHED_OTHER, 0, 19, 1, 0, 0, 0, 0, 0));
    show("set deadline", set(56, SCHED_DEADLINE, 0, 0, 0, 1000000, 2000000, 3000000, 0, 0));
    show("set deadline bad", set(56, SCHED_DEADLINE, 0, 0, 0, 3000000, 2000000, 1000000, 0, 0));
    show("set deadline zero", set(56, SCHED_DEADLINE, 0, 0, 0, 0, 0, 0, 0, 0));
    show("set unknown policy", set(56, 9, 0, 0, 0, 0, 0, 0, 0, 0));
    show("set ext", set(56, 7, 0, 0, 0, 0, 0, 0, 0, 0));
    show("set negative policy", set(56, -1, 0, 0, 0, 0, 0, 0, 0, 0));
    show("set unknown flag", set(56, SCHED_BATCH, 0x80, 19, 0, 0, 0, 0, 0, 0));
    show("set clamp small size", set(48, SCHED_BATCH, 0x20, 19, 0, 0, 0, 0, 100, 0));
    show("set clamp", set(56, SCHED_BATCH, 0x60, 19, 0, 0, 0, 0, 100, 200));
    show("set slice", set(56, SCHED_BATCH, 0, 19, 0, 5000000, 0, 0, 0, 0));
    get("after slice", 56);
    show("set tiny slice", set(56, SCHED_BATCH, 0, 19, 0, 5, 0, 0, 0, 0));
    get("after tiny slice", 56);
    show("set huge slice", set(56, SCHED_BATCH, 0, 19, 0, 1000000000, 0, 0, 0, 0));
    get("after huge slice", 56);
    show("setscheduler keeps slice", sched_setscheduler(0, SCHED_BATCH, &(struct sched_param){0}));
    get("after setscheduler", 56);
    struct timespec custom;
    show("rr interval custom", syscall(SYS_sched_rr_get_interval, 0, &custom));
    printf("interval %ld %ld\n", (long)custom.tv_sec, custom.tv_nsec);
    show("set default slice", set(56, SCHED_BATCH, 0, 19, 0, 0, 0, 0, 0, 0));
    show("set sugov", set(56, SCHED_BATCH, 0x10000000, 19, 0, 0, 0, 0, 0, 0));
    show("set sugov deadline", set(56, SCHED_DEADLINE, 0x10000000, 0, 0, 0, 0, 0, 0, 0));
    show("set idle", set(56, SCHED_IDLE, 0, 19, 0, 0, 0, 0, 0, 0));
    stat_fields("idle");
    get("after idle", 56);
    show("set leave idle early", set(56, SCHED_BATCH, 0, 19, 0, 0, 0, 0, 0, 0));
    show("set reset on fork", set(56, SCHED_IDLE, 1, 19, 0, 0, 0, 0, 0, 0));
    get("after reset on fork", 56);
    // A child of a reset-on-fork parent keeps a policy that is not real-time, without the custom
    // slice or the flag.
    show("set batch reset on fork", set(56, SCHED_BATCH, 1, 19, 0, 5000000, 0, 0, 0, 0));
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        get("child of reset batch", 56);
        fflush(stdout);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    show("set clear reset", set(56, SCHED_BATCH, 0, 19, 0, 0, 0, 0, 0, 0));
    show("set keep policy", set(56, SCHED_FIFO, 8, 19, 0, 0, 0, 0, 0, 0));
    show("set keep params", set(56, SCHED_IDLE, 0x10, 0, 0, 0, 0, 0, 0, 0));
    get("after keep params", 56);

    get("after clamp", 56);
    show("set clamp too big", set(56, SCHED_IDLE, 0x40, 19, 0, 0, 0, 0, 0, 2000));
    show("set clamp reset", set(56, SCHED_IDLE, 0x60, 19, 0, 0, 0, 0, -1, -1));
    get("after clamp reset", 56);
    show("set clamp min over max", set(56, SCHED_IDLE, 0x60, 19, 0, 0, 0, 0, 300, 200));
    show("set leave idle", set(56, SCHED_OTHER, 0, 19, 0, 0, 0, 0, 0, 0));
    show("getscheduler", sched_getscheduler(0));

    struct timespec interval = {123, 456};
    show("rr interval", syscall(SYS_sched_rr_get_interval, 0, &interval));
    printf("interval is the slice in whole ms: %d\n",
           interval.tv_sec == 0 && interval.tv_nsec == (long)(base_slice / 1000000 * 1000000));
    show("rr interval bad pid", syscall(SYS_sched_rr_get_interval, -1, &interval));
    show("rr interval missing", syscall(SYS_sched_rr_get_interval, 99999, &interval));
    show("rr interval fault", syscall(SYS_sched_rr_get_interval, 0, (void *)8));

    unsigned char vector[8];
    char *area = mmap(NULL, 4 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    munmap(area + 2 * 4096, 4096);
    show("mincore", mincore(area, 2 * 4096, vector));
    show("mincore unaligned", mincore(area + 1, 4096, vector));
    show("mincore gap", mincore(area, 4 * 4096, vector));
    show("mincore unmapped", mincore(area + 2 * 4096, 4096, vector));
    show("mincore fault", mincore(area, 4096, (void *)8));
    show("mincore huge", mincore(area, -4096, vector));
    show("mincore zero", mincore(area, 0, vector));
    show("mincore partial", mincore(area, 4097, vector));
    mprotect(area, 4096, PROT_NONE);
    show("mincore none", mincore(area, 4096, vector));
    // Another user's scheduling is not changed without CAP_SYS_NICE (`check_same_owner`).
    {
        pid_t parent = getpid();
        pid_t child = fork();
        if (child == 0) {
            struct sched_param keep = {.sched_priority = 0};
            if (seteuid(65534)) _exit(1);
            errno = 0;
            printf("another user's setparam %d %s\n", sched_setparam(parent, &keep), strerror(errno));
            errno = 0;
            printf("another user's setscheduler %d %s\n", sched_setscheduler(parent, SCHED_OTHER, &keep), strerror(errno));
            errno = 0;
            printf("its own setparam %d %s\n", sched_setparam(0, &keep), strerror(errno));
            _exit(0);
        }
        waitpid(child, NULL, 0);
    }
    return 0;
}
