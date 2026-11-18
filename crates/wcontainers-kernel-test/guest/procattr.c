// /proc/<pid>/attr with no security module: its files and modes, reads and writes refused, and who
// may write through which file.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

static void *thread(void *unused) {
    (void)unused;
    int fd = open("/proc/self/attr/current", O_WRONLY);
    result("thread write through the process", write(fd, "x", 1));
    char path[64];
    snprintf(path, sizeof path, "/proc/thread-self/attr/current");
    int own = open(path, O_WRONLY);
    result("thread write through its own", write(own, "x", 1));
    close(fd);
    close(own);
    return NULL;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    DIR *directory = opendir("/proc/self/attr");
    struct dirent *entry;
    while ((entry = readdir(directory)) != NULL) {
        struct stat status;
        char path[300];
        snprintf(path, sizeof path, "/proc/self/attr/%s", entry->d_name);
        lstat(path, &status);
        printf("%s %o\n", entry->d_name, status.st_mode);
    }
    closedir(directory);
    const char *names[] = {"current", "prev", "exec", "fscreate", "keycreate", "sockcreate"};
    char buffer[64];
    for (int i = 0; i < 6; i++) {
        char path[64];
        snprintf(path, sizeof path, "/proc/self/attr/%s", names[i]);
        int fd = open(path, O_RDONLY);
        result(names[i], read(fd, buffer, sizeof buffer));
        close(fd);
    }
    int fd = open("/proc/self/attr/fscreate", O_RDWR);
    result("write", write(fd, "\n", 1));
    result("write a bad buffer", write(fd, (void *)8, 3));
    result("pwrite a bad buffer past the start", pwrite(fd, (void *)8, 3, 1));
    result("seek", lseek(fd, 5, SEEK_SET));
    result("seek end", lseek(fd, 0, SEEK_END));
    result("write prev", open("/proc/self/attr/prev", O_WRONLY));

    pthread_t other;
    pthread_create(&other, NULL, thread, NULL);
    pthread_join(other, NULL);

    pid_t parent = getpid();
    pid_t child = fork();
    if (child == 0) {
        result("child write through the parent's file", write(fd, "x", 1));
        char path[64];
        snprintf(path, sizeof path, "/proc/%d/attr/current", parent);
        int theirs = open(path, O_RDWR);
        result("child open of the parent's", theirs < 0 ? -1 : 0);
        result("child write of the parent's", write(theirs, "x", 1));
        result("child read of the parent's", read(theirs, buffer, sizeof buffer));
        _exit(0);
    }
    waitpid(child, NULL, 0);

    int pipes[2];
    pipe(pipes);
    child = fork();
    if (child == 0) {
        char byte;
        read(pipes[0], &byte, 1);
        _exit(0);
    }
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/attr/current", child);
    int held = open(path, O_RDONLY);
    write(pipes[1], "x", 1);
    usleep(200000);
    result("open of an ended child", open(path, O_RDONLY) < 0 ? -1 : 0);
    result("read of an ended child", read(held, buffer, sizeof buffer));
    waitpid(child, NULL, 0);
    result("read once reaped", pread(held, buffer, sizeof buffer, 0));
    return 0;
}
