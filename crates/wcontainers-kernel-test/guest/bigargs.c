// Starting programs with as many page-long arguments as an unlimited stack takes: a search for the
// most a child's execve accepts, then starts at and around that count. Each started program prints
// its argument count and the length of its last argument.

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

static char **arguments;
static char *argument;

// Starts this program again with `count` arguments in a child; 1 when it started and exited 0.
static int start(long count) {
    arguments[0] = "bigargs";
    arguments[1] = "started";
    for (long at = 2; at < count; at++) arguments[at] = argument;
    arguments[count] = 0;
    pid_t child = fork();
    if (child == 0) {
        char *environment[] = {0};
        execve("/proc/self/exe", arguments, environment);
        printf("%ld arguments: errno %d (%s)\n", count, errno, strerror(errno));
        _exit(1);
    }
    int status;
    waitpid(child, &status, 0);
    printf("%ld arguments: status %#x\n", count, status);
    return WIFEXITED(status) && WEXITSTATUS(status) == 0;
}

int main(int argc, char **argv) {
    setvbuf(stdout, 0, _IONBF, 0);
    if (argc > 1 && strcmp(argv[1], "started") == 0) {
        printf("started with %d arguments, the last %zu bytes long\n", argc,
               strlen(argv[argc - 1]));
        return 0;
    }
    struct rlimit unlimited = {RLIM_INFINITY, RLIM_INFINITY};
    setrlimit(RLIMIT_STACK, &unlimited);
    long page = sysconf(_SC_PAGESIZE);
    long most = 2 * sysconf(_SC_ARG_MAX);
    arguments = calloc(most + 1, sizeof *arguments);
    argument = malloc(page);
    memset(argument, 'a', page - 1);
    argument[page - 1] = 0;
    long low = 2, high = most;
    while (high - low > 1) {
        long middle = (low + high) / 2;
        if (start(middle))
            low = middle;
        else
            high = middle;
    }
    printf("most: %ld\n", low);
    for (long count = low - 2; count <= low + 1; count++) start(count);
    return 0;
}
