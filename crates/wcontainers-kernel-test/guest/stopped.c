// A child's threads wait in calls Linux answers differently when a stop arrives (a futex wait,
// one already woken, a sleep, a write half done, a semaphore, epoll_wait, a pipe read); the parent
// stops the child, changes and wakes the futex and counts the semaphore's waiters while it is
// stopped, continues it, gives the read its byte and empties the pipe. The child prints what each
// call answered, then the parent what it saw.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/futex.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/mman.h>
#include <sys/sem.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum { STOPPED_CALLS = 7, STOPPED_SLEEP_MS = 600, STOPPED_SETTLE_MS = 300, MS = 1000 * 1000 };
static int *stopped_words, stopped_full[2], stopped_empty[2], stopped_data[2];
static int stopped_semaphore, stopped_epoll;
static long stopped_answer[STOPPED_CALLS];
static int stopped_error[STOPPED_CALLS], stopped_slept_enough;

static void stopped_record(int at, long answer) {
    stopped_answer[at] = answer;
    stopped_error[at] = answer < 0 ? errno : 0;
}

static void *stopped_call(void *argument) {
    int at = (int)(long)argument;
    switch (at) {
    case 0:
        stopped_record(at, syscall(SYS_futex, &stopped_words[0], FUTEX_WAIT, 0, NULL, NULL, 0));
        break;
    case 1: {
        struct timespec limit = {3, 0};
        stopped_record(at, syscall(SYS_futex, &stopped_words[1], FUTEX_WAIT, 0, &limit, NULL, 0));
        break;
    }
    case 2: {
        struct timespec sleep = {0, STOPPED_SLEEP_MS * (long)MS}, before, after;
        clock_gettime(CLOCK_MONOTONIC, &before);
        stopped_record(at, syscall(SYS_nanosleep, &sleep, NULL));
        clock_gettime(CLOCK_MONOTONIC, &after);
        long elapsed = (after.tv_sec - before.tv_sec) * 1000 * MS + after.tv_nsec - before.tv_nsec;
        stopped_slept_enough = elapsed >= STOPPED_SLEEP_MS * (long)MS;
        break;
    }
    case 3: {
        long size = 2 * fcntl(stopped_full[1], F_GETPIPE_SZ);
        char *bytes = calloc(size, 1);
        stopped_record(at, write(stopped_full[1], bytes, size));
        close(stopped_full[1]);
        free(bytes);
        break;
    }
    case 4: {
        struct sembuf take = {0, -1, 0};
        stopped_record(at, semop(stopped_semaphore, &take, 1));
        break;
    }
    case 5: {
        struct epoll_event event;
        stopped_record(at, epoll_wait(stopped_epoll, &event, 1, -1));
        break;
    }
    case 6: {
        char byte;
        stopped_record(at, read(stopped_data[0], &byte, 1));
        break;
    }
    }
    return NULL;
}

static int stopped_child(void) {
    pthread_t threads[STOPPED_CALLS];
    for (long at = 0; at < STOPPED_CALLS; at++)
        pthread_create(&threads[at], NULL, stopped_call, (void *)at);
    for (int at = 0; at < STOPPED_CALLS; at++) pthread_join(threads[at], NULL);
    for (int at = 0; at < STOPPED_CALLS; at++)
        printf("call %d answered %ld %s\n", at, stopped_answer[at], strerror(stopped_error[at]));
    printf("slept enough %d\n", stopped_slept_enough);
    return 0;
}

int main(void) {
    stopped_words = mmap(NULL, sysconf(_SC_PAGESIZE), PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    pipe(stopped_full);
    pipe(stopped_empty);
    pipe(stopped_data);
    stopped_semaphore = semget(IPC_PRIVATE, 1, IPC_CREAT | 0600);
    stopped_epoll = epoll_create1(0);
    struct epoll_event event = {.events = EPOLLIN};
    epoll_ctl(stopped_epoll, EPOLL_CTL_ADD, stopped_empty[0], &event);
    pid_t child = fork();
    if (!child) exit(stopped_child());
    close(stopped_full[1]);
    struct timespec settle = {0, STOPPED_SETTLE_MS * (long)MS};
    nanosleep(&settle, NULL);
    long woken = syscall(SYS_futex, &stopped_words[1], FUTEX_WAKE, 1, NULL, NULL, 0);
    kill(child, SIGSTOP);
    pid_t stopped_pid = waitpid(child, NULL, WUNTRACED);
    __atomic_store_n(&stopped_words[0], 1, __ATOMIC_SEQ_CST);
    long woken_while_stopped = syscall(SYS_futex, &stopped_words[0], FUTEX_WAKE, 1, NULL, NULL, 0);
    int semaphore_waiters = semctl(stopped_semaphore, 0, GETNCNT);
    kill(child, SIGCONT);
    nanosleep(&settle, NULL);
    write(stopped_data[1], "x", 1);
    char bytes[PIPE_BUF];
    long drained = 0, got;
    while ((got = read(stopped_full[0], bytes, sizeof bytes)) > 0) drained += got;
    int status;
    waitpid(child, &status, 0);
    semctl(stopped_semaphore, 0, IPC_RMID);
    printf("woken %ld\nstopped %d\nwoken while stopped %ld\nsemaphore waiters %d\n", woken,
           stopped_pid == child, woken_while_stopped, semaphore_waiters);
    printf("drained %ld\nchild exit %d\n", drained, WEXITSTATUS(status));
    return 0;
}
