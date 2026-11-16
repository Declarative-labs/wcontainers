// Who runs while another waits on one CPU: a parent that spins with sched_yield until its child has
// run, one that yields until a child is seen asleep, and a child that only makes system calls beside
// another that must still get its turn.

#define _GNU_SOURCE
#include <fcntl.h>
#include <sched.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

static char state(pid_t pid) {
    char path[64], buf[256];
    snprintf(path, sizeof path, "/proc/%d/stat", pid);
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        return '-';
    }
    int n = read(fd, buf, sizeof buf - 1);
    close(fd);
    buf[n > 0 ? n : 0] = 0;
    char *end = strrchr(buf, ')');
    return end ? end[2] : '?';
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    volatile int *shared = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);

    // A yield lets a child that waits for its turn run.
    pid_t child = fork();
    if (child == 0) {
        shared[0] = 1;
        _exit(0);
    }
    long i;
    for (i = 0; i < 1000000 && !shared[0]; i++) {
        sched_yield();
    }
    printf("child ran within 100 yields: %d, at all: %d\n", i < 100, shared[0]);
    waitpid(child, NULL, 0);

    // Yields until a child that blocks is seen asleep.
    int fds[2];
    pipe(fds);
    child = fork();
    if (child == 0) {
        char byte;
        read(fds[0], &byte, 1);
        _exit(0);
    }
    char seen = 0;
    for (i = 0; i < 20000 && (seen = state(child)) != 'S'; i++) {
        sched_yield();
    }
    printf("child seen asleep: %c\n", seen);
    write(fds[1], "x", 1);
    waitpid(child, NULL, 0);

    // A child that only makes system calls does not keep the CPU from another.
    pid_t busy = fork();
    if (busy == 0) {
        for (long n = 0; n < 100000000 && !shared[1]; n++) {
            getppid();
        }
        _exit(shared[1] ? 0 : 1);
    }
    child = fork();
    if (child == 0) {
        _exit(7);
    }
    int status;
    waitpid(child, &status, 0);
    shared[1] = 1;
    printf("the other child ended with %d\n", WEXITSTATUS(status));
    waitpid(busy, &status, 0);
    printf("the busy child stopped when told: %d\n", WEXITSTATUS(status) == 0);
    return 0;
}
