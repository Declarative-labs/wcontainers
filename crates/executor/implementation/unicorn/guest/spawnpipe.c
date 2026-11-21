// A child started as Go starts one (clone with CLONE_VM, CLONE_VFORK and CLONE_PIDFD, its
// output pipe moved to its standard output), read to the end with edge-triggered epoll.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static int pipe_ends[2];
static int gate[2];
static char stack[65536];

static int child(void *argument) {
    (void)argument;
    dup3(gate[0], 0, 0);
    dup3(pipe_ends[1], 1, 0);
    char *arguments[] = {"/bin/sh", "-c", "echo from the child; read go; exit 2", NULL};
    execve("/bin/sh", arguments, NULL);
    _exit(127);
}

static void wait_for(int ep, const char *what) {
    struct epoll_event got;
    int ready = epoll_wait(ep, &got, 1, 10000);
    printf("%s: %d events %#x\n", what, ready, ready > 0 ? got.events : 0);
}

static void read_once(const char *what) {
    char buffer[100];
    long n = read(pipe_ends[0], buffer, sizeof buffer);
    if (n > 0)
        printf("%s: %.*s", what, (int)n, buffer);
    else
        printf("%s: %ld %s\n", what, n, n < 0 ? strerror(errno) : "end");
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    pipe2(pipe_ends, O_CLOEXEC | O_NONBLOCK);
    pipe2(gate, O_CLOEXEC);
    int ep = epoll_create1(EPOLL_CLOEXEC);
    struct epoll_event event = {.events = EPOLLIN | EPOLLOUT | EPOLLRDHUP | EPOLLET, .data.fd = pipe_ends[0]};
    epoll_ctl(ep, EPOLL_CTL_ADD, pipe_ends[0], &event);
    int pidfd = -1;
    pid_t pid = clone(child, stack + sizeof stack, CLONE_VM | CLONE_VFORK | CLONE_PIDFD | SIGCHLD, NULL,
                      &pidfd);
    printf("started %d, pidfd %d\n", pid > 0, pidfd >= 0);
    close(pipe_ends[1]);
    close(gate[0]);
    wait_for(ep, "the child wrote");
    read_once("read");
    read_once("read again");
    write(gate[1], "\n", 1);
    wait_for(ep, "the child ended");
    read_once("read at the end");
    siginfo_t info;
    printf("waitid: %ld\n", syscall(SYS_waitid, P_PIDFD, pidfd, &info, WEXITED, NULL));
    printf("status %d\n", info.si_status);
    return 0;
}
