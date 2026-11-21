// A time namespace with offsets: what /proc/self/timens_offsets and the namespace links show, the
// monotonic and boot time clocks, uptime, and absolute sleeps, timers and futex waits on the moved
// clocks. With "unshare MONOTONIC BOOTTIME" (seconds) it makes the namespace itself first, as
// runc does for a container's timeOffsets; without, it is in one already.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/futex.h>
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/sysinfo.h>
#include <sys/timerfd.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifndef CLONE_NEWTIME
#define CLONE_NEWTIME 0x80
#endif

static int64_t now(clockid_t clock) {
    struct timespec t;
    clock_gettime(clock, &t);
    return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}

static struct timespec at(int64_t nanoseconds) {
    return (struct timespec){nanoseconds / 1000000000, nanoseconds % 1000000000};
}

static void show(void) {
    char text[256] = {0};
    int file = open("/proc/self/timens_offsets", O_RDONLY);
    long monotonic = 0, boottime = 0;
    if (file >= 0) {
        read(file, text, sizeof text - 1);
        close(file);
        printf("%s", text);
        sscanf(text, "monotonic %ld %*d boottime %ld", &monotonic, &boottime);
    }
    char own[64] = {0}, children[64] = {0};
    readlink("/proc/self/ns/time", own, sizeof own - 1);
    readlink("/proc/self/ns/time_for_children", children, sizeof children - 1);
    printf("time and time_for_children the same: %d\n", strcmp(own, children) == 0);
    printf("the machine's first time namespace: %d\n", strcmp(own, "time:[4026531834]") == 0);
    int64_t mono = now(CLOCK_MONOTONIC), boot = now(CLOCK_BOOTTIME);
    printf("monotonic past its offset: %d\n", mono >= monotonic * 1000000000LL);
    printf("boot time past its offset: %d\n", boot >= boottime * 1000000000LL);
    int64_t apart = (boot - mono) / 1000000000 - (boottime - monotonic);
    printf("boot time ahead of monotonic by the offsets: %d\n", apart >= -1 && apart <= 1);
    double up = 0;
    FILE *uptime = fopen("/proc/uptime", "r");
    if (uptime) {
        fscanf(uptime, "%lf", &up);
        fclose(uptime);
    }
    printf("uptime past the boot offset: %d\n", up >= boottime);
    struct sysinfo info;
    sysinfo(&info);
    printf("sysinfo uptime past the boot offset: %d\n", info.uptime >= boottime);

    int64_t start = now(CLOCK_MONOTONIC);
    struct timespec until = at(start + 20000000);
    int result = clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &until, NULL);
    int64_t slept = now(CLOCK_MONOTONIC) - start;
    printf("absolute monotonic sleep: %d, %d\n", result,
           slept >= 20000000 && slept < 2000000000);
    start = now(CLOCK_BOOTTIME);
    until = at(start - 1000000000);
    result = clock_nanosleep(CLOCK_BOOTTIME, TIMER_ABSTIME, &until, NULL);
    printf("absolute boot time sleep in the past: %d, %d\n", result,
           now(CLOCK_BOOTTIME) - start < 1000000000);

    int timer = timerfd_create(CLOCK_MONOTONIC, 0);
    struct itimerspec value = {.it_value = at(now(CLOCK_MONOTONIC) + 10000000)};
    timerfd_settime(timer, TFD_TIMER_ABSTIME, &value, NULL);
    uint64_t expirations = 0;
    start = now(CLOCK_MONOTONIC);
    read(timer, &expirations, sizeof expirations);
    printf("absolute timerfd: %llu, %d\n", (unsigned long long)expirations,
           now(CLOCK_MONOTONIC) - start < 2000000000);
    close(timer);

    timer_t posix;
    struct sigevent event = {.sigev_notify = SIGEV_NONE};
    timer_create(CLOCK_BOOTTIME, &event, &posix);
    value = (struct itimerspec){.it_value = at(now(CLOCK_BOOTTIME) + 3000000000LL)};
    timer_settime(posix, TIMER_ABSTIME, &value, NULL);
    struct itimerspec left;
    timer_gettime(posix, &left);
    printf("absolute boot time timer left about 3 s: %d\n",
           left.it_value.tv_sec >= 2 && left.it_value.tv_sec <= 3);
    timer_delete(posix);

    int word = 0;
    struct timespec deadline = at(now(CLOCK_MONOTONIC) + 10000000);
    start = now(CLOCK_MONOTONIC);
    long waited = syscall(SYS_futex, &word, FUTEX_WAIT_BITSET | FUTEX_PRIVATE_FLAG, 0, &deadline,
                          NULL, FUTEX_BITSET_MATCH_ANY);
    printf("absolute futex wait: %ld %s, %d\n", waited, strerror(errno),
           now(CLOCK_MONOTONIC) - start < 2000000000);

    file = open("/proc/self/timens_offsets", O_WRONLY);
    errno = 0;
    long wrote = write(file, "monotonic 1 0\n", 14);
    printf("offsets written again: %ld %s\n", wrote, strerror(errno));
    close(file);
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc > 3 && strcmp(argv[1], "unshare") == 0) {
        if (unshare(CLONE_NEWTIME) != 0) {
            printf("unshare: %s\n", strerror(errno));
            return 1;
        }
        char offsets[128];
        int length = snprintf(offsets, sizeof offsets, "monotonic %s 0\nboottime %s 0\n", argv[2],
                              argv[3]);
        int file = open("/proc/self/timens_offsets", O_WRONLY);
        if (write(file, offsets, length) != length) {
            printf("offsets: %s\n", strerror(errno));
            return 1;
        }
        close(file);
        pid_t child = fork();
        if (child == 0) {
            show();
            return 0;
        }
        int status = 0;
        waitpid(child, &status, 0);
        return WEXITSTATUS(status);
    }
    show();
    return 0;
}
