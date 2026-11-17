// Sleeps on CPU clocks, which only time used by other threads or processes ends: the process's
// clock, another thread's and another process's, relative and absolute, interrupted by a handled
// signal and by a stop and continue.

#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static volatile int stop;

static void *spin(void *arg) {
    (void)arg;
    while (!stop) {
    }
    return NULL;
}

static long long ns(struct timespec t) { return t.tv_sec * 1000000000LL + t.tv_nsec; }

static struct timespec later(clockid_t clock, long nanoseconds) {
    struct timespec t;
    clock_gettime(clock, &t);
    t.tv_nsec += nanoseconds;
    t.tv_sec += t.tv_nsec / 1000000000;
    t.tv_nsec %= 1000000000;
    return t;
}

static void on_alarm(int signal) { (void)signal; }

// A process whose main thread sleeps on the process's clock while another thread computes; it
// tells `ready` just before the sleep.
static void sleeper_process(int ready) {
    pthread_t th;
    pthread_create(&th, NULL, spin, NULL);
    struct timespec t = {0, 100000000};
    write(ready, "x", 1);
    int r = clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, 0, &t, NULL);
    struct timespec used;
    clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &used);
    printf("stopped and continued sleep = %d used at least 0.1 s %d\n", r, ns(used) >= 100000000);
    fflush(stdout);
    _exit(0);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    pthread_t th;
    pthread_create(&th, NULL, spin, NULL);
    clockid_t other;
    pthread_getcpuclockid(th, &other);

    struct timespec ten = {0, 10000000};
    struct timespec start, end;
    clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &start);
    int r = clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, 0, &ten, NULL);
    clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &end);
    printf("relative sleep on process clock = %d reached %d\n", r, ns(end) - ns(start) >= 10000000);

    struct timespec target = later(CLOCK_PROCESS_CPUTIME_ID, 10000000);
    r = clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, TIMER_ABSTIME, &target, NULL);
    clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &end);
    printf("absolute sleep on process clock = %d reached %d\n", r, ns(end) >= ns(target));

    target = later(other, 10000000);
    r = clock_nanosleep(other, TIMER_ABSTIME, &target, NULL);
    clock_gettime(other, &end);
    printf("absolute sleep on other thread clock = %d reached %d\n", r, ns(end) >= ns(target));

    clock_gettime(other, &start);
    r = clock_nanosleep(other, 0, &ten, NULL);
    clock_gettime(other, &end);
    printf("relative sleep on other thread clock = %d reached %d\n", r, ns(end) - ns(start) >= 10000000);

    struct timespec past = {0, 1};
    printf("absolute sleep on a passed time = %d\n", clock_nanosleep(other, TIMER_ABSTIME, &past, NULL));
    struct timespec zero = {0, 0};
    printf("relative sleep of zero = %d\n", clock_nanosleep(other, 0, &zero, NULL));

    stop = 1;
    pthread_join(th, NULL);
    stop = 0;

    // Another process's clock.
    pid_t child = fork();
    if (child == 0) {
        spin(NULL);
        _exit(0);
    }
    clockid_t theirs;
    printf("clock_getcpuclockid = %d\n", clock_getcpuclockid(child, &theirs));
    clock_gettime(theirs, &start);
    r = clock_nanosleep(theirs, 0, &ten, NULL);
    clock_gettime(theirs, &end);
    printf("relative sleep on another process clock = %d reached %d\n", r,
           ns(end) - ns(start) >= 10000000);
    kill(child, SIGKILL);
    waitpid(child, NULL, 0);

    // With no other thread the process's clock stands still: only the signal ends the sleep.
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = on_alarm;
    sigaction(SIGALRM, &action, NULL);
    struct itimerval soon = {{0, 0}, {0, 20000}};
    setitimer(ITIMER_REAL, &soon, NULL);
    struct timespec second = {1, 0};
    struct timespec left = {7, 7};
    r = clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, 0, &second, &left);
    printf("relative sleep interrupted = %d left in (0.9 s, 1 s] %d\n", r,
           ns(left) > 900000000 && ns(left) <= 1000000000);
    setitimer(ITIMER_REAL, &soon, NULL);
    target = later(CLOCK_PROCESS_CPUTIME_ID, 1000000000);
    left = (struct timespec){7, 7};
    r = clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, TIMER_ABSTIME, &target, &left);
    printf("absolute sleep interrupted = %d left untouched %d\n", r, left.tv_sec == 7 && left.tv_nsec == 7);
    left = (struct timespec){7, 7};
    setitimer(ITIMER_REAL, &soon, NULL);
    r = clock_nanosleep(CLOCK_PROCESS_CPUTIME_ID, 0, &second, (struct timespec *)8);
    printf("relative sleep interrupted with a bad address = %d\n", r);

    // A stop and continue restarts the sleep, which then ends when the other thread has used the
    // time.
    int pipe_ends[2];
    pipe(pipe_ends);
    child = fork();
    if (child == 0) {
        close(pipe_ends[0]);
        sleeper_process(pipe_ends[1]);
    }
    close(pipe_ends[1]);
    char byte;
    read(pipe_ends[0], &byte, 1);
    struct timespec moment = {0, 5000000};
    nanosleep(&moment, NULL);
    kill(child, SIGSTOP);
    int status;
    waitpid(child, &status, WUNTRACED);
    printf("child stopped %d\n", WIFSTOPPED(status));
    kill(child, SIGCONT);
    waitpid(child, &status, 0);
    printf("child exited %d status %d\n", WIFEXITED(status), WEXITSTATUS(status));
    return 0;
}
