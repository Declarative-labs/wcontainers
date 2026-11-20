// What a process's tasks see when the embedder takes some of them off the kernel and puts them back:
// on the reference nothing is taken, so every answer is Linux's own for a task that ran throughout.
//   join:    a child's worker ends while its main thread waits to join it; the child then exits.
//   kill:    a child blocked in a read is sent a signal (the number given), then its pipe closes.
//   timer:   a thread's own timer fires while it waits in a read; the main thread writes later.
//   wait:    a parent asks (without waiting, then waiting) for a child that sleeps, then exits.
//   mapping: the process maps a memory file shared and writable, prints where, and unmaps it.
//   raise:   the process raises a signal it handles.

#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum {
    AWAY_JOIN_EXIT = 7,
    AWAY_WAIT_EXIT = 5,
    AWAY_WORKER_MS = 200,
    AWAY_CHILD_MS = 300,
    AWAY_PARENT_MS = 100,
    AWAY_TIMER_MS = 150,
    AWAY_WRITE_MS = 300,
    AWAY_LATER_MS = 100,
    AWAY_PAGES = 2,
    MS = 1000 * 1000,
    SECOND_MS = 1000,
};

static void away_sleep(long ms) {
    struct timespec time = {ms / SECOND_MS, (ms % SECOND_MS) * MS};
    nanosleep(&time, NULL);
}

static void away_report(int status) {
    if (WIFEXITED(status)) {
        printf("child exit %d\n", WEXITSTATUS(status));
    } else if (WIFSIGNALED(status)) {
        printf("child signal %d\n", WTERMSIG(status));
    }
}

static void *away_worker(void *argument) {
    (void)argument;
    away_sleep(AWAY_WORKER_MS);
    return NULL;
}

static int away_join(void) {
    pid_t child = fork();
    if (child == 0) {
        pthread_t worker;
        pthread_create(&worker, NULL, away_worker, NULL);
        pthread_join(worker, NULL);
        printf("joined\n");
        fflush(stdout);
        _exit(AWAY_JOIN_EXIT);
    }
    int status;
    waitpid(child, &status, 0);
    away_report(status);
    return 0;
}

static int away_kill(int signal) {
    int pipe_ends[2];
    pipe(pipe_ends);
    pid_t child = fork();
    if (child == 0) {
        close(pipe_ends[1]);
        char byte;
        long answer = read(pipe_ends[0], &byte, 1);
        printf("read %ld\n", answer);
        fflush(stdout);
        _exit(0);
    }
    close(pipe_ends[0]);
    away_sleep(AWAY_PARENT_MS);
    printf("kill %d\n", kill(child, signal));
    fflush(stdout);
    away_sleep(AWAY_PARENT_MS);
    close(pipe_ends[1]);
    int status;
    waitpid(child, &status, 0);
    away_report(status);
    return 0;
}

static int away_timer_pipe[2];
static volatile sig_atomic_t away_handled;
static long away_read_answer;
static int away_read_error;

static void away_handler(int signal) {
    (void)signal;
    away_handled++;
}

static void *away_timed_reader(void *argument) {
    (void)argument;
    struct sigevent event;
    memset(&event, 0, sizeof event);
    event.sigev_notify = SIGEV_THREAD_ID;
    event.sigev_signo = SIGUSR1;
    event.sigev_notify_thread_id = gettid();
    timer_t timer;
    timer_create(CLOCK_MONOTONIC, &event, &timer);
    struct itimerspec when = {{0, 0}, {0, AWAY_TIMER_MS * (long)MS}};
    timer_settime(timer, 0, &when, NULL);
    char byte;
    away_read_answer = read(away_timer_pipe[0], &byte, 1);
    away_read_error = away_read_answer < 0 ? errno : 0;
    return NULL;
}

static int away_timer(void) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = away_handler;
    sigaction(SIGUSR1, &action, NULL);
    pipe(away_timer_pipe);
    pthread_t reader;
    pthread_create(&reader, NULL, away_timed_reader, NULL);
    away_sleep(AWAY_WRITE_MS);
    // A mark the embedder's test acts on.
    getppid();
    away_sleep(AWAY_LATER_MS);
    write(away_timer_pipe[1], "", 1);
    pthread_join(reader, NULL);
    printf("read %ld %s handled %d\n", away_read_answer, strerror(away_read_error), away_handled);
    return 0;
}

static int away_wait(void) {
    pid_t child = fork();
    if (child == 0) {
        away_sleep(AWAY_CHILD_MS);
        _exit(AWAY_WAIT_EXIT);
    }
    away_sleep(AWAY_PARENT_MS);
    int status;
    printf("no wait %d\n", waitpid(child, &status, WNOHANG));
    if (waitpid(child, &status, 0) != child) {
        printf("wait %s\n", strerror(errno));
        return 0;
    }
    away_report(status);
    return 0;
}

static int away_mapping(void) {
    long size = AWAY_PAGES * sysconf(_SC_PAGESIZE);
    int file = memfd_create("away", 0);
    ftruncate(file, size);
    away_sleep(AWAY_PARENT_MS);
    char *memory = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, file, 0);
    printf("mapped %lx %lx\n", (unsigned long)memory, (unsigned long)memory + size);
    fflush(stdout);
    munmap(memory, size);
    printf("unmapped\n");
    return 0;
}

static int away_raise(void) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = away_handler;
    sigaction(SIGUSR1, &action, NULL);
    raise(SIGUSR1);
    printf("handled %d\n", away_handled);
    return 0;
}

int main(int count, char **arguments) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    const char *mode = count > 1 ? arguments[1] : "";
    if (strcmp(mode, "join") == 0) {
        return away_join();
    }
    if (strcmp(mode, "kill") == 0 && count > 2) {
        return away_kill(atoi(arguments[2]));
    }
    if (strcmp(mode, "timer") == 0) {
        return away_timer();
    }
    if (strcmp(mode, "wait") == 0) {
        return away_wait();
    }
    if (strcmp(mode, "mapping") == 0) {
        return away_mapping();
    }
    if (strcmp(mode, "raise") == 0) {
        return away_raise();
    }
    fprintf(stderr, "unknown mode %s\n", mode);
    return 1;
}
