// kill and rt_sigqueueinfo given a thread's number other than the first: the signal goes to the
// whole process, as Linux finds any task by its number.
#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static volatile int caught, thread_id;

static void on_signal(int signal, siginfo_t *info, void *context) {
    (void)context;
    caught = signal * 1000 + (info->si_code == SI_QUEUE ? 1 : 0);
}

static void *worker(void *arg) {
    (void)arg;
    thread_id = syscall(SYS_gettid);
    for (;;) pause();
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    struct sigaction action = {.sa_sigaction = on_signal, .sa_flags = SA_SIGINFO};
    sigaction(SIGUSR1, &action, NULL);
    sigaction(SIGUSR2, &action, NULL);
    pthread_t t;
    pthread_create(&t, NULL, worker, NULL);
    while (!thread_id)
        ;
    sigset_t block;
    sigemptyset(&block);
    sigaddset(&block, SIGUSR1);
    sigaddset(&block, SIGUSR2);
    pthread_sigmask(SIG_BLOCK, &block, NULL);
    printf("kill the thread's number: %d\n", kill(thread_id, SIGUSR1));
    while (!caught)
        ;
    printf("caught %d\n", caught);
    caught = 0;
    siginfo_t info;
    memset(&info, 0, sizeof info);
    info.si_code = SI_QUEUE;
    info.si_pid = getpid();
    info.si_uid = getuid();
    printf("rt_sigqueueinfo the thread's number: %ld\n", syscall(SYS_rt_sigqueueinfo, thread_id, SIGUSR2, &info));
    while (!caught)
        ;
    printf("caught %d\n", caught);
    errno = 0;
    printf("kill no such number: %d %s\n", kill(999999, SIGUSR1), strerror(errno));
    return 0;
}
