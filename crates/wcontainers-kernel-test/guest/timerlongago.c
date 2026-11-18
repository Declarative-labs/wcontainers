// Timers set to times on the wall clock from long before now, with an interval: the first expiry
// counts every interval since, a POSIX timer's overrun up to INT_MAX and a descriptor's ticks.
#define _GNU_SOURCE
#include <limits.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/timerfd.h>
#include <time.h>
#include <unistd.h>

static volatile int called, overrun;
static timer_t timer;

static void handler(int signal) {
    (void)signal;
    if (called) return;
    overrun = timer_getoverrun(timer);
    struct itimerspec off;
    memset(&off, 0, sizeof off);
    timer_settime(timer, 0, &off, NULL);
    called = 1;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    signal(SIGUSR1, handler);
    struct sigevent event;
    memset(&event, 0, sizeof event);
    event.sigev_notify = SIGEV_SIGNAL;
    event.sigev_signo = SIGUSR1;
    timer_create(CLOCK_REALTIME, &event, &timer);
    struct itimerspec spec;
    memset(&spec, 0, sizeof spec);
    clock_gettime(CLOCK_REALTIME, &spec.it_value);
    spec.it_value.tv_sec -= 21;
    spec.it_interval.tv_nsec = 1;
    printf("settime 21 s ago every nanosecond: %d\n", timer_settime(timer, TIMER_ABSTIME, &spec, NULL));
    while (!called)
        ;
    printf("overrun capped %d\n", overrun == INT_MAX);
    timer_delete(timer);
    int fd = timerfd_create(CLOCK_REALTIME, 0);
    struct itimerspec every = {{0, 1000000}, {0, 0}};
    clock_gettime(CLOCK_REALTIME, &every.it_value);
    every.it_value.tv_sec -= 100;
    printf("timerfd 100 s ago every millisecond: %d\n", timerfd_settime(fd, TFD_TIMER_ABSTIME, &every, NULL));
    uint64_t ticks = 0;
    long r = read(fd, &ticks, 8);
    printf("read %ld: at least 100000 ticks %d, under 200000 %d\n", r, ticks >= 100000, ticks < 200000);
    return 0;
}
