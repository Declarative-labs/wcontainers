// CPU time does not include time spent asleep: sleeps, a wait in ppoll, and a sleep on the
// process's own CPU clock that is already over. Time in system calls shows as system time, in
// the process's and, once reaped, in its parent's children's.

#define _GNU_SOURCE
#include <poll.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/times.h>
#include <sys/wait.h>
#include <unistd.h>
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

// Works and calls into the kernel in turn until the process has used a third of a second.
static void calls(void) {
    while (seconds(CLOCK_PROCESS_CPUTIME_ID) < 0.3) {
        for (volatile int i = 0; i < 1000; i++)
            ;
        getppid();
    }
}

// Whether the system times are above 0: times, getrusage and /proc/self/stat's field 15.
static void system_times(const char *who) {
    struct tms ticks;
    times(&ticks);
    struct rusage usage;
    getrusage(RUSAGE_SELF, &usage);
    unsigned long stime = 0;
    FILE *f = fopen("/proc/self/stat", "r");
    fscanf(f, "%*d %*s %*c %*d %*d %*d %*d %*d %*u %*u %*u %*u %*u %*u %lu", &stime);
    fclose(f);
    printf("%s: times stime %d utime %d, rusage stime %d, stat stime %d\n", who, ticks.tms_stime > 0,
           ticks.tms_utime > 0, usage.ru_stime.tv_sec + usage.ru_stime.tv_usec > 0, stime > 0);
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
    calls();
    system_times("calls");
    fflush(stdout);
    if (fork() == 0) {
        calls();
        _exit(0);
    }
    struct rusage child;
    wait4(-1, NULL, 0, &child);
    printf("waited child: utime %d stime %d\n", child.ru_utime.tv_sec + child.ru_utime.tv_usec > 0,
           child.ru_stime.tv_sec + child.ru_stime.tv_usec > 0);
    struct tms ticks;
    times(&ticks);
    struct rusage children;
    getrusage(RUSAGE_CHILDREN, &children);
    printf("children: cutime %d cstime %d, rusage utime %d stime %d\n", ticks.tms_cutime > 0,
           ticks.tms_cstime > 0, children.ru_utime.tv_sec + children.ru_utime.tv_usec > 0,
           children.ru_stime.tv_sec + children.ru_stime.tv_usec > 0);
    return 0;
}
