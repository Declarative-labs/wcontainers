// Timed waits that end while another process computes: each wait's deadline must end it although
// the CPU is never idle, as Linux's timer interrupt does.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <mqueue.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/ipc.h>
#include <sys/select.h>
#include <sys/sem.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

// A child that computes for ever, with system calls in its loop or without.
static pid_t spinner(int with_calls) {
    pid_t child = fork();
    if (child == 0) {
        if (with_calls) {
            struct timespec t;
            for (;;) clock_gettime(CLOCK_MONOTONIC, &t);
        }
        volatile unsigned long n = 0;
        for (;;) n++;
    }
    return child;
}

static void stop(pid_t child) {
    int status;
    kill(child, SIGKILL);
    waitpid(child, &status, 0);
    printf("  child killed: %d\n", WIFSIGNALED(status) ? WTERMSIG(status) : -1);
}

static const struct timespec twenty_ms = {0, 20 * 1000 * 1000};

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    for (int with_calls = 0; with_calls < 2; with_calls++) {
        printf("child computing %s system calls\n", with_calls ? "with" : "without");

        pid_t child = spinner(with_calls);
        printf("nanosleep %d\n", nanosleep(&twenty_ms, NULL));
        stop(child);

        child = spinner(with_calls);
        struct timespec at;
        clock_gettime(CLOCK_MONOTONIC, &at);
        at.tv_nsec += 20 * 1000 * 1000;
        if (at.tv_nsec >= 1000000000) {
            at.tv_sec++;
            at.tv_nsec -= 1000000000;
        }
        printf("clock_nanosleep absolute %d\n",
               clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &at, NULL));
        stop(child);

        int fds[2];
        pipe(fds);
        child = spinner(with_calls);
        struct pollfd poller = {fds[0], POLLIN, 0};
        printf("ppoll %d\n", ppoll(&poller, 1, &twenty_ms, NULL));
        fd_set readable;
        FD_ZERO(&readable);
        FD_SET(fds[0], &readable);
        printf("pselect %d\n", pselect(fds[0] + 1, &readable, NULL, NULL, &twenty_ms, NULL));
        stop(child);
        close(fds[0]);
        close(fds[1]);

        sigset_t set;
        sigemptyset(&set);
        sigaddset(&set, SIGUSR1);
        sigprocmask(SIG_BLOCK, &set, NULL);
        child = spinner(with_calls);
        int got = sigtimedwait(&set, NULL, &twenty_ms);
        printf("sigtimedwait %d errno %d\n", got, got < 0 ? errno : 0);
        stop(child);

        int semaphores = semget(IPC_PRIVATE, 1, 0600);
        child = spinner(with_calls);
        struct sembuf take = {0, -1, 0};
        int taken = semtimedop(semaphores, &take, 1, &twenty_ms);
        printf("semtimedop %d errno %d\n", taken, taken < 0 ? errno : 0);
        stop(child);
        semctl(semaphores, 0, IPC_RMID);

        mqd_t queue = mq_open("/deadlines", O_RDWR | O_CREAT, 0600, NULL);
        child = spinner(with_calls);
        char message[8192];
        clock_gettime(CLOCK_REALTIME, &at);
        at.tv_nsec += 20 * 1000 * 1000;
        if (at.tv_nsec >= 1000000000) {
            at.tv_sec++;
            at.tv_nsec -= 1000000000;
        }
        ssize_t received = mq_timedreceive(queue, message, sizeof message, NULL, &at);
        printf("mq_timedreceive %zd errno %d\n", received, received < 0 ? errno : 0);
        stop(child);
        mq_close(queue);
        mq_unlink("/deadlines");

        // Two computing children and a sleeper: the sleeper still wakes in time to stop both.
        pid_t first = spinner(with_calls);
        pid_t second = spinner(with_calls);
        printf("nanosleep beside two %d\n", nanosleep(&twenty_ms, NULL));
        stop(first);
        stop(second);
    }

    // A child that computes for a while and then ends: the parent's short sleep ends first.
    pid_t child = fork();
    if (child == 0) {
        volatile unsigned long n = 0;
        for (unsigned long i = 0; i < 30000000UL; i++) n++;
        printf("child done\n");
        _exit(0);
    }
    struct timespec five_ms = {0, 5 * 1000 * 1000};
    nanosleep(&five_ms, NULL);
    printf("parent woke\n");
    int status;
    waitpid(child, &status, 0);
    printf("status %d\n", status);
    return 0;
}
