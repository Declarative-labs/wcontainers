// What each thread holds for itself: its credentials and personality, changed by raw system calls in
// one thread (the C library would change every thread), and a message queue's notification request,
// which a close by any thread of the process takes away, even through a table of its own.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/capability.h>
#include <linux/futex.h>
#include <mqueue.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/personality.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <unistd.h>

#define THREAD (CLONE_VM | CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM | CLONE_CHILD_CLEARTID)

static char stack[1 << 16] __attribute__((aligned(16)));
static volatile int child_tid, step;
static volatile long seen[16];
static mqd_t queue;

static long raw(long number, long a, long b, long c) { return syscall(number, a, b, c); }

static long raw_clone(long flags, void *top, volatile int *ctid, int (*fn)(void)) {
    register long x0 __asm__("x0") = flags;
    register long x1 __asm__("x1") = (long)top;
    register long x2 __asm__("x2") = 0;
    register long x3 __asm__("x3") = 0;
    register long x4 __asm__("x4") = (long)ctid;
    register long x8 __asm__("x8") = SYS_clone;
    register long x9 __asm__("x9") = (long)fn;
    __asm__ volatile("svc #0\n"
                     "cbnz x0, 1f\n"
                     "blr x9\n"
                     "mov x8, #93\n"
                     "svc #0\n"
                     "1:\n"
                     : "+r"(x0)
                     : "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x8), "r"(x9)
                     : "memory", "x30");
    return x0;
}

static void wait_word(volatile int *at, int value) {
    while (*at == value) syscall(SYS_futex, at, FUTEX_WAIT, value, NULL);
}

static void wake_word(volatile int *at) { syscall(SYS_futex, at, FUTEX_WAKE, 1); }

static void join(void) {
    // Each wait is on the value just read, so a clear in between is not missed.
    int value;
    while ((value = child_tid) != 0) syscall(SYS_futex, &child_tid, FUTEX_WAIT, value, NULL);
}

static void start(int (*fn)(void), long flags) {
    child_tid = -1;
    if (raw_clone(flags, stack + sizeof stack, &child_tid, fn) < 0) {
        printf("clone failed\n");
        child_tid = 0;
    }
}

// Reads one line of a thread's status.
static void status_line(pid_t tid, const char *key) {
    char path[64], buffer[2048];
    snprintf(path, sizeof path, "/proc/self/task/%d/status", tid);
    int fd = raw(SYS_openat, AT_FDCWD, (long)path, O_RDONLY);
    long got = raw(SYS_read, fd, (long)buffer, sizeof buffer - 1);
    raw(SYS_close, fd, 0, 0);
    buffer[got > 0 ? got : 0] = 0;
    char *line = strstr(buffer, key);
    if (!line) {
        printf("  %s missing\n", key);
        return;
    }
    char *end = strchr(line, '\n');
    printf("  %.*s\n", (int)(end ? end - line : (long)strlen(line)), line);
}

static unsigned effective_caps(void) {
    struct __user_cap_header_struct header = {_LINUX_CAPABILITY_VERSION_3, 0};
    struct __user_cap_data_struct data[2];
    raw(SYS_capget, (long)&header, (long)data, 0);
    return data[0].effective;
}

// A thread that becomes another user, takes a personality, and waits to be looked at.
static int changes_itself(void) {
    seen[0] = raw(SYS_setresuid, 1000, 1000, 1000);
    seen[1] = raw(SYS_getuid, 0, 0, 0);
    seen[2] = effective_caps();
    seen[3] = raw(SYS_personality, UNAME26, 0, 0);
    seen[4] = raw(SYS_personality, 0xffffffff, 0, 0);
    seen[5] = raw(SYS_gettid, 0, 0, 0);
    seen[6] = raw(SYS_setresuid, 0, 0, 0) < 0 ? errno : 0;
    step = 1;
    wake_word(&step);
    wait_word(&step, 1);
    return 0;
}

// A thread made after the change sees the maker's own credentials.
static int looks(void) {
    seen[8] = raw(SYS_getuid, 0, 0, 0);
    seen[9] = raw(SYS_personality, 0xffffffff, 0, 0);
    return 0;
}

// A thread with its own descriptor table closes its copy of the queue.
static int closes_queue(void) {
    seen[10] = raw(SYS_close, queue, 0, 0);
    return 0;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    unsigned caps = effective_caps();
    start(changes_itself, THREAD | CLONE_FILES | CLONE_FS);
    wait_word(&step, 0);
    printf("thread: setresuid %ld uid %ld caps dropped %d personality %lx then %lx back to root %ld\n",
           seen[0], seen[1], seen[2] != caps, seen[3], seen[4], seen[6]);
    printf("first thread: uid %d euid %d caps kept %d personality %x\n", getuid(), geteuid(),
           effective_caps() == caps, personality(0xffffffff));
    printf("status of the thread:\n");
    status_line(seen[5], "Uid:");
    status_line(seen[5], "CapEff:");
    printf("status of the first thread:\n");
    status_line(getpid(), "Uid:");
    errno = 0;
    int priority = getpriority(PRIO_USER, 1000);
    printf("getpriority of user 1000: %d errno %d\n", priority, errno);
    step = 2;
    wake_word(&step);
    join();
    start(looks, THREAD | CLONE_FILES | CLONE_FS);
    join();
    printf("new thread: uid %ld personality %lx\n", seen[8], seen[9]);

    // A request to be told about a queue goes with a close by another thread of the process.
    struct mq_attr attr = {0, 4, 16, 0};
    queue = mq_open("/wc-threadcreds", O_RDWR | O_CREAT, 0600, &attr);
    struct sigevent event = {0};
    event.sigev_notify = SIGEV_SIGNAL;
    event.sigev_signo = SIGUSR1;
    printf("queue %d notify %d\n", queue >= 0, mq_notify(queue, &event));
    start(closes_queue, THREAD | CLONE_FS);
    join();
    errno = 0;
    printf("thread closed its copy %ld; notify again %d errno %d\n", seen[10],
           mq_notify(queue, &event), errno);
    mq_unlink("/wc-threadcreds");
    return 0;
}
