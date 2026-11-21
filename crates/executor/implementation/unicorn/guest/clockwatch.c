// Watches the clocks while processes sleep and compute: the monotonic and boot clocks never go back,
// a sleep lasts at least as long as asked, and a timed wait ends by its timeout.

#define _GNU_SOURCE
#include <poll.h>
#include <stdio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static long long now(clockid_t clock) {
    struct timespec t;
    clock_gettime(clock, &t);
    return t.tv_sec * 1000000000LL + t.tv_nsec;
}

// Returns 0 if every check held.
static int watch(int rounds) {
    int failures = 0;
    long long last = now(CLOCK_MONOTONIC), last_boot = now(CLOCK_BOOTTIME);
    for (int i = 0; i < rounds; i++) {
        struct timespec ms = {0, 1000000};
        long long before = now(CLOCK_MONOTONIC);
        nanosleep(&ms, NULL);
        long long after = now(CLOCK_MONOTONIC);
        failures += after - before < 1000000;
        int fds[2];
        pipe(fds);
        struct pollfd poller = {fds[0], POLLIN, 0};
        before = now(CLOCK_MONOTONIC);
        failures += poll(&poller, 1, 2) != 0;
        failures += now(CLOCK_MONOTONIC) - before < 2000000;
        close(fds[0]);
        close(fds[1]);
        for (volatile int spin = 0; spin < 20000; spin++) {
        }
        long long monotonic = now(CLOCK_MONOTONIC), boot = now(CLOCK_BOOTTIME);
        failures += monotonic < last || boot < last_boot;
        last = monotonic;
        last_boot = boot;
    }
    return failures;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    pid_t children[3];
    for (int i = 0; i < 3; i++) {
        children[i] = fork();
        if (children[i] == 0) {
            _exit(watch(40) != 0);
        }
    }
    int failures = watch(40);
    for (int i = 0; i < 3; i++) {
        int status;
        waitpid(children[i], &status, 0);
        failures += WEXITSTATUS(status);
    }
    printf("the clocks held: %d\n", failures == 0);
    return 0;
}
