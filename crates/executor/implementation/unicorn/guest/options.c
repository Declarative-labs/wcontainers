// fcntl's commands besides locks and duplicates, and prctl's options one process uses: owners and
// their signals, write hints, leases, queries, the parent's death signal, timer slack, the child
// subreaper, transparent huge pages, the saved auxiliary vector and the forms that need powers.
// Process numbers are shown only as whether they equal the caller's.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <linux/capability.h>
#include <sys/prctl.h>
#include <sys/select.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#define R(expr)                                                                                    \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long r_ = (long)(expr);                                                                    \
        printf("%-64s = %ld (%s)\n", #expr, r_, r_ < 0 ? strerror(errno) : "-");                 \
    } while (0)

#ifndef F_DUPFD_QUERY
#define F_DUPFD_QUERY 1027
#endif
#ifndef F_CREATED_QUERY
#define F_CREATED_QUERY 1028
#endif
#ifndef PR_GET_AUXV
#define PR_GET_AUXV 0x41555856
#endif
#ifndef PR_GET_MDWE
#define PR_GET_MDWE 66
#endif

static void owner(int fd) {
    struct f_owner_ex ex = {-1, -1};
    long r = fcntl(fd, F_GETOWN_EX, &ex);
    long own = fcntl(fd, F_GETOWN);
    printf("owner: getown %s, ex %ld type %d pid %s\n",
           own == 0 ? "0" : own == getpid() ? "self" : own == -getpgrp() ? "-group" : "other", r,
           ex.type,
           ex.pid == 0 ? "0" : ex.pid == getpid() ? "self" : ex.pid == getpgrp() ? "group" : "other");
    unsigned ids[2] = {7, 7};
    r = fcntl(fd, F_GETOWNER_UIDS, ids);
    printf("owner uids %ld %u %u\n", r, ids[0], ids[1]);
}

int main(void) {
    unlink("/tmp/o");
    int fd = open("/tmp/o", O_RDWR | O_CREAT, 0644);
    int again = open("/tmp/o", O_RDWR | O_CREAT, 0644);
    int path = open("/tmp/o", O_PATH);
    int copy = dup(fd);

    R(fcntl(fd, F_CREATED_QUERY));
    R(fcntl(again, F_CREATED_QUERY));
    R(fcntl(path, F_CREATED_QUERY));
    R(fcntl(fd, F_DUPFD_QUERY, copy));
    R(fcntl(fd, F_DUPFD_QUERY, again));
    R(fcntl(path, F_DUPFD_QUERY, path));
    R(fcntl(fd, F_DUPFD_QUERY, 999));

    owner(fd);
    R(fcntl(fd, F_GETSIG));
    R(fcntl(fd, F_SETOWN, getpid()) == 0);
    owner(fd);
    R(fcntl(fd, F_SETOWN, -getpgrp()) == 0);
    owner(fd);
    R(fcntl(fd, F_SETOWN, 0));
    owner(fd);
    R(fcntl(fd, F_SETOWN, 99999));
    R(fcntl(fd, F_SETOWN, -99999));
    R(fcntl(fd, F_SETOWN, (int)0x80000000));
    owner(copy);
    struct f_owner_ex ex = {F_OWNER_TID, gettid()};
    R(fcntl(fd, F_SETOWN_EX, &ex));
    owner(fd);
    ex.type = F_OWNER_PGRP;
    ex.pid = getpgrp();
    R(fcntl(fd, F_SETOWN_EX, &ex));
    owner(fd);
    ex.type = 3;
    R(fcntl(fd, F_SETOWN_EX, &ex));
    ex.type = F_OWNER_PID;
    ex.pid = 99999;
    R(fcntl(fd, F_SETOWN_EX, &ex));
    ex.pid = 0;
    R(fcntl(fd, F_SETOWN_EX, &ex));
    owner(fd);
    R(fcntl(fd, F_SETOWN_EX, (void *)8));
    R(fcntl(fd, F_GETOWN_EX, (void *)8));
    R(fcntl(fd, F_GETOWNER_UIDS, (void *)8));
    R(fcntl(fd, F_SETSIG, 10));
    R(fcntl(fd, F_GETSIG));
    R(fcntl(copy, F_GETSIG));
    R(fcntl(again, F_GETSIG));
    R(fcntl(fd, F_SETSIG, 64));
    R(fcntl(fd, F_SETSIG, 65));
    R(fcntl(fd, F_SETSIG, -1));
    R(fcntl(fd, F_GETSIG));
    R(fcntl(fd, F_SETSIG, 0));
    R(fcntl(path, F_SETOWN, 0));
    R(fcntl(path, F_GETSIG));

    R(fcntl(fd, F_GETPIPE_SZ));
    R(fcntl(fd, F_SETPIPE_SZ, 4096));
    R(fcntl(path, F_GETPIPE_SZ));
    R(fcntl(fd, F_GETLEASE));
    R(fcntl(path, F_GETLEASE));

    unsigned long long hint = 99;
    R(fcntl(fd, F_GET_RW_HINT, &hint));
    printf("hint %llu\n", hint);
    hint = 3;
    R(fcntl(fd, F_SET_RW_HINT, &hint));
    hint = 99;
    R(fcntl(again, F_GET_RW_HINT, &hint));
    printf("hint %llu\n", hint);
    hint = 6;
    R(fcntl(fd, F_SET_RW_HINT, &hint));
    R(fcntl(fd, F_SET_RW_HINT, (void *)8));
    R(fcntl(fd, F_GET_RW_HINT, (void *)8));
    R(fcntl(path, F_GET_RW_HINT, &hint));
    int devnull = open("/dev/null", O_RDONLY);
    hint = 5;
    R(fcntl(devnull, F_SET_RW_HINT, &hint));
    R(fcntl(devnull, F_GET_RW_HINT, &hint));
    printf("hint %llu\n", hint);

    int signal = -1;
    R(prctl(PR_GET_PDEATHSIG, &signal));
    printf("death signal %d\n", signal);
    R(prctl(PR_SET_PDEATHSIG, 10));
    R(prctl(PR_GET_PDEATHSIG, &signal));
    printf("death signal %d\n", signal);
    R(prctl(PR_SET_PDEATHSIG, 65));
    R(prctl(PR_SET_PDEATHSIG, 64));
    R(prctl(PR_GET_PDEATHSIG, (void *)8));
    R(prctl(PR_GET_TIMERSLACK));
    R(prctl(PR_SET_TIMERSLACK, 1000));
    R(prctl(PR_GET_TIMERSLACK));
    R(prctl(PR_SET_TIMERSLACK, 0));
    R(prctl(PR_GET_TIMERSLACK));
    // Ambient powers: asked, raised only when permitted and inheritable, lowered, cleared.
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, 40, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, 41, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, CAP_CHOWN, 1, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, CAP_SYS_ADMIN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, 9, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 0, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 1, 0, 0));
    {
        struct __user_cap_header_struct header = {_LINUX_CAPABILITY_VERSION_3, 0};
        struct __user_cap_data_struct data[2];
        syscall(SYS_capget, &header, data);
        data[0].inheritable |= 1 << CAP_CHOWN | 1 << CAP_KILL;
        R(syscall(SYS_capset, &header, data));
    }
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, CAP_KILL, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_LOWER, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, CAP_CHOWN, 0, 0));
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, CAP_KILL, 0, 0));
    {
        // Dropping an inheritable power drops it from the ambient set.
        struct __user_cap_header_struct header = {_LINUX_CAPABILITY_VERSION_3, 0};
        struct __user_cap_data_struct data[2];
        syscall(SYS_capget, &header, data);
        data[0].inheritable &= ~(1 << CAP_KILL);
        R(syscall(SYS_capset, &header, data));
    }
    R(prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, CAP_KILL, 0, 0));
    // A sleep's timer may end as late as the slack after it; where in the slack it ends depends on
    // other timers' interrupts, so only that it lasted no longer than the slack is shown.
    R(prctl(PR_SET_TIMERSLACK, 2000000000));
    for (int kind = 0; kind < 4; kind++) {
        struct timespec start, end, short_sleep = {0, 1000};
        clock_gettime(CLOCK_MONOTONIC, &start);
        if (kind == 0) {
            R(nanosleep(&short_sleep, 0));
        } else if (kind == 1) {
            R(clock_nanosleep(CLOCK_REALTIME, 0, &short_sleep, 0));
        } else if (kind == 2) {
            R(ppoll(0, 0, &short_sleep, 0));
        } else {
            R(select(0, 0, 0, 0, &(struct timeval){0, 1}));
        }
        clock_gettime(CLOCK_MONOTONIC, &end);
        long waited = (end.tv_sec - start.tv_sec) * 1000 + (end.tv_nsec - start.tv_nsec) / 1000000;
        printf("sleep %d ended within the slack: %d\n", kind, waited <= 3000);
    }
    R(prctl(PR_SET_TIMERSLACK, 0));
    int reaper = -1;
    R(prctl(PR_GET_CHILD_SUBREAPER, &reaper));
    printf("subreaper %d\n", reaper);
    R(prctl(PR_SET_CHILD_SUBREAPER, 7));
    R(prctl(PR_GET_CHILD_SUBREAPER, &reaper));
    printf("subreaper %d\n", reaper);
    R(prctl(PR_SET_CHILD_SUBREAPER, 0));
    R(prctl(PR_GET_CHILD_SUBREAPER, (void *)8));
    R(prctl(PR_GET_THP_DISABLE, 0, 0, 0, 0));
    R(prctl(PR_GET_THP_DISABLE, 1, 0, 0, 0));
    R(prctl(PR_SET_THP_DISABLE, 1, 0, 0, 0));
    R(prctl(PR_GET_THP_DISABLE, 0, 0, 0, 0));
    R(prctl(PR_SET_THP_DISABLE, 1, 2, 0, 0));
    R(prctl(PR_GET_THP_DISABLE, 0, 0, 0, 0));
    R(prctl(PR_SET_THP_DISABLE, 0, 2, 0, 0));
    R(prctl(PR_SET_THP_DISABLE, 1, 4, 0, 0));
    R(prctl(PR_SET_THP_DISABLE, 1, 0, 1, 0));
    R(prctl(PR_SET_THP_DISABLE, 0, 0, 0, 0));
    R(prctl(PR_GET_THP_DISABLE, 0, 0, 0, 0));
    R(prctl(PR_GET_MDWE, 0, 0, 0, 0));
    R(prctl(PR_GET_MDWE, 1, 0, 0, 0));

    unsigned long auxv[64];
    memset(auxv, 0xff, sizeof(auxv));
    R(prctl(PR_GET_AUXV, auxv, 0, 0, 0));
    R(prctl(PR_GET_AUXV, auxv, sizeof(auxv), 0, 0));
    // The vector is the one `/proc/self/auxv` shows, then zeros to the size kept, then untouched.
    unsigned char shown[512] = {0};
    int file = open("/proc/self/auxv", O_RDONLY);
    long length = read(file, shown, sizeof(shown));
    printf("auxv as /proc/self/auxv shows it, %s: %d\n", length > 0 ? "read" : "unread",
           memcmp(auxv, shown, 432) == 0 && auxv[54] == ~0UL);
    R(prctl(PR_GET_AUXV, auxv, 16, 1, 0));
    R(prctl(PR_GET_AUXV, (void *)8, 16, 0, 0));
    R(prctl(PR_GET_AUXV, (void *)8, 0, 0, 0));

    R(prctl(PR_GET_SECUREBITS));
    R(prctl(PR_SET_KEEPCAPS, 1));
    R(prctl(PR_GET_SECUREBITS));
    R(prctl(PR_SET_KEEPCAPS, 0));
    unsigned int size = 0;
    R(prctl(PR_SET_MM, PR_SET_MM_MAP_SIZE, &size, 0, 0));
    printf("map size %u\n", size);
    R(prctl(PR_SET_MM, PR_SET_MM_MAP_SIZE, (void *)8, 0, 0));
    R(prctl(PR_SET_MM, PR_SET_MM_START_CODE, 0x10000, 0, 0));
    R(prctl(PR_SET_MM, PR_SET_MM_START_CODE, 0x10000, 1, 0));
    R(prctl(PR_SET_MM, PR_SET_MM_START_CODE, 0x10000, 0, 1));
    R(prctl(PR_SET_MM, PR_SET_MM_AUXV, 0x10000, 1, 0));
    R(prctl(PR_SET_MM, 99, 0, 0, 0));
    R(prctl(PR_SET_IO_FLUSHER, 1, 0, 0, 0));
    R(prctl(PR_GET_IO_FLUSHER, 0, 0, 0, 0));

    // A change of the effective user clears the parent's death signal.
    R(prctl(PR_SET_PDEATHSIG, 9));
    R(setresuid(-1, 0, -1));
    R(prctl(PR_GET_PDEATHSIG, &signal));
    printf("death signal %d\n", signal);
    R(setresuid(-1, 1000, -1));
    R(prctl(PR_GET_PDEATHSIG, &signal));
    printf("death signal %d\n", signal);
    return 0;
}
