// Which thread takes a signal sent to the process when several could: the first thread if it
// wants it, else the next willing thread from the last one chosen. The first thread is shown
// willing while it runs (it sends the signal itself): asleep, the loaded reference sometimes
// passed it over.
#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static volatile pid_t taker;
static volatile int handled;
static pid_t ids[4];
static volatile int ready;

static void handler(int sig) {
    (void)sig;
    taker = gettid();
    handled++;
}

static int which(pid_t tid) {
    for (int i = 0; i < 4; i++)
        if (ids[i] == tid) return i;
    return -1;
}

static void *idle(void *arg) {
    long me = (long)arg;
    ids[me] = gettid();
    sigset_t open;
    sigemptyset(&open);
    sigaddset(&open, SIGUSR1);
    pthread_sigmask(SIG_UNBLOCK, &open, NULL);
    __sync_fetch_and_add(&ready, 1);
    for (;;) pause();
    return NULL;
}

// A child sends the signal, so the sender is not one of the threads. It waits for the parent's
// word first: the C library's fork blocks every signal until it returns in the parent.
static void send_from_child(void) {
    int before = handled;
    pid_t parent = getpid();
    int go[2];
    pipe(go);
    pid_t child = fork();
    if (child == 0) {
        char byte;
        close(go[1]);
        read(go[0], &byte, 1);
        kill(parent, SIGUSR1);
        _exit(0);
    }
    close(go[0]);
    write(go[1], "g", 1);
    close(go[1]);
    waitpid(child, NULL, 0);
    while (handled == before) {
        struct timespec ts = {0, 1000000};
        nanosleep(&ts, NULL);
    }
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    signal(SIGUSR1, handler);
    ids[0] = gettid();
    sigset_t block;
    sigemptyset(&block);
    sigaddset(&block, SIGUSR1);
    pthread_sigmask(SIG_BLOCK, &block, NULL);
    pthread_t t;
    for (long i = 1; i < 4; i++) pthread_create(&t, NULL, idle, (void *)i);
    while (ready < 3) usleep(1000);
    usleep(10000);
    for (int round = 0; round < 3; round++) {
        send_from_child();
        printf("main blocking: taken by thread %d\n", which(taker));
    }
    pthread_sigmask(SIG_UNBLOCK, &block, NULL);
    for (int round = 0; round < 2; round++) {
        int before = handled;
        kill(getpid(), SIGUSR1);
        while (handled == before) {
            struct timespec ts = {0, 1000000};
            nanosleep(&ts, NULL);
        }
        printf("main willing: taken by thread %d\n", which(taker));
    }
    return 0;
}
