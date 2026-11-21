// Timers of every kind: interval timers on the real and CPU clocks, POSIX timers with every
// notification and their overruns, and timer descriptors. Times are printed only as how they relate
// to what was asked, since both machines are busy.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <sys/timerfd.h>
#include <time.h>
#include <unistd.h>

#ifndef TFD_IOC_SET_TICKS
#define TFD_IOC_SET_TICKS _IOW('T', 0, uint64_t)
#endif

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

static volatile int alarms, profiles, virtuals, usr1s;
static volatile int last_code, last_value, last_timer, last_overrun;
static volatile pid_t handled_by;

static void on_signal(int signal, siginfo_t *info, void *context) {
    (void)context;
    if (signal == SIGALRM)
        alarms++;
    if (signal == SIGPROF)
        profiles++;
    if (signal == SIGVTALRM)
        virtuals++;
    if (signal == SIGUSR1)
        usr1s++;
    last_code = info->si_code;
    last_value = info->si_value.sival_int;
    last_timer = info->si_timerid;
    last_overrun = info->si_overrun;
    handled_by = gettid();
}

static long timer_create_raw(clockid_t clock, struct sigevent *event, int *id) {
    return syscall(SYS_timer_create, clock, event, id);
}

static int counted_to(volatile int *counter, int target, long most) {
    for (long i = 0; i < most && *counter < target; i++)
        __asm__ volatile("" ::: "memory");
    return *counter >= target;
}

static volatile int thread_ready;
static pid_t thread_id;

static void *wait_in_thread(void *arg) {
    (void)arg;
    thread_id = gettid();
    sigset_t unblock;
    sigemptyset(&unblock);
    sigaddset(&unblock, SIGUSR1);
    pthread_sigmask(SIG_UNBLOCK, &unblock, NULL);
    thread_ready = 1;
    while (usr1s == 0)
        usleep(1000);
    return NULL;
}

// A POSIX timer takes its signal's place in the user's RLIMIT_SIGPENDING count when it is made:
// none left is EAGAIN (and no timer number used), and a timer made before always sends its signal,
// with its information, however full the count is.
static void pending_limit(void) {
    struct rlimit saved, none = {0, 0};
    getrlimit(RLIMIT_SIGPENDING, &saved);
    sigset_t rt;
    sigemptyset(&rt);
    sigaddset(&rt, SIGRTMIN + 1);
    sigprocmask(SIG_BLOCK, &rt, NULL);
    struct sigevent event = {.sigev_notify = SIGEV_SIGNAL, .sigev_signo = SIGRTMIN + 1};
    timer_t made, refused, after;
    p("timer_create before the limit", timer_create(CLOCK_MONOTONIC, &event, &made));
    setrlimit(RLIMIT_SIGPENDING, &none);
    p("timer_create at a limit of 0", timer_create(CLOCK_MONOTONIC, &event, &refused));
    p("timer_create at a limit of 0, bad address", syscall(SYS_timer_create, CLOCK_MONOTONIC, NULL, (void *)8));
    struct itimerspec soon = {.it_value = {0, 1000000}};
    timer_settime(made, 0, &soon, NULL);
    siginfo_t info;
    int signal = sigwaitinfo(&rt, &info);
    printf("its signal came at the limit: %d code %d\n", signal == SIGRTMIN + 1, info.si_code);
    setrlimit(RLIMIT_SIGPENDING, &saved);
    timer_create(CLOCK_MONOTONIC, &event, &after);
    printf("the refused one took no number: %d\n", (long)after - (long)made == 1);
    timer_delete(made);
    timer_delete(after);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    struct sigaction action = {.sa_sigaction = on_signal, .sa_flags = SA_SIGINFO | SA_RESTART};
    sigaction(SIGALRM, &action, NULL);
    sigaction(SIGPROF, &action, NULL);
    sigaction(SIGVTALRM, &action, NULL);
    sigaction(SIGUSR1, &action, NULL);
    sigaction(SIGUSR2, &action, NULL);

    // Interval timers.
    struct itimerval value = {{0, 0}, {0, 20000}}, old;
    p("setitimer bad which", setitimer(3, &value, NULL));
    struct itimerval bad = {{0, 0}, {0, 1000000}};
    p("setitimer bad usec", setitimer(ITIMER_REAL, &bad, NULL));
    p("getitimer bad which", getitimer(5, &old));
    p("setitimer real", setitimer(ITIMER_REAL, &value, &old));
    printf("old value was zero %d\n", old.it_value.tv_sec == 0 && old.it_value.tv_usec == 0);
    getitimer(ITIMER_REAL, &old);
    printf("left within 20 ms %d\n", old.it_value.tv_sec == 0 && old.it_value.tv_usec <= 20000);
    while (alarms == 0)
        pause();
    printf("alarm came, code %d\n", last_code);
    getitimer(ITIMER_REAL, &old);
    printf("one-shot now disarmed %d\n", old.it_value.tv_sec == 0 && old.it_value.tv_usec == 0);
    struct itimerval periodic = {{0, 5000}, {0, 5000}};
    setitimer(ITIMER_REAL, &periodic, NULL);
    while (alarms < 4)
        pause();
    getitimer(ITIMER_REAL, &old);
    printf("periodic interval kept %ld\n", (long)old.it_interval.tv_usec);
    p("setitimer null disarms", setitimer(ITIMER_REAL, NULL, &old));
    getitimer(ITIMER_REAL, &old);
    printf("disarmed %d\n", old.it_value.tv_usec == 0 && old.it_interval.tv_usec == 0);
    printf("alarm(5) %u alarm(0) %u\n", alarm(5), alarm(0));
    alarm(3);
    printf("alarm left rounds up %u\n", alarm(0));
    struct itimerval cpu = {{0, 10000}, {0, 10000}};
    setitimer(ITIMER_PROF, &cpu, NULL);
    printf("profiling timer fired twice while computing %d\n", counted_to(&profiles, 2, 4000000000L));
    setitimer(ITIMER_PROF, NULL, NULL);
    setitimer(ITIMER_VIRTUAL, &cpu, NULL);
    printf("virtual timer fired while computing %d\n", counted_to(&virtuals, 1, 4000000000L));
    setitimer(ITIMER_VIRTUAL, NULL, NULL);

    // POSIX timers: their checks and numbers.
    int id = -1, id2 = -1;
    struct sigevent event = {.sigev_notify = SIGEV_SIGNAL, .sigev_signo = SIGUSR2};
    p("timer_create bad clock", timer_create_raw(99, NULL, &id));
    p("timer_create raw clock", timer_create_raw(CLOCK_MONOTONIC_RAW, NULL, &id));
    p("timer_create coarse clock", timer_create_raw(CLOCK_MONOTONIC_COARSE, NULL, &id));
    event.sigev_signo = 0;
    p("timer_create no signal", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    event.sigev_signo = 65;
    p("timer_create signal 65", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    event.sigev_signo = SIGUSR2;
    event.sigev_notify = SIGEV_THREAD_ID;
    p("timer_create thread id alone", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    event.sigev_notify = SIGEV_SIGNAL | SIGEV_THREAD_ID;
    event.sigev_notify_thread_id = 99999;
    p("timer_create no such thread", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    event.sigev_notify = 7;
    p("timer_create bad notify", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    p("timer_create alarm clock", timer_create_raw(CLOCK_REALTIME_ALARM, NULL, &id));
    clockid_t no_process = (clockid_t)((~99999u << 3) | 2);
    p("timer_create gone process clock", timer_create_raw(no_process, NULL, &id));
    p("timer_create bad address", timer_create_raw(CLOCK_MONOTONIC, NULL, (int *)8));
    p("timer_create default", timer_create_raw(CLOCK_MONOTONIC, NULL, &id));
    printf("timer number %d\n", id);
    struct itimerspec spec = {{0, 0}, {0, 10000000}}, before;
    p("timer_settime null", syscall(SYS_timer_settime, id, 0, NULL, NULL));
    p("timer_settime bad timer", syscall(SYS_timer_settime, 12345, 0, &spec, NULL));
    struct itimerspec bad_spec = {{0, 0}, {0, 1000000000}};
    p("timer_settime bad time", syscall(SYS_timer_settime, id, 0, &bad_spec, NULL));
    alarms = 0;
    p("timer_settime", syscall(SYS_timer_settime, id, 0, &spec, &before));
    printf("before disarmed %d\n", before.it_value.tv_sec == 0 && before.it_value.tv_nsec == 0);
    while (alarms == 0)
        pause();
    printf("default timer: SIGALRM code %d value is its number %d timer %d\n", last_code, last_value == id, last_timer == id);
    syscall(SYS_timer_gettime, id, &before);
    printf("one-shot after firing %ld %ld\n", (long)before.it_value.tv_nsec, (long)before.it_interval.tv_nsec);
    p("timer_gettime bad timer", syscall(SYS_timer_gettime, 777, &before));
    p("timer_getoverrun", syscall(SYS_timer_getoverrun, id));
    p("timer_delete", syscall(SYS_timer_delete, id));
    p("timer_delete again", syscall(SYS_timer_delete, id));

    // Silent timers count down; periodic ones go round.
    event.sigev_notify = SIGEV_NONE;
    p("timer_create none", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    printf("numbers go on %d\n", id);
    struct itimerspec silent = {{0, 2000000}, {0, 3000000}};
    syscall(SYS_timer_settime, id, 0, &silent, NULL);
    usleep(10000);
    syscall(SYS_timer_gettime, id, &before);
    printf("silent periodic: left within its interval %d interval %ld\n",
           before.it_value.tv_sec == 0 && before.it_value.tv_nsec > 0 && before.it_value.tv_nsec <= 3000000,
           (long)before.it_interval.tv_nsec);
    struct itimerspec once = {{0, 0}, {0, 1000000}};
    syscall(SYS_timer_settime, id, 0, &once, NULL);
    usleep(5000);
    syscall(SYS_timer_gettime, id, &before);
    printf("silent one-shot after its time %ld\n", (long)before.it_value.tv_nsec);
    syscall(SYS_timer_delete, id);

    // A value of its own on an absolute time of the wall clock.
    event.sigev_notify = SIGEV_SIGNAL;
    event.sigev_signo = SIGUSR2;
    event.sigev_value.sival_int = 4242;
    timer_create_raw(CLOCK_REALTIME, &event, &id);
    struct timespec now;
    clock_gettime(CLOCK_REALTIME, &now);
    now.tv_nsec += 10000000;
    if (now.tv_nsec >= 1000000000) {
        now.tv_sec++;
        now.tv_nsec -= 1000000000;
    }
    struct itimerspec absolute = {{0, 0}, now};
    last_value = 0;
    syscall(SYS_timer_settime, id, TIMER_ABSTIME, &absolute, NULL);
    while (last_value != 4242)
        pause();
    printf("absolute wall clock timer: code %d value %d\n", last_code, last_value);
    struct itimerspec past = {{0, 0}, {1, 0}};
    last_value = 0;
    syscall(SYS_timer_settime, id, TIMER_ABSTIME, &past, NULL);
    while (last_value != 4242)
        pause();
    printf("a time already past fires at once\n");

    // Overruns while the signal waits blocked.
    sigset_t block;
    sigemptyset(&block);
    sigaddset(&block, SIGUSR2);
    sigprocmask(SIG_BLOCK, &block, NULL);
    struct itimerspec fast = {{0, 1000000}, {0, 1000000}};
    syscall(SYS_timer_settime, id, 0, &fast, NULL);
    usleep(60000);
    last_overrun = -1;
    sigprocmask(SIG_UNBLOCK, &block, NULL);
    printf("overruns while blocked at least 20 %d, as getoverrun says %d\n", last_overrun >= 20,
           syscall(SYS_timer_getoverrun, id) == last_overrun);
    struct itimerspec off = {{0, 0}, {0, 0}};
    syscall(SYS_timer_settime, id, 0, &off, NULL);
    // A timer's signal waiting blocked goes with the timer.
    sigprocmask(SIG_BLOCK, &block, NULL);
    syscall(SYS_timer_settime, id, 0, &once, NULL);
    usleep(10000);
    sigset_t pending;
    sigpending(&pending);
    printf("signal waiting %d\n", sigismember(&pending, SIGUSR2));
    syscall(SYS_timer_delete, id);
    sigpending(&pending);
    printf("after delete still waiting %d\n", sigismember(&pending, SIGUSR2));
    last_value = 0;
    sigprocmask(SIG_UNBLOCK, &block, NULL);
    printf("a deleted timer's signal is not handled %d\n", last_value == 0);
    sigpending(&pending);
    printf("and no longer waits %d\n", !sigismember(&pending, SIGUSR2));

    // To one thread.
    sigemptyset(&block);
    sigaddset(&block, SIGUSR1);
    sigprocmask(SIG_BLOCK, &block, NULL);
    pthread_t thread;
    pthread_create(&thread, NULL, wait_in_thread, NULL);
    while (!thread_ready)
        usleep(1000);
    event.sigev_notify = SIGEV_SIGNAL | SIGEV_THREAD_ID;
    event.sigev_signo = SIGUSR1;
    event.sigev_notify_thread_id = thread_id;
    p("timer_create to a thread", timer_create_raw(CLOCK_MONOTONIC, &event, &id));
    syscall(SYS_timer_settime, id, 0, &spec, NULL);
    pthread_join(thread, NULL);
    printf("thread timer handled by its thread %d\n", handled_by == thread_id);
    syscall(SYS_timer_delete, id);

    // On the process's CPU clock.
    event.sigev_notify = SIGEV_SIGNAL;
    event.sigev_signo = SIGPROF;
    timer_create_raw(CLOCK_PROCESS_CPUTIME_ID, &event, &id);
    struct itimerspec cpu_once = {{0, 0}, {0, 20000000}};
    profiles = 0;
    syscall(SYS_timer_settime, id, 0, &cpu_once, NULL);
    printf("CPU time timer fired while computing %d code %d\n", counted_to(&profiles, 1, 4000000000L), last_code);
    syscall(SYS_timer_delete, id);
    timer_create_raw(CLOCK_THREAD_CPUTIME_ID, &event, &id2);
    profiles = 0;
    syscall(SYS_timer_settime, id2, 0, &cpu_once, NULL);
    printf("thread CPU time timer fired while computing %d\n", counted_to(&profiles, 1, 4000000000L));
    syscall(SYS_timer_delete, id2);

    // Timer descriptors.
    p("timerfd_create bad clock", timerfd_create(CLOCK_PROCESS_CPUTIME_ID, 0));
    p("timerfd_create bad flags", timerfd_create(CLOCK_MONOTONIC, 1));
    p("timerfd_create alarm", timerfd_create(CLOCK_BOOTTIME_ALARM, 0));
    int fd = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK | TFD_CLOEXEC);
    printf("timerfd made %d cloexec %d nonblock %d\n", fd >= 0, (fcntl(fd, F_GETFD) & FD_CLOEXEC) != 0,
           (fcntl(fd, F_GETFL) & O_NONBLOCK) != 0);
    uint64_t ticks = 0;
    p("read unarmed", read(fd, &ticks, 8));
    p("read short", read(fd, &ticks, 4));
    p("write", write(fd, &ticks, 8));
    p("settime bad flags", timerfd_settime(fd, 4, &spec, NULL));
    p("settime bad time", timerfd_settime(fd, 0, &bad_spec, NULL));
    p("settime not a timer", timerfd_settime(0, 0, &spec, NULL));
    p("gettime bad descriptor", timerfd_gettime(999, &before));
    int path_only = open("/", O_PATH);
    p("settime path only", timerfd_settime(path_only, 0, &spec, NULL));
    p("gettime path only", timerfd_gettime(path_only, &before));
    close(path_only);
    struct itimerspec every = {{0, 10000000}, {0, 10000000}};
    p("settime periodic", timerfd_settime(fd, 0, &every, NULL));
    struct pollfd poller = {fd, POLLIN, 0};
    p("poll before", poll(&poller, 1, 0));
    usleep(45000);
    p("poll after", poll(&poller, 1, 0));
    long r = read(fd, &ticks, 8);
    printf("read %ld ticks at least 3 %d\n", r, ticks >= 3);
    timerfd_gettime(fd, &before);
    printf("still periodic %ld within %d\n", (long)before.it_interval.tv_nsec, before.it_value.tv_nsec <= 10000000);
    uint64_t set = 7;
    p("set ticks", ioctl(fd, TFD_IOC_SET_TICKS, &set));
    set = 0;
    p("set no ticks", ioctl(fd, TFD_IOC_SET_TICKS, &set));
    r = read(fd, &ticks, 8);
    printf("read set ticks %ld %d\n", r, (int)(ticks >= 7));
    p("settime disarm", timerfd_settime(fd, 0, &off, &before));
    printf("old periodic interval %ld\n", (long)before.it_interval.tv_nsec);
    p("read disarmed", read(fd, &ticks, 8));
    clock_gettime(CLOCK_MONOTONIC, &now);
    struct itimerspec at_once = {{0, 0}, now};
    p("settime absolute past", timerfd_settime(fd, TFD_TIMER_ABSTIME, &at_once, NULL));
    p("poll for it", poll(&poller, 1, 1000));
    r = read(fd, &ticks, 8);
    printf("absolute past: read %ld ticks %llu\n", r, (unsigned long long)ticks);
    int waiting = timerfd_create(CLOCK_REALTIME, 0);
    clock_gettime(CLOCK_REALTIME, &now);
    now.tv_nsec += 20000000;
    if (now.tv_nsec >= 1000000000) {
        now.tv_sec++;
        now.tv_nsec -= 1000000000;
    }
    struct itimerspec wall = {{0, 0}, now};
    p("settime wall cancel on set", timerfd_settime(waiting, TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET, &wall, NULL));
    r = read(waiting, &ticks, 8);
    printf("blocking read %ld ticks %llu\n", r, (unsigned long long)ticks);
    p("lseek", lseek(waiting, 5, SEEK_SET));
    pending_limit();
    return 0;
}
