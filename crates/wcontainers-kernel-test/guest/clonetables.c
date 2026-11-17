// clone's sharing flags: threads that keep their own descriptor table, working directory, umask or
// semaphore undo list, and processes that share the caller's descriptor table or working directory.
// The children make only system calls and leave what they saw for the parent to print.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/futex.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/ipc.h>
#include <sys/sem.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#define THREAD (CLONE_VM | CLONE_SIGHAND | CLONE_THREAD | CLONE_CHILD_CLEARTID | CLONE_CHILD_SETTID)

static char stack[1 << 16] __attribute__((aligned(16)));
static volatile int child_tid;
static long seen[8];
static int fd_a, semid;

static long raw(long number, long a, long b, long c) { return syscall(number, a, b, c); }

// Waits for a thread made with CLONE_CHILD_CLEARTID to end.
static void join(void) {
    while (child_tid != 0) {
        syscall(SYS_futex, &child_tid, FUTEX_WAIT, child_tid, NULL);
    }
}

// clone as the system call makes it (musl's wrapper refuses threads): the child runs `fn` on the
// stack, then ends.
static long raw_clone(long flags, void *top, volatile int *ctid, int (*fn)(void *)) {
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
                     "mov x0, #0\n"
                     "mov x8, #93\n"
                     "svc #0\n"
                     "1:\n"
                     : "+r"(x0)
                     : "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x8), "r"(x9)
                     : "memory", "x30");
    return x0;
}

static int start(int (*fn)(void *), int flags) {
    child_tid = -1;
    long made = raw_clone(flags, stack + sizeof stack, &child_tid, fn);
    if (made < 0) {
        printf("clone: errno %ld\n", -made);
        child_tid = 0;
    }
    return (int)made;
}

static char *cwd(char *buffer) {
    long got = raw(SYS_getcwd, (long)buffer, 256, 0);
    return got < 0 ? "?" : buffer;
}

// A thread with its own table: it closes one of the caller's descriptors and opens another.
static int own_table(void *argument) {
    (void)argument;
    seen[0] = raw(SYS_close, fd_a, 0, 0);
    seen[1] = raw(SYS_openat, AT_FDCWD, (long)"/dev/null", O_RDONLY);
    seen[2] = raw(SYS_fcntl, 0, F_GETFD, 0);
    return 0;
}

// The same with the table shared.
static int shared_table(void *argument) {
    (void)argument;
    seen[0] = raw(SYS_close, fd_a, 0, 0);
    seen[1] = raw(SYS_openat, AT_FDCWD, (long)"/dev/zero", O_RDONLY);
    return 0;
}

// Changes directory and umask.
static int moves(void *argument) {
    (void)argument;
    seen[0] = raw(SYS_chdir, (long)"/tmp", 0, 0);
    seen[1] = raw(SYS_umask, 077, 0, 0);
    return 0;
}

// Takes one from a semaphore with SEM_UNDO.
static int takes(void *argument) {
    (void)argument;
    struct sembuf op = {0, -1, SEM_UNDO};
    seen[0] = raw(SYS_semop, semid, (long)&op, 1);
    seen[1] = raw(SYS_semctl, semid, 0, GETVAL);
    return 0;
}

// Sleeps a little, then notes that it ran.
static int naps(void *argument) {
    (void)argument;
    struct timespec pause = {0, 20 * 1000 * 1000};
    raw(SYS_nanosleep, (long)&pause, 0, 0);
    seen[0] = 1;
    return 0;
}

static int descriptor_open(int fd) { return fcntl(fd, F_GETFD) >= 0; }

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    char buffer[256];

    // A thread without CLONE_FILES.
    fd_a = open("/dev/null", O_RDONLY);
    int last = open("/dev/null", O_RDONLY);
    close(last);
    printf("own table clone %d\n", start(own_table, THREAD | CLONE_FS | CLONE_SYSVSEM) > 0);
    join();
    printf("thread closed %ld opened the next %d sees 0 %ld\n", seen[0], seen[1] == fd_a,
           seen[2]);
    printf("caller still has it %d\n", descriptor_open(fd_a));

    // A thread with CLONE_FILES sees the same table.
    printf("shared table clone %d\n", start(shared_table, THREAD | CLONE_FILES | CLONE_FS) > 0);
    join();
    printf("thread closed %ld opened the next %d\n", seen[0], seen[1] == fd_a);
    char link[64] = {0};
    snprintf(buffer, sizeof buffer, "/proc/self/fd/%d", fd_a);
    readlink(buffer, link, sizeof link - 1);
    printf("caller sees %s\n", link);
    close(fd_a);

    // A thread without CLONE_FS.
    umask(022);
    printf("own directory clone %d\n", start(moves, THREAD | CLONE_FILES) > 0);
    join();
    printf("thread chdir %ld old umask %lo; caller in %s umask %o\n", seen[0], seen[1], cwd(buffer),
           umask(022));

    // A thread with CLONE_FS moves the caller too.
    printf("shared directory clone %d\n", start(moves, THREAD | CLONE_FILES | CLONE_FS) > 0);
    join();
    printf("thread chdir %ld; caller in %s umask %o\n", seen[0], cwd(buffer), umask(022));
    chdir("/");

    // A process with CLONE_FILES and CLONE_FS.
    fd_a = open("/dev/null", O_RDONLY);
    pid_t child = clone(shared_table, stack + sizeof stack, CLONE_FILES | SIGCHLD, NULL);
    int status;
    waitpid(child, &status, 0);
    memset(link, 0, sizeof link);
    snprintf(buffer, sizeof buffer, "/proc/self/fd/%d", fd_a);
    readlink(buffer, link, sizeof link - 1);
    printf("process sharing the table: caller sees %s\n", link);
    close(fd_a);
    child = clone(moves, stack + sizeof stack, CLONE_FS | SIGCHLD, NULL);
    waitpid(child, &status, 0);
    printf("process sharing the directory: caller in %s umask %o\n", cwd(buffer), umask(022));
    chdir("/");
    child = clone(moves, stack + sizeof stack, SIGCHLD, NULL);
    waitpid(child, &status, 0);
    printf("process with its own: caller in %s umask %o\n", cwd(buffer), umask(022));

    // A thread without CLONE_SYSVSEM undoes its adjustments when it ends; with it, they stay
    // until the process ends.
    semid = semget(IPC_PRIVATE, 1, 0600);
    semctl(semid, 0, SETVAL, 5);
    start(takes, THREAD | CLONE_FILES | CLONE_FS);
    join();
    printf("own undo list: took %ld saw %ld now %d\n", seen[0], seen[1], semctl(semid, 0, GETVAL));
    start(takes, THREAD | CLONE_FILES | CLONE_FS | CLONE_SYSVSEM);
    join();
    printf("shared undo list: took %ld saw %ld now %d\n", seen[0], seen[1],
           semctl(semid, 0, GETVAL));
    semctl(semid, 0, IPC_RMID);

    // A thread made with CLONE_VFORK: the caller waits until it ends.
    seen[0] = 0;
    long made = start(naps, THREAD | CLONE_FILES | CLONE_FS | CLONE_VFORK);
    printf("vfork thread %d: ran %ld before clone returned, number cleared %d\n", made > 0, seen[0],
           child_tid == 0);
    join();

    // Combinations refused.
    printf("thread without sighand %ld\n",
           syscall(SYS_clone, CLONE_VM | CLONE_THREAD, stack + sizeof stack, 0, 0, 0) < 0 ? errno
                                                                                          : 0L);
    printf("sighand without vm %ld\n",
           syscall(SYS_clone, CLONE_SIGHAND | SIGCHLD, stack + sizeof stack, 0, 0, 0) < 0 ? errno
                                                                                          : 0L);
    return 0;
}
