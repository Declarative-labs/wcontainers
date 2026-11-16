// Nice values and I/O priorities across processes: one process, a group with a running child, an
// ended child not yet reaped and a child of another user, and a user's processes; refusals for
// permission and for the nice limit, and the order a group's refusals come in.

#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

enum { WHO_PROCESS = 1, WHO_PGRP = 2, WHO_USER = 3 };
#define IOPRIO(class, level) ((class) << 13 | (level))

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: errno %d\n", what, errno);
    else
        printf("%s: %ld\n", what, result);
    errno = 0;
}

static long get(int which, int who) { return syscall(SYS_getpriority, which, who); }
static long set(int which, int who, int nice) { return syscall(SYS_setpriority, which, who, nice); }
static long ioget(int which, int who) { return syscall(SYS_ioprio_get, which, who); }
static long ioset(int which, int who, int prio) { return syscall(SYS_ioprio_set, which, who, prio); }

// A child that waits until its pipe closes, after taking the given user.
static pid_t waiting_child(int *write_end, int uid) {
    int ends[2], ready[2];
    pipe(ends);
    pipe(ready);
    pid_t pid = fork();
    char byte = 0;
    if (pid == 0) {
        close(ends[1]);
        if (uid && setresuid(uid, uid, uid) != 0)
            _exit(9);
        write(ready[1], &byte, 1);
        read(ends[0], &byte, 1);
        _exit(0);
    }
    close(ends[0]);
    close(ready[1]);
    read(ready[0], &byte, 1);
    close(ready[0]);
    *write_end = ends[1];
    return pid;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    setpgid(0, 0);
    show("self", get(PRIO_PROCESS, 0));
    show("self io", ioget(WHO_PROCESS, 0));

    int a_end;
    pid_t a = waiting_child(&a_end, 0);
    pid_t b = fork();
    if (b == 0)
        _exit(3);
    siginfo_t info;
    waitid(P_PID, b, &info, WEXITED | WNOWAIT);

    show("set a 5", set(PRIO_PROCESS, a, 5));
    show("get a", get(PRIO_PROCESS, a));
    show("get ended b", get(PRIO_PROCESS, b));
    show("get group", get(PRIO_PGRP, 0));
    show("set group 3", set(PRIO_PGRP, 0, 3));
    show("get ended b after group", get(PRIO_PROCESS, b));
    show("get a after group", get(PRIO_PROCESS, a));
    show("get self after group", get(PRIO_PROCESS, 0));
    show("lower a to 1", set(PRIO_PROCESS, a, 1));
    show("raise self to 7", set(PRIO_PROCESS, 0, 7));
    show("get group after", get(PRIO_PGRP, 0));
    show("get user 0", get(PRIO_USER, 0));
    show("get user nobody", get(PRIO_USER, 4242));
    show("get no group", get(PRIO_PGRP, 99999));
    show("get no process", get(PRIO_PROCESS, 99999));
    show("get negative", get(PRIO_PROCESS, -3));
    show("get which 3", get(3, 0));

    show("io set a idle", ioset(WHO_PROCESS, a, IOPRIO(3, 0)));
    show("io get a", ioget(WHO_PROCESS, a));
    show("io get ended b", ioget(WHO_PROCESS, b));
    show("io get group", ioget(WHO_PGRP, 0));
    show("io set group best 2", ioset(WHO_PGRP, 0, IOPRIO(2, 2)));
    show("io get group after", ioget(WHO_PGRP, 0));
    show("io get self", ioget(WHO_PROCESS, 0));
    show("io get ended b after", ioget(WHO_PROCESS, b));
    show("io get user", ioget(WHO_USER, 0));
    show("io set ended b", ioset(WHO_PROCESS, b, IOPRIO(3, 0)));
    show("io get ended b again", ioget(WHO_PROCESS, b));
    show("io set none", ioset(WHO_PROCESS, 0, 0));
    show("io get group none", ioget(WHO_PGRP, 0));
    show("io get user nobody", ioget(WHO_USER, 4242));
    show("io get which 0", ioget(0, 0));
    show("io get which 4", ioget(4, 0));

    // A child of another user: refusals, alone and within the group.
    int c_end;
    pid_t c = waiting_child(&c_end, 1000);
    show("set c", set(PRIO_PROCESS, c, 10));
    show("get c", get(PRIO_PROCESS, c));
    show("get user 1000", get(PRIO_USER, 1000));
    show("set user 1000", set(PRIO_USER, 1000, 10));
    show("set group 12", set(PRIO_PGRP, 0, 12));
    show("get a after refused group", get(PRIO_PROCESS, a));
    show("get self after refused group", get(PRIO_PROCESS, 0));
    show("io set c", ioset(WHO_PROCESS, c, IOPRIO(2, 5)));
    show("io set group with c", ioset(WHO_PGRP, 0, IOPRIO(2, 6)));
    show("io get a after", ioget(WHO_PROCESS, a));
    show("io get self after", ioget(WHO_PROCESS, 0));
    show("io get user 1000", ioget(WHO_USER, 1000));

    // A forked child keeps the nice value and I/O priority it was made with.
    ioset(WHO_PROCESS, 0, IOPRIO(2, 1));
    pid_t d = fork();
    if (d == 0) {
        printf("child nice %ld io %ld\n", get(PRIO_PROCESS, 0), ioget(WHO_PROCESS, 0));
        _exit(0);
    }
    waitpid(d, NULL, 0);

    close(a_end);
    close(c_end);
    waitpid(a, NULL, 0);
    waitpid(c, NULL, 0);
    waitpid(b, NULL, 0);
    show("get reaped b", get(PRIO_PROCESS, b));
    show("io get reaped b", ioget(WHO_PROCESS, b));
    return 0;
}
