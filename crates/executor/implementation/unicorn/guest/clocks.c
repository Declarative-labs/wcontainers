// Reads, asks the resolution of and sleeps on every kind of clock id: the fixed ones and the CPU clocks
// below zero (of the caller's process and thread, of process 1, of no process, and clock devices),
// printing what each call gave.

#include <errno.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

// A call's result, or its error negated.
static long result(long value) {
    return value < 0 ? -errno : value;
}

int main(void) {
    int pid = getpid();
    int pids[] = {0, pid, 1, 99999, 2};
    int ids[64];
    int count = 0;
    for (int bits = 0; bits < 8; bits++) {
        for (int index = 0; index < 5; index++) {
            ids[count++] = (~pids[index] << 3) | bits;
        }
    }
    for (int id = 0; id <= 12; id++) {
        ids[count++] = id;
    }
    for (int index = 0; index < count; index++) {
        int id = ids[index];
        struct timespec value = {7, 7}, resolution = {7, 7}, zero = {0, 0};
        long got = result(syscall(SYS_clock_gettime, id, &value));
        long res = result(syscall(SYS_clock_getres, id, &resolution));
        long none = result(syscall(SYS_clock_getres, id, NULL));
        // Sleeps that end at once: relative 0, an absolute time long past, and an unreadable duration.
        long relative = result(syscall(SYS_clock_nanosleep, id, 0, &zero, NULL));
        long absolute = result(syscall(SYS_clock_nanosleep, id, TIMER_ABSTIME, &zero, NULL));
        long flagged = result(syscall(SYS_clock_nanosleep, id, 2, &zero, NULL));
        long bad = result(syscall(SYS_clock_nanosleep, id, TIMER_ABSTIME, (void *)1, NULL));
        printf("id %d: gettime %ld res %ld %ld.%09ld res(null) %ld sleep %ld %ld %ld %ld\n", id, got, res,
               (long)resolution.tv_sec, (long)resolution.tv_nsec, none, relative, absolute, flagged, bad);
    }
    return 0;
}
