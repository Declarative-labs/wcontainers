// get_robust_list of itself, of no such thread, and of another process as another user, which
// needs the right to read it (`ptrace_may_access` with the real ids).
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s: %ld %s\n", what, r, r < 0 ? strerror(errno) : ""); }

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    void *head;
    size_t length;
    p("itself", syscall(SYS_get_robust_list, 0, &head, &length));
    printf("length %zu\n", length);
    p("no such thread", syscall(SYS_get_robust_list, 999999, &head, &length));
    pid_t parent = getpid();
    pid_t child = fork();
    if (child == 0) {
        p("parent as root", syscall(SYS_get_robust_list, parent, &head, &length));
        if (setresuid(1, 1, 1)) return 1;
        p("parent as user 1", syscall(SYS_get_robust_list, parent, &head, &length));
        p("itself as user 1", syscall(SYS_get_robust_list, 0, &head, &length));
        return 0;
    }
    waitpid(child, NULL, 0);
    return 0;
}
