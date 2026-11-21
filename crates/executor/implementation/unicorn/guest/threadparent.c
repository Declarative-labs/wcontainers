// A child's parent is the thread that made it: its parent death signal comes when that thread ends
// while the process goes on, a wait with __WNOTHREAD sees only the caller's own children, and a
// wait looks at the caller's children before the other threads'.
#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <unistd.h>

static int report[2];

static void got(int sig) {
    char c = (char)sig;
    write(report[1], &c, 1);
}

// Forks a child that waits for its parent death signal, and returns once it has asked for it.
static void *maker(void *arg) {
    pid_t *out = arg;
    int ready[2];
    pipe(ready);
    pid_t child = fork();
    if (child == 0) {
        signal(SIGUSR1, got);
        prctl(PR_SET_PDEATHSIG, SIGUSR1);
        write(ready[1], "r", 1);
        for (int i = 0; i < 200; i++) usleep(5000);
        char c = 0;
        write(report[1], &c, 1);
        _exit(0);
    }
    *out = child;
    char c;
    read(ready[0], &c, 1);
    return NULL;
}

static void show(const char *what, pid_t result, pid_t child) {
    printf("%s: %s %s\n", what, result == child ? "the child" : result == 0 ? "none" : "-1",
           result < 0 ? strerror(errno) : "");
}

// Threads that each fork a child ending with a given status; one told to wait takes a child, then
// looks for one of its own with __WNOTHREAD.
struct forker {
    int status;
    pid_t child;
    volatile int forked, wait;
};
static void *forker(void *arg) {
    struct forker *f = arg;
    pid_t child = fork();
    if (child == 0) _exit(f->status);
    f->child = child;
    f->forked = 1;
    while (!f->wait) usleep(1000);
    if (f->wait > 0) {
        int status;
        pid_t r = wait(&status);
        printf("a thread's wait takes its own child before an older one: %d\n",
               r == f->child && WEXITSTATUS(status) == f->status);
        errno = 0;
        show("then __WNOTHREAD any", waitpid(-1, &status, __WNOTHREAD | WNOHANG), 0);
    }
    return NULL;
}

static void both(struct forker *one, struct forker *two, pthread_t *a, pthread_t *b) {
    pthread_create(a, NULL, forker, one);
    while (!one->forked) usleep(1000);
    pthread_create(b, NULL, forker, two);
    while (!two->forked) usleep(1000);
    siginfo_t info;
    waitid(P_PID, one->child, &info, WEXITED | WNOWAIT);
    waitid(P_PID, two->child, &info, WEXITED | WNOWAIT);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    pipe(report);

    pid_t child;
    pthread_t t;
    pthread_create(&t, NULL, maker, &child);
    pthread_join(t, NULL);
    char c;
    read(report[0], &c, 1);
    printf("child got %d when its making thread ended (process alive)\n", c);
    int status;
    errno = 0;
    show("its child now the first thread's, __WNOTHREAD", waitpid(child, &status, __WNOTHREAD | WNOHANG), child);
    kill(child, SIGKILL);
    show("reaped with __WNOTHREAD", waitpid(child, &status, __WNOTHREAD), child);

    struct forker one = {1}, two = {2};
    pthread_t a, b;
    both(&one, &two, &a, &b);
    errno = 0;
    show("__WNOTHREAD for another thread's child", waitpid(one.child, &status, __WNOTHREAD | WNOHANG), 0);
    errno = 0;
    show("__WNOTHREAD any", waitpid(-1, &status, __WNOTHREAD | WNOHANG), 0);
    // The first thread has none; a wait looks at the threads after it in turn.
    pid_t r = waitpid(-1, &status, 0);
    printf("first thread's wait takes the first forker's child: %d\n", r == one.child && WEXITSTATUS(status) == 1);
    r = waitpid(-1, &status, 0);
    printf("then the second's: %d\n", r == two.child && WEXITSTATUS(status) == 2);
    one.wait = two.wait = -1;
    pthread_join(a, NULL);
    pthread_join(b, NULL);

    struct forker three = {1}, four = {2};
    both(&three, &four, &a, &b);
    four.wait = 1;
    pthread_join(b, NULL);
    r = waitpid(-1, &status, 0);
    printf("the older child is left: %d\n", r == three.child && WEXITSTATUS(status) == 1);
    three.wait = -1;
    pthread_join(a, NULL);
    return 0;
}
