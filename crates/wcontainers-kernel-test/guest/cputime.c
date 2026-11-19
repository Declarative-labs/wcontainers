// CPU time does not include time spent asleep: sleeps, a wait in ppoll, and a sleep on the
// process's own CPU clock that is already over.

#define _GNU_SOURCE
#include <poll.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/times.h>
#include <time.h>

static double seconds(clockid_t id) {
    struct timespec now;
    clock_gettime(id, &now);
    return now.tv_sec + now.tv_nsec / 1e9;
}

static void show(const char *when) {
    struct rusage usage;
    getrusage(RUSAGE_SELF, &usage);
    struct tms ticks;
    times(&ticks);
    printf("%s: process below 1 s %d, thread below 1 s %d, rusage below 1 s %d, times below 100 ticks %d\n",
           when, seconds(CLOCK_PROCESS_CPUTIME_ID) < 1, seconds(CLOCK_THREAD_CPUTIME_ID) < 1,
           usage.ru_utime.tv_sec + usage.ru_stime.tv_sec < 1, ticks.tms_utime + ticks.tms_stime < 100);
}

// The user time /proc/self/stat shows grows with work done (field 14, in clock ticks).
static unsigned long stat_utime(void) {
    unsigned long utime = 0;
    FILE *f = fopen("/proc/self/stat", "r");
    if (f) {
        fscanf(f, "%*d %*s %*c %*d %*d %*d %*d %*d %*u %*u %*u %*u %*u %lu", &utime);
        fclose(f);
    }
    return utime;
}

int main(void) {
    int reads = 0;
    while (stat_utime() == 0 && reads < 100000)
        reads++;
    printf("stat's user time grew: %d\n", stat_utime() > 0);
    show("start");
    struct timespec two = {2, 0};
    nanosleep(&two, 0);
    show("after nanosleep");
    clock_nanosleep(CLOCK_MONOTONIC, 0, &two, 0);
    show("after clock_nanosleep");
    ppoll(0, 0, &two, 0);
    show("after ppoll");
    struct timespec zero = {0, 0};
    printf("cpu clock sleep of zero: %d\n", clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, 0, &zero, 0));
    return 0;
}
