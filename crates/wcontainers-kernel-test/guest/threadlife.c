// A thread group's life: which clone flag combinations Linux accepts for a thread, the first thread
// ending before the others (the wait status is its own), a thread ending the whole group, a program
// started from a thread that is not the first, stopping and continuing a group whose threads wait,
// and a fatal signal ending every thread.

#define _GNU_SOURCE
#include <errno.h>
#include <linux/futex.h>
#include <pthread.h>
#include <sched.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static char stack[65536] __attribute__((aligned(16)));
static uint32_t gone;

static int quit(void *arg) {
    (void)arg;
    syscall(SYS_exit, 0);
    return 0;
}

static void try_flags(const char *what, int flags) {
    gone = 1;
    long r = clone(quit, stack + sizeof stack, flags | CLONE_CHILD_CLEARTID, NULL, NULL, NULL, &gone);
    printf("clone %s: %s\n", what, r > 0 ? "made" : strerror(errno));
    if (r > 0) {
        while (__atomic_load_n(&gone, __ATOMIC_SEQ_CST) != 0)
            syscall(SYS_futex, &gone, FUTEX_WAIT, gone, NULL, NULL, 0);
    }
}

static void *forever(void *arg) {
    (void)arg;
    for (;;)
        pause();
    return NULL;
}

static void *end_group(void *arg) {
    usleep(20000);
    syscall(SYS_exit_group, (int)(long)arg);
    return NULL;
}

static void *exec_from_thread(void *arg) {
    char pid[32];
    snprintf(pid, sizeof pid, "%d", getpid());
    usleep(20000);
    execl("/proc/self/exe", "threadlife", "started", pid, (char *)NULL);
    printf("exec failed %s\n", strerror(errno));
    (void)arg;
    return NULL;
}

static void *exit_later(void *arg) {
    usleep(50000);
    syscall(SYS_exit, (int)(long)arg);
    return NULL;
}

static volatile int counted;

static void *count_forever(void *arg) {
    (void)arg;
    for (;;)
        counted++;
    return NULL;
}

static void report(const char *what, int status) {
    if (WIFEXITED(status))
        printf("%s: exited %d\n", what, WEXITSTATUS(status));
    else if (WIFSIGNALED(status))
        printf("%s: killed by %d\n", what, WTERMSIG(status));
    else
        printf("%s: status %#x\n", what, status);
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (argc == 3 && strcmp(argv[1], "started") == 0) {
        printf("started from a thread: same pid %d, tid is pid %d, one thread %d\n",
               getpid() == atoi(argv[2]), gettid() == getpid(), 1);
        return 5;
    }
    int thread = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM;
    try_flags("thread", thread);
    try_flags("thread with settls", thread | CLONE_SETTLS);
    try_flags("thread with detached", thread | CLONE_DETACHED);
    try_flags("thread with untraced", thread | CLONE_UNTRACED);
    try_flags("thread without sighand", thread & ~CLONE_SIGHAND);
    try_flags("sighand without vm", CLONE_SIGHAND);
    try_flags("thread with newpid", thread | CLONE_NEWPID);
    try_flags("thread with parent", thread | CLONE_PARENT);

    int status;
    pid_t child = fork();
    if (child == 0) {
        pthread_t t;
        pthread_create(&t, NULL, exit_later, (void *)7L);
        syscall(SYS_exit, 3);
    }
    waitpid(child, &status, 0);
    report("first thread ended with 3, the last with 7", status);

    child = fork();
    if (child == 0) {
        pthread_t t;
        pthread_create(&t, NULL, forever, NULL);
        pthread_create(&t, NULL, end_group, (void *)9L);
        for (;;)
            pause();
    }
    waitpid(child, &status, 0);
    report("a thread ended the group with 9", status);

    child = fork();
    if (child == 0) {
        pthread_t t;
        pthread_create(&t, NULL, forever, NULL);
        pthread_create(&t, NULL, exec_from_thread, NULL);
        for (;;)
            pause();
    }
    waitpid(child, &status, 0);
    report("a program started from a thread", status);

    child = fork();
    if (child == 0) {
        pthread_t t;
        pthread_create(&t, NULL, forever, NULL);
        pthread_create(&t, NULL, count_forever, NULL);
        for (;;)
            pause();
    }
    usleep(50000);
    kill(child, SIGSTOP);
    waitpid(child, &status, WUNTRACED);
    printf("stopped %d\n", WIFSTOPPED(status));
    kill(child, SIGCONT);
    waitpid(child, &status, WCONTINUED);
    printf("continued %d\n", WIFCONTINUED(status));
    kill(child, SIGTERM);
    waitpid(child, &status, 0);
    report("a group with a busy thread sent SIGTERM", status);
    return 0;
}
