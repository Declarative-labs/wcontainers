// Work that SIGUSR1 may interrupt at any moment, written so that what it prints does not depend on
// whether or when the signal comes: pipes between processes, sleeps, threads with locks, waits.
#define _GNU_SOURCE
#include <errno.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t caught;
static void on_usr1(int sig) { (void)sig; caught++; }

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static long counter;

static void *count(void *arg) {
    (void)arg;
    for (int i = 0; i < 2000; i++) {
        pthread_mutex_lock(&lock);
        counter++;
        pthread_mutex_unlock(&lock);
        if (i % 500 == 0) {
            sched_yield();
        }
    }
    return 0;
}

static void sleep_ms(long ms) {
    struct timespec ts = {ms / 1000, (ms % 1000) * 1000000};
    while (nanosleep(&ts, &ts) != 0 && errno == EINTR) {
    }
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = on_usr1;
    sa.sa_flags = SA_RESTART;
    sigaction(SIGUSR1, &sa, 0);

    for (int round = 0; round < 3; round++) {
        int ends[2];
        pipe(ends);
        pid_t child = fork();
        if (child == 0) {
            close(ends[0]);
            char block[4096];
            for (int i = 0; i < 16; i++) {
                memset(block, 'a' + i, sizeof block);
                size_t done = 0;
                while (done < sizeof block) {
                    ssize_t n = write(ends[1], block + done, sizeof block - done);
                    if (n > 0) {
                        done += (size_t)n;
                    }
                }
            }
            _exit(round + 1);
        }
        close(ends[1]);
        long total = 0, sum = 0;
        char buffer[1000];
        for (;;) {
            struct pollfd p = {ends[0], POLLIN, 0};
            if (poll(&p, 1, 1000) < 0 && errno == EINTR) {
                continue;
            }
            ssize_t n = read(ends[0], buffer, sizeof buffer);
            if (n == 0) {
                break;
            }
            for (ssize_t i = 0; i < n; i++) {
                sum += buffer[i];
            }
            total += n;
        }
        close(ends[0]);
        int status;
        while (waitpid(child, &status, 0) < 0 && errno == EINTR) {
        }
        printf("round %d: %ld bytes, sum %ld, child status %d\n", round, total, sum,
               WEXITSTATUS(status));

        pthread_t threads[3];
        for (int i = 0; i < 3; i++) {
            pthread_create(&threads[i], 0, count, 0);
        }
        for (int i = 0; i < 3; i++) {
            pthread_join(threads[i], 0);
        }
        printf("round %d: counter %ld\n", round, counter);
        sleep_ms(3);
        printf("round %d: slept\n", round);
    }
    return 0;
}
