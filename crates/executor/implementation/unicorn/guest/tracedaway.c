// A child stopped for its tracer is taken off the kernel and put back by the embedder (at the
// parent's getppid, whose first argument names the child): it stays stopped until its tracer
// resumes it.
#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/ptrace.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, 0, _IOLBF, 0);
    pid_t child = fork();
    if (child == 0) {
        syscall(SYS_ptrace, PTRACE_TRACEME, 0, 0, 0);
        kill(getpid(), SIGUSR1);
        _exit(3);
    }
    int status = 0;
    waitpid(child, &status, 0);
    printf("stop: %s %d\n", WIFSTOPPED(status) ? "stopped" : "other", WSTOPSIG(status));
    syscall(SYS_getppid, child);
    usleep(200000);
    pid_t got = waitpid(child, &status, WNOHANG);
    printf("after it was away: %s\n", got == 0 ? "nothing" : "changed");
    char path[64], line[256];
    snprintf(path, sizeof path, "/proc/%d/status", child);
    FILE *file = fopen(path, "r");
    while (file && fgets(line, sizeof line, file))
        if (strncmp(line, "State:", 6) == 0)
            fputs(line, stdout);
    if (file)
        fclose(file);
    syscall(SYS_ptrace, PTRACE_CONT, child, 0, 0);
    waitpid(child, &status, 0);
    printf("end: %s %d\n", WIFEXITED(status) ? "exited" : "other", WEXITSTATUS(status));
    return 0;
}
