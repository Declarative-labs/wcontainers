// Timers whose signal is ignored or blocked when they fire: POSIX timers caught or blocked later,
// what the timers report while their signal waits, and ITIMER_REAL with SIGALRM ignored.
#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static volatile int handled;
static void on_info(int sig, siginfo_t *info, void *c) { (void)sig; (void)info; (void)c; handled++; }
static void on_sig(int sig) { (void)sig; handled++; }

static void sleep_ms(long ms) {
    struct timespec ts = {ms / 1000, (ms % 1000) * 1000000};
    while (nanosleep(&ts, &ts) && errno == EINTR) {}
}

static void set(int sig, int ignore) {
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    if (ignore) sa.sa_handler = SIG_IGN;
    else { sa.sa_sigaction = on_info; sa.sa_flags = SA_SIGINFO; }
    sigaction(sig, &sa, 0);
}

static void ignored_posix(void) {
    for (int periodic = 0; periodic < 2; periodic++) {
        int sig = SIGRTMIN + periodic;
        set(sig, 1);
        struct sigevent ev = {0};
        ev.sigev_notify = SIGEV_SIGNAL;
        ev.sigev_signo = sig;
        timer_t t;
        timer_create(CLOCK_MONOTONIC, &ev, &t);
        struct itimerspec it = {{0, periodic ? 2000000 : 0}, {0, 2000000}};
        timer_settime(t, 0, &it, 0);
        sleep_ms(30);
        struct itimerspec cur;
        timer_gettime(t, &cur);
        printf("periodic %d: armed while ignored %d, overrun %d\n", periodic,
               cur.it_value.tv_sec || cur.it_value.tv_nsec, timer_getoverrun(t));
        handled = 0;
        set(sig, 0);
        sleep_ms(30);
        printf("periodic %d: handled after catching %d\n", periodic, handled > 0);
        sigset_t s; sigemptyset(&s); sigaddset(&s, sig);
        sigprocmask(SIG_BLOCK, &s, 0);
        sleep_ms(30);
        sigset_t p; sigpending(&p);
        printf("periodic %d: pending while blocked %d\n", periodic, sigismember(&p, sig));
        timer_delete(t);
        sigprocmask(SIG_UNBLOCK, &s, 0);
    }

}
static void blocked(void) {
    sigset_t s; sigemptyset(&s); sigaddset(&s, SIGALRM); sigaddset(&s, SIGUSR1); sigaddset(&s, SIGUSR2);
    sigprocmask(SIG_BLOCK, &s, 0);
    struct itimerval it = {{0, 0}, {0, 2000}}, cur;
    setitimer(ITIMER_REAL, &it, 0);
    sleep_ms(20);
    getitimer(ITIMER_REAL, &cur);
    printf("one-shot itimer fired, blocked: value %ld.%06ld\n", (long)cur.it_value.tv_sec, (long)cur.it_value.tv_usec);
    struct itimerval per = {{0, 2000}, {0, 2000}};
    setitimer(ITIMER_REAL, &per, 0);
    sleep_ms(20);
    getitimer(ITIMER_REAL, &cur);
    printf("periodic itimer fired, blocked: value set %d\n", cur.it_value.tv_sec || cur.it_value.tv_usec);
    printf("alarm(0) while pending %u\n", alarm(0));
    struct sigevent ev = {0};
    ev.sigev_notify = SIGEV_SIGNAL;
    ev.sigev_signo = SIGUSR1;
    timer_t t;
    timer_create(CLOCK_MONOTONIC, &ev, &t);
    struct itimerspec one = {{0, 0}, {0, 2000000}}, pcur;
    timer_settime(t, 0, &one, 0);
    sleep_ms(20);
    timer_gettime(t, &pcur);
    printf("one-shot posix fired, blocked: value %ld.%09ld\n", (long)pcur.it_value.tv_sec, pcur.it_value.tv_nsec);
    ev.sigev_signo = SIGUSR2;
    timer_t t2;
    timer_create(CLOCK_MONOTONIC, &ev, &t2);
    struct itimerspec per2 = {{0, 2000000}, {0, 2000000}};
    timer_settime(t2, 0, &per2, 0);
    sleep_ms(20);
    timer_gettime(t2, &pcur);
    printf("periodic posix fired, blocked: value set %d, under 2ms %d\n", pcur.it_value.tv_sec || pcur.it_value.tv_nsec, pcur.it_value.tv_sec == 0 && pcur.it_value.tv_nsec <= 2000000);
    printf("overrun before taken %d\n", timer_getoverrun(t2));

}
static void ignored_alarm(void) {
    signal(SIGALRM, SIG_IGN);
    struct itimerval it = {{0, 2000}, {0, 2000}}, cur;
    setitimer(ITIMER_REAL, &it, 0);
    sleep_ms(30);
    getitimer(ITIMER_REAL, &cur);
    printf("itimer value set %d interval %ld\n", cur.it_value.tv_sec || cur.it_value.tv_usec, (long)cur.it_interval.tv_usec);
    signal(SIGALRM, on_sig);
    sleep_ms(30);
    printf("alarms after catching %d\n", handled > 0);
    struct itimerval zero = {{0, 0}, {0, 0}};
    setitimer(ITIMER_REAL, &zero, 0);

    // SIG_DFL whose default is to ignore (SIGCHLD, SIGURG, SIGWINCH) for a POSIX timer.
    struct sigevent ev = {0};
    ev.sigev_notify = SIGEV_SIGNAL;
    ev.sigev_signo = SIGWINCH;
    timer_t t;
    timer_create(CLOCK_MONOTONIC, &ev, &t);
    struct itimerspec pit = {{0, 2000000}, {0, 2000000}};
    timer_settime(t, 0, &pit, 0);
    sleep_ms(30);
    handled = 0;
    signal(SIGWINCH, on_sig);
    sleep_ms(30);
    printf("sigwinch timer after catching %d\n", handled > 0);

}

// Each part in a child of its own, so what one blocks or ignores does not reach the next.
static void apart(void (*part)(void)) {
    pid_t child = fork();
    if (child == 0) {
        part();
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("part ended %d\n", status);
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    apart(ignored_posix);
    apart(blocked);
    apart(ignored_alarm);
    return 0;
}
