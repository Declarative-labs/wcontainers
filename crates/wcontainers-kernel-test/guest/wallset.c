// Timers and waits on the wall clock when the wall clock is set forward two hours while they wait
// for a time one hour ahead (the test host moves it at each save). Timers on the wall clock's base
// fire, a timer descriptor set with TFD_TIMER_CANCEL_ON_SET is canceled, absolute sleeps and futex
// waits end; relative POSIX timers on the wall clock and monotonic ones go on.
#define _GNU_SOURCE
#include <errno.h>
#include <linux/futex.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/timerfd.h>
#include <time.h>
#include <unistd.h>

#define HOUR 3600

static uint32_t word;
static int slept;
static long waited, wait_error;

// An hour from now on the wall clock, read just before each use, so a save may come at any point.
static struct timespec hour_ahead(void) {
    struct timespec at;
    clock_gettime(CLOCK_REALTIME, &at);
    at.tv_sec += HOUR;
    return at;
}

static void *sleeper(void *arg) {
    (void)arg;
    struct timespec at = hour_ahead();
    slept = clock_nanosleep(CLOCK_REALTIME, TIMER_ABSTIME, &at, NULL);
    return NULL;
}

static void *waiter(void *arg) {
    (void)arg;
    struct timespec at = hour_ahead();
    waited = syscall(SYS_futex, &word, FUTEX_WAIT_BITSET_PRIVATE | FUTEX_CLOCK_REALTIME, 0, &at,
                     NULL, FUTEX_BITSET_MATCH_ANY);
    wait_error = errno;
    return NULL;
}

static int descriptor(clockid_t clock, int flags, time_t seconds) {
    int fd = timerfd_create(clock, TFD_NONBLOCK);
    struct itimerspec value = {.it_value = {seconds, 0}};
    if (timerfd_settime(fd, flags, &value, NULL) != 0) printf("timerfd_settime failed: %s\n", strerror(errno));
    return fd;
}

static void show(const char *what, int fd) {
    uint64_t ticks = 0;
    ssize_t r = read(fd, &ticks, sizeof ticks);
    printf("%s: %zd %s\n", what, r, r < 0 ? strerror(errno) : "fired");
}

static timer_t posix(int signal, int flags, time_t seconds) {
    struct sigevent event = {.sigev_notify = SIGEV_SIGNAL, .sigev_signo = signal};
    timer_t timer;
    timer_create(CLOCK_REALTIME, &event, &timer);
    struct itimerspec value = {.it_value = {seconds, 0}};
    timer_settime(timer, flags, &value, NULL);
    return timer;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    sigset_t blocked;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGUSR1);
    sigaddset(&blocked, SIGUSR2);
    sigprocmask(SIG_BLOCK, &blocked, NULL);
    struct timespec start;
    clock_gettime(CLOCK_MONOTONIC, &start);

    int cancel = descriptor(CLOCK_REALTIME, TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET, hour_ahead().tv_sec);
    int absolute = descriptor(CLOCK_REALTIME, TFD_TIMER_ABSTIME, hour_ahead().tv_sec);
    int relative = descriptor(CLOCK_REALTIME, 0, HOUR);
    int monotonic = descriptor(CLOCK_MONOTONIC, 0, HOUR);
    timer_t posix_absolute = posix(SIGUSR1, TIMER_ABSTIME, hour_ahead().tv_sec);
    timer_t posix_relative = posix(SIGUSR2, 0, HOUR);
    pthread_t a, b;
    pthread_create(&a, NULL, sleeper, NULL);
    pthread_create(&b, NULL, waiter, NULL);

    // The threads wait, then turns enough for the saves, which move the wall clock.
    usleep(10000);
    for (int i = 0; i < 3000; i++) getppid();
    pthread_join(a, NULL);
    pthread_join(b, NULL);
    printf("absolute sleep on the wall clock: %d\n", slept);
    printf("futex wait until a wall clock time: %ld %s\n", waited, strerror(wait_error));
    struct timespec end;
    clock_gettime(CLOCK_MONOTONIC, &end);
    printf("waits ended before the hour on the monotonic clock: %d\n", end.tv_sec - start.tv_sec < HOUR);

    show("canceled on set", cancel);
    show("canceled on set, read again", cancel);
    struct itimerspec value = {.it_value = {hour_ahead().tv_sec, 0}};
    printf("set again: %d\n", timerfd_settime(cancel, TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET, &value, NULL));
    show("absolute on the wall clock", absolute);
    show("relative on the wall clock", relative);
    show("relative on the monotonic clock", monotonic);
    sigset_t pending;
    sigpending(&pending);
    printf("absolute POSIX timer fired: %d\n", sigismember(&pending, SIGUSR1));
    printf("relative POSIX timer fired: %d\n", sigismember(&pending, SIGUSR2));
    struct itimerspec left;
    timer_gettime(posix_relative, &left);
    printf("relative POSIX timer has most of its hour left: %d\n", left.it_value.tv_sec > HOUR - 60);
    (void)posix_absolute;
    return 0;
}
