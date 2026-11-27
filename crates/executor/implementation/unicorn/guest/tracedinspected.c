// A child traces its parent with PTRACE_ATTACH while the parent yields, and then lets it go with
// PTRACE_DETACH. The parent yields at three points for the embedder to look: before the child,
// while the child traces it, and after the child let it go.
#define _GNU_SOURCE
#include <sched.h>
#include <signal.h>
#include <sys/ptrace.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static long pt(long request, long pid) {
    return syscall(SYS_ptrace, request, pid, 0, 0);
}

int main(void) {
    int attached[2], done[2];
    if (pipe(attached) || pipe(done))
        return 1;
    pid_t parent = getpid();
    sched_yield();
    pid_t child = fork();
    if (child == 0) {
        int status;
        char byte = 'f';
        if (pt(PTRACE_ATTACH, parent) == 0 && waitpid(parent, &status, 0) == parent &&
            pt(PTRACE_CONT, parent) == 0)
            byte = 'a';
        write(attached[1], &byte, 1);
        read(done[0], &byte, 1);
        // It must be stopped to be let go of.
        kill(parent, SIGUSR1);
        byte = 'f';
        if (waitpid(parent, &status, 0) == parent && pt(PTRACE_DETACH, parent) == 0)
            byte = 'd';
        write(attached[1], &byte, 1);
        _exit(0);
    }
    char byte;
    read(attached[0], &byte, 1);
    if (byte != 'a')
        return 2;
    sched_yield();
    write(done[1], "d", 1);
    read(attached[0], &byte, 1);
    if (byte != 'd')
        return 3;
    sched_yield();
    int status;
    waitpid(child, &status, 0);
    return 0;
}
