// SIGTERM sent to the container's first process, which has no handler for it, while its first
// thread blocks it and a second thread does not.
#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

static void *second(void *unused) {
    (void)unused;
    for (;;)
        pause();
    return 0;
}

int main(void) {
    setvbuf(stdout, 0, _IOLBF, 0);
    printf("first: %d\n", getpid() == 1);
    pthread_t thread;
    pthread_create(&thread, 0, second, 0);
    usleep(100000);
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGTERM);
    pthread_sigmask(SIG_BLOCK, &set, 0);
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        kill(getppid(), SIGTERM);
        _exit(0);
    }
    int status = 0;
    waitpid(child, &status, 0);
    usleep(200000);
    sigset_t pending;
    sigpending(&pending);
    printf("lived past SIGTERM, pending %d\n", sigismember(&pending, SIGTERM));
    return 0;
}
