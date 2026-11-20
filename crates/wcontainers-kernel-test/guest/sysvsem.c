// System V semaphores: semget's keys, sizes and flags, semop's operations whole or not at all,
// waits and the order waiting operations finish in, waits for zero, counts of waiters, timeouts,
// signals and removal, undo at a process's end (shared with CLONE_SYSVSEM), limits, permissions and
// every semctl command. Times are printed as unset or set, and process numbers as the caller's, a
// child's or none.

#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

// Linux's own layouts for 64-bit ARM (`struct semid64_ds`, `struct seminfo`, `struct sembuf`).
struct perm64 {
    int key;
    unsigned uid, gid, cuid, cgid, mode;
    unsigned short seq, pad;
    unsigned long unused1, unused2;
};
struct semid64 {
    struct perm64 perm;
    long otime, ctime;
    unsigned long nsems, unused3, unused4;
};
struct seminfo {
    int semmap, semmni, semmns, semmnu, semmsl, semopm, semume, semusz, semvmx, semaem;
};
struct op {
    unsigned short num;
    short op, flags;
};

enum { RMID = 0, SET = 1, STAT = 2, INFO = 3 };
enum { GETPID = 11, GETVAL, GETALL, GETNCNT, GETZCNT, SETVAL, SETALL, SSTAT, SINFO, SSTAT_ANY };
enum { CREAT = 01000, EXCL = 02000, NOWAIT = 04000, UNDO = 0x1000 };

static pid_t self, child_pid;

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: errno %d\n", what, errno);
    else
        printf("%s: %ld\n", what, result);
    errno = 0;
}

static long get(long key, long count, long flags) { return syscall(SYS_semget, key, count, flags); }
static long ctl(int id, int num, int command, long argument) { return syscall(SYS_semctl, id, num, command, argument); }
static long ops(int id, struct op *list, long count) { return syscall(SYS_semop, id, list, count); }
static long one(int id, int num, int op, int flags) {
    struct op o = {num, op, flags};
    return ops(id, &o, 1);
}
static long two(int id, int n1, int o1, int f1, int n2, int o2, int f2) {
    struct op o[2] = {{n1, o1, f1}, {n2, o2, f2}};
    return ops(id, o, 2);
}
static long timed(int id, struct op *list, long count, long seconds, long nanoseconds) {
    struct timespec t = {seconds, nanoseconds};
    return syscall(SYS_semtimedop, id, list, count, &t);
}

static const char *who(long pid) {
    return pid == 0 ? "none" : pid == self ? "me" : pid == child_pid ? "child" : "other";
}

static void status(const char *what, int id) {
    struct semid64 s;
    memset(&s, 0, sizeof s);
    long r = ctl(id, 0, STAT, (long)&s);
    if (r < 0) {
        show(what, r);
        return;
    }
    printf("%s: key %d uid %u gid %u cuid %u cgid %u mode %o nsems %lu otime %s ctime %s\n", what, s.perm.key,
           s.perm.uid, s.perm.gid, s.perm.cuid, s.perm.cgid, s.perm.mode, s.nsems, s.otime ? "set" : "0",
           s.ctime ? "set" : "0");
}

static void values(const char *what, int id) {
    unsigned short v[8];
    memset(v, 0xff, sizeof v);
    struct semid64 s;
    ctl(id, 0, STAT, (long)&s);
    long r = ctl(id, 0, GETALL, (long)v);
    printf("%s: %ld values", what, r);
    for (unsigned long i = 0; i < s.nsems && i < 8; i++)
        printf(" %u(n%ld z%ld %s)", v[i], ctl(id, i, GETNCNT, 0), ctl(id, i, GETZCNT, 0), who(ctl(id, i, GETPID, 0)));
    printf("\n");
}

// A child that runs `list` once and exits with 0 or 100 plus the errno.
static pid_t waiter(int id, struct op *list, int count) {
    pid_t pid = fork();
    if (pid == 0) {
        long r = ops(id, list, count);
        _exit(r < 0 ? 100 + errno : 0);
    }
    usleep(100000);
    return pid;
}

// Whether each child has ended, and how, without waiting for those that have not.
static void ended(const char *what, pid_t *kids, int count) {
    usleep(100000);
    printf("%s:", what);
    for (int i = 0; i < count; i++) {
        if (kids[i] == 0) {
            printf(" -");
            continue;
        }
        int status;
        pid_t r = waitpid(kids[i], &status, WNOHANG);
        if (r == kids[i]) {
            printf(" %d", WEXITSTATUS(status));
            kids[i] = 0;
        } else {
            printf(" waiting");
        }
    }
    printf("\n");
}

static void basics(void) {
    int id = get(0x5345, 3, CREAT | 0640);
    show("make", id >= 0);
    show("same key", get(0x5345, 3, 0) == id);
    show("same key fewer", get(0x5345, 1, 0) == id);
    show("same key none", get(0x5345, 0, 0) == id);
    show("same key more", get(0x5345, 4, 0));
    show("same key exclusive", get(0x5345, 3, CREAT | EXCL));
    show("no key", get(0x5346, 1, 0));
    show("private none", get(0, 0, 0600));
    show("private negative", get(0, -1, 0600));
    show("private too many", get(0, 32001, 0600));
    int most = get(0, 32000, 0600);
    show("private most", most >= 0);
    ctl(most, 0, RMID, 0);
    show("private twice differ", get(0, 1, 0600) != get(0, 1, 0600));
    status("new", id);
    values("new", id);
    show("add 2", one(id, 0, 2, 0));
    status("after add", id);
    show("take 1", one(id, 0, -1, 0));
    show("take 2 nowait", one(id, 0, -2, NOWAIT));
    show("zero nowait", one(id, 0, 0, NOWAIT));
    show("zero of another", one(id, 1, 0, 0));
    show("two at once", two(id, 0, -1, 0, 1, 3, 0));
    values("after ops", id);
    show("whole or nothing", two(id, 1, -1, 0, 2, -1, NOWAIT));
    values("unchanged", id);
    show("no operations", ops(id, NULL, 0));
    struct op many[501];
    memset(many, 0, sizeof many);
    show("too many operations", ops(id, many, 501));
    show("most operations", ops(id, many, 500));
    show("beyond the set", one(id, 3, 1, 0));
    show("far beyond the set", one(id, 65535, 1, 0));
    show("bad buffer", ops(id, (struct op *)8, 1));
    show("negative id", one(-1, 0, 1, 0));
    show("wrong id", one(id + 32768, 0, 1, 0));
    show("too large", one(id, 1, 32767, 0));
    show("largest", ctl(id, 2, SETVAL, 32767));
    show("past largest", one(id, 2, 1, 0));
    // An operation naming a semaphore an earlier one changed sees the change.
    ctl(id, 0, SETVAL, 1);
    show("add then take more", two(id, 0, 1, 0, 0, -2, NOWAIT));
    show("take then add", two(id, 0, -1, NOWAIT, 0, 1, 0));
    show("zero then add", two(id, 0, 0, NOWAIT, 0, 1, 0));
    show("add then zero", two(id, 0, 1, 0, 0, 0, NOWAIT));
    values("after repeats", id);
    show("semtimedop bad time", timed(id, many, 1, 0, 1000000000));
    show("semtimedop negative time", timed(id, many, 1, -1, 0));
    show("semtimedop time fault", syscall(SYS_semtimedop, id, many, 1, (void *)8));
    show("semtimedop zero time", ({
        struct op o = {0, -9, 0};
        timed(id, &o, 1, 0, 0);
    }));
    show("semtimedop short time", ({
        struct op o = {0, -9, 0};
        timed(id, &o, 1, 0, 20000000);
    }));
    show("semtimedop done", timed(id, many, 1, 1, 0));
    show("remove", ctl(id, 0, RMID, 0));
    show("remove again", ctl(id, 0, RMID, 0));
    show("op on removed", one(id, 0, 1, 0));
}

static void commands(void) {
    int id = get(0, 4, 0600);
    unsigned short set[4] = {1, 2, 3, 4};
    show("set all", ctl(id, 0, SETALL, (long)set));
    values("after set all", id);
    unsigned short big[4] = {1, 32768, 3, 4};
    show("set all too large", ctl(id, 0, SETALL, (long)big));
    show("set all fault", ctl(id, 0, SETALL, 8));
    show("get all fault", ctl(id, 0, GETALL, 8));
    show("set value", ctl(id, 3, SETVAL, 9));
    show("set value negative", ctl(id, 3, SETVAL, -1));
    show("set value too large", ctl(id, 3, SETVAL, 32768));
    show("set value high bits", ctl(id, 3, SETVAL, 0x700000005L));
    show("set value bad number", ctl(id, 4, SETVAL, 1));
    show("set value negative number", ctl(id, -1, SETVAL, 1));
    show("get value", ctl(id, 3, GETVAL, 0));
    show("get value bad number", ctl(id, 4, GETVAL, 0));
    show("get value negative number", ctl(id, -1, GETVAL, 0));
    show("get pid", ctl(id, 3, GETPID, 0) == self);
    show("get bad id", ctl(id + 1, 0, GETVAL, 0));
    show("negative id info", ctl(-1, 0, INFO, 0));
    show("bad command", ctl(id, 0, 99, 0));
    show("with ipc_64", ctl(id, 0, STAT | 0x100, 0));
    show("stat fault", ctl(id, 0, STAT, 8));
    struct seminfo info;
    memset(&info, 0, sizeof info);
    long max = ctl(0, 0, INFO, (long)&info);
    printf("info: max index %d map %d mni %d mns %d mnu %d msl %d opm %d ume %d usz %d vmx %d aem %d\n", max >= 0,
           info.semmap, info.semmni, info.semmns, info.semmnu, info.semmsl, info.semopm, info.semume, info.semusz,
           info.semvmx, info.semaem);
    memset(&info, 0, sizeof info);
    max = ctl(0, 0, SINFO, (long)&info);
    printf("sem info: max index %d usz %d aem %d\n", max >= 0, info.semusz, info.semaem);
    show("info fault", ctl(0, 0, INFO, 8));
    struct semid64 s;
    long index = id & 0x7fff;
    show("stat by index is the id", ctl(index, 0, SSTAT, (long)&s) == id);
    show("stat any by index", ctl(index, 0, SSTAT_ANY, (long)&s) == id);
    show("stat unused index", ctl(30000, 0, SSTAT, (long)&s));
    ctl(id, 0, STAT, (long)&s);
    s.perm.mode = 0123444;
    s.perm.uid = 1000;
    show("set", ctl(id, 0, SET, (long)&s));
    status("after set", id);
    show("set value without permission", ctl(id, 0, SETVAL, 1));
    show("op without permission", one(id, 0, 1, 0));
    show("wait for zero with read permission", one(id, 0, 0, NOWAIT));
    s.perm.uid = (unsigned)-1;
    show("set bad uid", ctl(id, 0, SET, (long)&s));
    show("set fault", ctl(id, 0, SET, 8));
    ctl(id, 0, RMID, 0);
}

static void waits(void) {
    int id = get(0, 2, 0600);
    pid_t kids[3];

    // Simple decrements: the first in line that can go goes.
    struct op take2 = {0, -2, 0}, take1 = {0, -1, 0}, zero = {0, 0, 0};
    kids[0] = waiter(id, &take2, 1);
    kids[1] = waiter(id, &take1, 1);
    kids[2] = waiter(id, &zero, 1);
    values("three waiting", id);
    one(id, 0, 1, 0);
    ended("add 1", kids, 3);
    values("after add 1", id);
    one(id, 0, 2, 0);
    ended("add 2", kids, 3);
    values("after add 2", id);
    for (int i = 0; i < 3; i++)
        if (kids[i])
            waitpid(kids[i], NULL, 0);

    // An operation on several semaphores waits in the set's queue, and simple ones after it too.
    ctl(id, 0, SETVAL, 0);
    ctl(id, 1, SETVAL, 0);
    struct op both[2] = {{0, -1, 0}, {1, -1, 0}};
    struct op second = {0, -1, 0};
    struct op zeros[2] = {{0, 0, 0}, {1, 0, 0}};
    kids[0] = waiter(id, both, 2);
    kids[1] = waiter(id, &second, 1);
    ctl(id, 1, SETVAL, 1);
    kids[2] = waiter(id, zeros, 2);
    values("complex waiting", id);
    one(id, 0, 1, 0);
    ended("add to the first", kids, 3);
    values("after add to the first", id);
    one(id, 0, 1, 0);
    ended("add to the first again", kids, 3);
    values("after the complex one", id);
    for (int i = 0; i < 3; i++)
        if (kids[i])
            waitpid(kids[i], NULL, 0);

    // A wait whose operation that cannot go on asks not to wait ends at once; one waits.
    ctl(id, 0, SETVAL, 0);
    show("no wait on the blocking one", two(id, 1, 0, 0, 0, -1, NOWAIT));
    kids[0] = waiter(id, (struct op[]){{1, 0, NOWAIT}, {0, -1, 0}}, 2);
    values("nowait elsewhere waits", id);
    // Setting a value lets it go.
    ctl(id, 0, SETVAL, 1);
    kids[1] = kids[2] = 0;
    ended("after set value", kids, 3);
    values("after set value", id);

    // A timeout, a signal and a removal end waits.
    struct op takebig = {0, -5, 0};
    show("timed out", timed(id, &takebig, 1, 0, 50000000));
    values("after timeout", id);
    child_pid = fork();
    if (child_pid == 0) {
        struct sigaction handled = {0};
        handled.sa_handler = (void (*)(int))abs;
        handled.sa_flags = SA_RESTART;
        sigaction(SIGUSR1, &handled, NULL);
        long r = one(id, 0, -5, 0);
        printf("interrupted: %ld errno %d\n", r, r < 0 ? errno : 0);
        _exit(0);
    }
    usleep(100000);
    values("one waiting", id);
    kill(child_pid, SIGUSR1);
    waitpid(child_pid, NULL, 0);
    values("after the signal", id);
    child_pid = fork();
    if (child_pid == 0) {
        long r = one(id, 1, -5, 0);
        printf("removed while waiting: %ld errno %d\n", r, r < 0 ? errno : 0);
        _exit(0);
    }
    usleep(100000);
    ctl(id, 0, RMID, 0);
    waitpid(child_pid, NULL, 0);

    // A waiter killed leaves the queue.
    id = get(0, 1, 0600);
    child_pid = fork();
    if (child_pid == 0) {
        one(id, 0, -1, 0);
        _exit(0);
    }
    usleep(100000);
    values("before kill", id);
    kill(child_pid, SIGKILL);
    waitpid(child_pid, NULL, 0);
    values("after kill", id);
    // A stopped and continued waiter's call ends with EINTR.
    child_pid = fork();
    if (child_pid == 0) {
        long r = one(id, 0, -1, 0);
        printf("stopped and continued: %ld errno %d\n", r, r < 0 ? errno : 0);
        _exit(0);
    }
    usleep(100000);
    kill(child_pid, SIGSTOP);
    usleep(100000);
    kill(child_pid, SIGCONT);
    waitpid(child_pid, NULL, 0);
    ctl(id, 0, RMID, 0);
}

static void undo(void) {
    int id = get(0, 2, 0600);
    int ends[2];
    pipe(ends);
    char c;

    child_pid = fork();
    if (child_pid == 0) {
        one(id, 0, 3, UNDO);
        one(id, 1, 5, 0);
        one(id, 1, -2, UNDO);
        _exit(0);
    }
    waitpid(child_pid, NULL, 0);
    values("undone at exit", id);

    // The value is held to its range when undone.
    ctl(id, 0, SETVAL, 0);
    child_pid = fork();
    if (child_pid == 0) {
        one(id, 0, 10, UNDO);
        write(ends[1], "x", 1);
        pause();
        _exit(0);
    }
    read(ends[0], &c, 1);
    one(id, 0, -8, 0);
    values("before undo below zero", id);
    kill(child_pid, SIGKILL);
    waitpid(child_pid, NULL, 0);
    values("undone below zero", id);

    // Setting a value clears its adjustments.
    child_pid = fork();
    if (child_pid == 0) {
        one(id, 0, 4, UNDO);
        one(id, 1, 4, UNDO);
        write(ends[1], "x", 1);
        pause();
        _exit(0);
    }
    read(ends[0], &c, 1);
    ctl(id, 0, SETVAL, 6);
    kill(child_pid, SIGKILL);
    waitpid(child_pid, NULL, 0);
    values("set value cleared one", id);

    // The range of an adjustment.
    ctl(id, 0, SETVAL, 32767);
    child_pid = fork();
    if (child_pid == 0) {
        show("take most with undo", one(id, 0, -32767, UNDO));
        one(id, 0, 1, 0);
        show("adjustment out of range", one(id, 0, -1, UNDO | NOWAIT));
        show("plain take", one(id, 0, -1, NOWAIT));
        _exit(0);
    }
    waitpid(child_pid, NULL, 0);
    values("after the range", id);

    // An undo made for an operation that failed still sets the time at exit.
    int fresh = get(0, 1, 0600);
    status("fresh", fresh);
    child_pid = fork();
    if (child_pid == 0) {
        show("undo beyond the set", one(fresh, 5, -1, UNDO));
        _exit(0);
    }
    waitpid(child_pid, NULL, 0);
    status("after a failed undo", fresh);
    ctl(fresh, 0, RMID, 0);

    // A waiter woken by an undo.
    ctl(id, 0, SETVAL, 0);
    child_pid = fork();
    if (child_pid == 0) {
        one(id, 0, 2, UNDO);
        write(ends[1], "x", 1);
        pause();
        _exit(0);
    }
    read(ends[0], &c, 1);
    pid_t kids[1];
    kids[0] = waiter(id, (struct op[]){{0, 0, 0}}, 1);
    kill(child_pid, SIGKILL);
    waitpid(child_pid, NULL, 0);
    ended("waiter for zero after undo", kids, 1);

    // CLONE_SYSVSEM shares the undo list, which is undone when the last sharer ends.
    ctl(id, 0, SETVAL, 0);
    child_pid = fork();
    if (child_pid == 0) {
        pid_t inner = syscall(SYS_clone, 0x40000 | SIGCHLD, 0, 0, 0, 0);
        if (inner == 0) {
            one(id, 0, 7, UNDO);
            _exit(0);
        }
        waitpid(inner, NULL, 0);
        printf("after the sharer: %ld\n", ctl(id, 0, GETVAL, 0));
        _exit(0);
    }
    waitpid(child_pid, NULL, 0);
    values("after both sharers", id);

    // Removing a set drops its adjustments.
    child_pid = fork();
    if (child_pid == 0) {
        one(id, 0, 1, UNDO);
        write(ends[1], "x", 1);
        pause();
        _exit(0);
    }
    read(ends[0], &c, 1);
    ctl(id, 0, RMID, 0);
    int again = get(0, 2, 0600);
    kill(child_pid, SIGKILL);
    waitpid(child_pid, NULL, 0);
    values("new set after removal", again);
    ctl(again, 0, RMID, 0);
}

static void permissions(void) {
    int id = get(0x5347, 1, CREAT | 0600);
    child_pid = fork();
    if (child_pid == 0) {
        setresgid(1000, 1000, 1000);
        setresuid(1000, 1000, 1000);
        show("other user get", get(0x5347, 1, 0));
        show("other user get asking nothing", get(0x5347, 1, 0) >= 0);
        show("other user op", one(id, 0, 1, NOWAIT));
        show("other user wait for zero", one(id, 0, 0, NOWAIT));
        show("other user get value", ctl(id, 0, GETVAL, 0));
        show("other user set value", ctl(id, 0, SETVAL, 1));
        struct semid64 s;
        show("other user stat", ctl(id, 0, STAT, (long)&s));
        show("other user stat any", ctl(id & 0x7fff, 0, SSTAT_ANY, (long)&s) >= 0);
        show("other user remove", ctl(id, 0, RMID, 0));
        show("other user undo on a set it cannot use", one(id, 0, 1, UNDO));
        show("other user make", get(0x5348, 2, CREAT | 0600) >= 0);
        _exit(0);
    }
    waitpid(child_pid, NULL, 0);
    status("made by another user", get(0x5348, 0, 0));
    status("after another user's undo", id);
    ctl(get(0x5348, 0, 0), 0, RMID, 0);
    ctl(id, 0, RMID, 0);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    self = getpid();
    basics();
    commands();
    waits();
    undo();
    permissions();
    return 0;
}
