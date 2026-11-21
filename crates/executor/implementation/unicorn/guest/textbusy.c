// A program a process runs is not opened for writing or truncated (ETXTBSY), and a file open for
// writing is not run; once the process is gone both go.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <stdlib.h>
#include <unistd.h>

static void p(const char *what, int r) { printf("%s: %d %s\n", what, r < 0 ? -1 : 0, r < 0 ? strerror(errno) : ""); }

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc > 2) {
        // Run: says so through the descriptor it was given, then waits.
        write(atoi(argv[2]), "r", 1);
        pause();
        return 0;
    }
    // A copy of this program, run by a child.
    int in = open("/proc/self/exe", O_RDONLY), out = open("/tmp/textbusy", O_CREAT | O_WRONLY | O_TRUNC, 0755);
    char buffer[65536];
    long n;
    while ((n = read(in, buffer, sizeof buffer)) > 0) write(out, buffer, n);
    close(in);
    errno = 0;
    int ready[2];
    pipe(ready);
    char number[16];
    snprintf(number, sizeof number, "%d", ready[1]);
    char *args[] = {"/tmp/textbusy", "x", number, NULL};
    p("run while open for writing", execve("/tmp/textbusy", args, NULL));
    close(out);
    pid_t child = fork();
    if (child == 0) {
        execve("/tmp/textbusy", args, NULL);
        _exit(1);
    }
    char got;
    read(ready[0], &got, 1);
    p("open for writing while it runs", open("/tmp/textbusy", O_WRONLY));
    p("open for reading and writing", open("/tmp/textbusy", O_RDWR));
    p("open for reading with truncation", open("/tmp/textbusy", O_RDONLY | O_TRUNC));
    p("open for reading", open("/tmp/textbusy", O_RDONLY));
    p("truncate", truncate("/tmp/textbusy", 0));
    p("creat", creat("/tmp/textbusy", 0755));
    kill(child, SIGKILL);
    waitpid(child, NULL, 0);
    while (wait(NULL) > 0)
        ;
    p("open for writing once it is gone", open("/tmp/textbusy", O_WRONLY));
    unlink("/tmp/textbusy");
    return 0;
}
