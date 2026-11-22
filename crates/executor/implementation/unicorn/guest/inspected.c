// A child holds its parent's memory open through /proc/<pid>/mem while the parent yields, and then
// closes it; the parent then opens its own. The parent yields at four points for the embedder to
// look: before the child, while the child holds the file, after it closed it, and while the parent
// holds its own.
#define _GNU_SOURCE
#include <fcntl.h>
#include <sched.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    int opened[2], done[2];
    if (pipe(opened) || pipe(done))
        return 1;
    pid_t parent = getpid();
    sched_yield();
    pid_t child = fork();
    if (child == 0) {
        char path[64];
        snprintf(path, sizeof path, "/proc/%d/mem", parent);
        int fd = open(path, O_RDONLY);
        char byte = fd >= 0 ? 'o' : 'f';
        write(opened[1], &byte, 1);
        read(done[0], &byte, 1);
        close(fd);
        write(opened[1], "c", 1);
        _exit(0);
    }
    char byte;
    read(opened[0], &byte, 1);
    if (byte != 'o')
        return 2;
    sched_yield();
    write(done[1], "d", 1);
    read(opened[0], &byte, 1);
    sched_yield();
    int status;
    waitpid(child, &status, 0);
    int own = open("/proc/self/mem", O_RDONLY);
    if (own < 0)
        return 3;
    sched_yield();
    close(own);
    return 0;
}
