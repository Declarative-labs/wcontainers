// Lock waits between processes: record locks and flock locks a waiter gets when the holder lets
// go, changes or closes them or ends, waits ended by signals, and deadlocks refused.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static const char *path = "/dev/shm/lockwaits";

static int lock(int fd, int type, int command, off_t start, off_t length) {
    struct flock l;
    memset(&l, 0, sizeof l);
    l.l_type = type;
    l.l_whence = SEEK_SET;
    l.l_start = start;
    l.l_len = length;
    return fcntl(fd, command, &l);
}

static void nap(void) {
    struct timespec t = {0, 1000 * 1000};
    nanosleep(&t, NULL);
}

// Waits until a process is asleep in a call, as its stat file shows.
static void asleep(pid_t pid) {
    char name[64], text[256];
    snprintf(name, sizeof name, "/proc/%d/stat", pid);
    for (;;) {
        int fd = open(name, O_RDONLY);
        ssize_t got = read(fd, text, sizeof text - 1);
        close(fd);
        if (got > 0) {
            text[got] = 0;
            char *end = strrchr(text, ')');
            if (end && end[2] == 'S') return;
        }
        nap();
    }
}

static void reap(const char *who, pid_t pid) {
    int status;
    waitpid(pid, &status, 0);
    printf("  %s ended %d\n", who, WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status));
}

// A child that waits for a record lock on byte `start` and says what it got.
static pid_t record_waiter(const char *who, int type, int command, off_t start) {
    pid_t pid = fork();
    if (pid == 0) {
        int fd = open(path, O_RDWR);
        (void)who;
        _exit(lock(fd, type, command, start, 1) < 0 ? errno : 0);
    }
    return pid;
}

static void on_signal(int signal) { (void)signal; }

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int fd = open(path, O_RDWR | O_CREAT, 0600);
    ftruncate(fd, 16);

    printf("unlock wakes a writer\n");
    lock(fd, F_WRLCK, F_SETLK, 0, 1);
    pid_t b = record_waiter("b", F_WRLCK, F_SETLKW, 0);
    asleep(b);
    printf("  a unlock %d\n", lock(fd, F_UNLCK, F_SETLK, 0, 1));
    reap("b", b);

    printf("a change to read wakes two readers, not a writer\n");
    lock(fd, F_WRLCK, F_SETLK, 0, 4);
    b = record_waiter("b", F_RDLCK, F_SETLKW, 1);
    asleep(b);
    pid_t c = record_waiter("c", F_RDLCK, F_SETLKW, 2);
    asleep(c);
    printf("  a read lock %d\n", lock(fd, F_RDLCK, F_SETLK, 0, 4));
    reap("b", b);
    reap("c", c);
    pid_t d = record_waiter("d", F_WRLCK, F_SETLKW, 3);
    asleep(d);
    printf("  a unlock %d\n", lock(fd, F_UNLCK, F_SETLK, 0, 4));
    reap("d", d);

    printf("an unlock elsewhere leaves the waiter waiting\n");
    lock(fd, F_WRLCK, F_SETLK, 0, 8);
    b = record_waiter("b", F_WRLCK, F_SETLKW, 5);
    asleep(b);
    printf("  a unlock of 0 to 3 %d\n", lock(fd, F_UNLCK, F_SETLK, 0, 4));
    asleep(b);
    printf("  b still waits\n");
    printf("  a unlock of 4 to 7 %d\n", lock(fd, F_UNLCK, F_SETLK, 4, 4));
    reap("b", b);

    printf("closing another descriptor of the file wakes the waiter\n");
    lock(fd, F_WRLCK, F_SETLK, 0, 1);
    b = record_waiter("b", F_WRLCK, F_SETLKW, 0);
    asleep(b);
    int other = open(path, O_RDONLY);
    printf("  a close %d\n", close(other));
    reap("b", b);

    printf("the holder's end wakes the waiter\n");
    int ready[2];
    pipe(ready);
    pid_t holder = fork();
    if (holder == 0) {
        int own = open(path, O_RDWR);
        lock(own, F_WRLCK, F_SETLK, 0, 1);
        write(ready[1], "x", 1);
        struct timespec t = {0, 50 * 1000 * 1000};
        nanosleep(&t, NULL);
        _exit(3);
    }
    char byte;
    read(ready[0], &byte, 1);
    printf("  a waits %d\n", lock(fd, F_WRLCK, F_SETLKW, 0, 1));
    reap("holder", holder);
    lock(fd, F_UNLCK, F_SETLK, 0, 0);

    printf("two processes in a loop\n");
    lock(fd, F_WRLCK, F_SETLK, 0, 1);
    b = fork();
    if (b == 0) {
        int own = open(path, O_RDWR);
        printf("  b lock 1: %d\n", lock(own, F_WRLCK, F_SETLK, 1, 1));
        write(ready[1], "x", 1);
        _exit(lock(own, F_WRLCK, F_SETLKW, 0, 1) < 0 ? errno : 0);
    }
    read(ready[0], &byte, 1);
    asleep(b);
    int got = lock(fd, F_WRLCK, F_SETLKW, 1, 1);
    printf("  a wait for 1: %d errno %d\n", got, got < 0 ? errno : 0);
    got = lock(fd, F_WRLCK, F_SETLK, 1, 1);
    printf("  a try for 1: %d errno %d\n", got, got < 0 ? errno : 0);
    lock(fd, F_UNLCK, F_SETLK, 0, 1);
    reap("b", b);

    printf("three processes in a loop\n");
    lock(fd, F_WRLCK, F_SETLK, 0, 1);
    pid_t c2 = fork();
    if (c2 == 0) {
        int own = open(path, O_RDWR);
        lock(own, F_WRLCK, F_SETLK, 2, 1);
        write(ready[1], "x", 1);
        _exit(lock(own, F_WRLCK, F_SETLKW, 0, 1) < 0 ? errno : 0);
    }
    read(ready[0], &byte, 1);
    asleep(c2);
    b = fork();
    if (b == 0) {
        int own = open(path, O_RDWR);
        lock(own, F_WRLCK, F_SETLK, 1, 1);
        write(ready[1], "x", 1);
        _exit(lock(own, F_WRLCK, F_SETLKW, 2, 1) < 0 ? errno : 0);
    }
    read(ready[0], &byte, 1);
    asleep(b);
    got = lock(fd, F_WRLCK, F_SETLKW, 1, 1);
    printf("  a wait for 1: %d errno %d\n", got, got < 0 ? errno : 0);
    lock(fd, F_UNLCK, F_SETLK, 0, 1);
    reap("c", c2);
    reap("b", b);

    printf("a wait for an open file's lock is never a loop\n");
    int mine = open(path, O_RDWR);
    lock(mine, F_WRLCK, F_OFD_SETLK, 0, 1);
    b = fork();
    if (b == 0) {
        int own = open(path, O_RDWR);
        lock(own, F_WRLCK, F_SETLK, 1, 1);
        write(ready[1], "x", 1);
        _exit(lock(own, F_WRLCK, F_SETLKW, 0, 1) < 0 ? errno : 0);
    }
    read(ready[0], &byte, 1);
    asleep(b);
    pid_t e = fork();
    if (e == 0) {
        // The caller's table waits for b, which waits for an open file's lock.
        _exit(lock(fd, F_WRLCK, F_SETLKW, 1, 1) < 0 ? errno : 0);
    }
    asleep(e);
    printf("  open file unlock %d\n", lock(mine, F_UNLCK, F_OFD_SETLK, 0, 1));
    reap("b", b);
    reap("e", e);
    close(mine);
    lock(fd, F_UNLCK, F_SETLK, 0, 0);

    printf("an open file's wait woken by its unlock\n");
    mine = open(path, O_RDWR);
    lock(mine, F_RDLCK, F_OFD_SETLK, 0, 1);
    b = record_waiter("b", F_WRLCK, F_OFD_SETLKW, 0);
    asleep(b);
    printf("  a unlock %d\n", lock(mine, F_UNLCK, F_OFD_SETLK, 0, 1));
    reap("b", b);
    close(mine);

    printf("signals during a record lock wait\n");
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = on_signal;
    sigaction(SIGUSR1, &action, NULL);
    lock(fd, F_WRLCK, F_SETLK, 0, 1);
    b = record_waiter("b", F_WRLCK, F_SETLKW, 0);
    asleep(b);
    kill(b, SIGUSR1);
    reap("b", b);
    action.sa_flags = SA_RESTART;
    sigaction(SIGUSR1, &action, NULL);
    b = record_waiter("b", F_WRLCK, F_SETLKW, 0);
    asleep(b);
    kill(b, SIGUSR1);
    asleep(b);
    printf("  a unlock %d\n", lock(fd, F_UNLCK, F_SETLK, 0, 1));
    reap("b", b);

    printf("flock\n");
    int fa = open(path, O_RDWR);
    flock(fa, LOCK_EX);
    pid_t waiters[2];
    for (int i = 0; i < 2; i++) {
        waiters[i] = fork();
        if (waiters[i] == 0) {
            int own = open(path, O_RDWR);
            _exit(flock(own, LOCK_SH) < 0 ? errno : 0);
        }
        asleep(waiters[i]);
    }
    printf("  a to shared %d\n", flock(fa, LOCK_SH));
    reap("first", waiters[0]);
    reap("second", waiters[1]);
    b = fork();
    if (b == 0) {
        int own = open(path, O_RDWR);
        _exit(flock(own, LOCK_EX) < 0 ? errno : 0);
    }
    asleep(b);
    printf("  a unlock %d\n", flock(fa, LOCK_UN));
    reap("b", b);
    flock(fa, LOCK_EX);
    b = fork();
    if (b == 0) {
        // The inherited copy must go too, or the lock stays held.
        close(fa);
        int own = open(path, O_RDWR);
        _exit(flock(own, LOCK_EX) < 0 ? errno : 0);
    }
    asleep(b);
    printf("  a close %d\n", close(fa));
    reap("b", b);
    fa = open(path, O_RDWR);
    flock(fa, LOCK_EX);
    action.sa_flags = 0;
    sigaction(SIGUSR1, &action, NULL);
    b = fork();
    if (b == 0) {
        int own = open(path, O_RDWR);
        _exit(flock(own, LOCK_EX) < 0 ? errno : 0);
    }
    asleep(b);
    kill(b, SIGUSR1);
    reap("b", b);
    close(fa);
    unlink(path);
    return 0;
}
