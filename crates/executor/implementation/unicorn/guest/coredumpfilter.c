// /proc/<pid>/coredump_filter: its value, numbers as written in each base, refusals, and what a
// child and a new program keep.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static void show(const char *what) {
    char buffer[32] = {0};
    int fd = open("/proc/self/coredump_filter", O_RDONLY);
    long n = read(fd, buffer, sizeof buffer - 1);
    close(fd);
    printf("%s: %ld %s", what, n, buffer);
}

static void put(const char *text, size_t length) {
    int fd = open("/proc/self/coredump_filter", O_WRONLY);
    long n = write(fd, text, length);
    printf("write %zu bytes: %ld %s\n", length, n, n < 0 ? strerror(errno) : "");
    close(fd);
    show(" now");
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc > 1) {
        show("after exec");
        return 0;
    }
    show("at start");
    put("0x1ff\n", 6);
    put("0x3ff", 5);
    put("0777", 4);
    put("+5", 2);
    put("12", 2);
    put("abc", 3);
    put("0x", 2);
    put("5 ", 2);
    put("4294967296", 10);
    put("4294967295", 10);
    put("7\0junk", 6);
    put("00000000000000000000000000017", 29);
    put("33", 2);
    pid_t child = fork();
    if (child == 0) {
        show("in a child");
        execl("/proc/self/exe", "coredumpfilter", "again", (char *)NULL);
        _exit(1);
    }
    waitpid(child, NULL, 0);
    return 0;
}
