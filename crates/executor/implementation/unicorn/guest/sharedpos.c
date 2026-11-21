// Processes and threads moving one shared open file's position on the backend: writes after fork,
// writes from two threads, reads from two threads, directory reads from two threads and appends
// through two descriptors. Linux holds a shared file's position across each call (f_pos_lock) and
// an inode across each write, so nothing is lost, doubled or read twice.
#define _GNU_SOURCE
#include <dirent.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static int fd;

static void *writer(void *arg) {
    for (int i = 0; i < 100; i++) write(fd, arg, 2);
    return 0;
}

static void count(const char *name) {
    char buf[1000];
    int in = open(name, O_RDONLY);
    int n = read(in, buf, sizeof buf), lines = 0, zeros = 0;
    for (int i = 0; i < n; i++) {
        lines += buf[i] == '\n';
        zeros += buf[i] == 0;
    }
    printf("%s: %d bytes, %d lines, %d zero bytes\n", name, n, lines, zeros);
    close(in);
}

static int seen[256];
static void *reader(void *arg) {
    unsigned char c;
    while (read(fd, &c, 1) == 1) __atomic_add_fetch(&seen[c], 1, __ATOMIC_SEQ_CST);
    return 0;
}

static int listed;
static void *lister(void *arg) {
    char buf[64];
    long n;
    // A buffer for about one entry, so each call moves the position a little.
    while ((n = syscall(SYS_getdents64, fd, buf, sizeof buf)) > 0) {
        for (long at = 0; at < n; at += ((struct dirent *)(buf + at))->d_reclen) {
            __atomic_add_fetch(&listed, 1, __ATOMIC_SEQ_CST);
        }
    }
    return 0;
}

static int appender;
static void *append(void *arg) {
    for (int i = 0; i < 100; i++) write(appender, arg, 2);
    return 0;
}

int main(void) {
    pthread_t a, b;
    fd = open("procs", O_CREAT | O_TRUNC | O_WRONLY, 0644);
    pid_t child = fork();
    if (child == 0) {
        writer("c\n");
        _exit(0);
    }
    writer("p\n");
    waitpid(child, 0, 0);
    printf("procs position %ld\n", (long)lseek(fd, 0, SEEK_CUR));
    close(fd);
    count("procs");

    fd = open("threads", O_CREAT | O_TRUNC | O_WRONLY, 0644);
    pthread_create(&a, 0, writer, "a\n");
    pthread_create(&b, 0, writer, "b\n");
    pthread_join(a, 0);
    pthread_join(b, 0);
    printf("threads position %ld\n", (long)lseek(fd, 0, SEEK_CUR));
    close(fd);
    count("threads");

    unsigned char bytes[256];
    for (int i = 0; i < 256; i++) bytes[i] = i;
    fd = open("bytes", O_CREAT | O_TRUNC | O_RDWR, 0644);
    write(fd, bytes, 256);
    lseek(fd, 0, SEEK_SET);
    pthread_create(&a, 0, reader, 0);
    pthread_create(&b, 0, reader, 0);
    pthread_join(a, 0);
    pthread_join(b, 0);
    int twice = 0, never = 0;
    for (int i = 0; i < 256; i++) {
        twice += seen[i] > 1;
        never += seen[i] == 0;
    }
    printf("bytes read twice %d, never %d, position after %ld\n", twice, never,
           (long)lseek(fd, 0, SEEK_CUR));
    close(fd);

    mkdir("many", 0755);
    for (int i = 0; i < 40; i++) {
        char name[32];
        snprintf(name, sizeof name, "many/entry-%02d", i);
        close(open(name, O_CREAT | O_WRONLY, 0644));
    }
    fd = open("many", O_RDONLY | O_DIRECTORY);
    pthread_create(&a, 0, lister, 0);
    pthread_create(&b, 0, lister, 0);
    pthread_join(a, 0);
    pthread_join(b, 0);
    printf("entries listed %d\n", listed);
    close(fd);

    fd = open("appended", O_CREAT | O_TRUNC | O_WRONLY | O_APPEND, 0644);
    appender = open("appended", O_WRONLY | O_APPEND);
    pthread_create(&a, 0, writer, "x\n");
    pthread_create(&b, 0, append, "y\n");
    pthread_join(a, 0);
    pthread_join(b, 0);
    close(fd);
    close(appender);
    count("appended");
    return 0;
}
