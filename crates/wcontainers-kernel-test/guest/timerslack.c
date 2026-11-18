// /proc/<pid>/timerslack_ns: read and written by the process's first thread, refused to its other
// threads and to other processes without CAP_SYS_NICE, and what prctl sees of it.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static pid_t leader;

static void show(const char *label) {
    char path[64], text[32] = "";
    snprintf(path, sizeof path, "/proc/%d/timerslack_ns", leader);
    int fd = open(path, O_RDONLY);
    errno = 0;
    long got = fd < 0 ? -1 : read(fd, text, sizeof text - 1);
    printf("%s: open %d read %ld %s %s", label, fd >= 0, got, got < 0 ? strerror(errno) : "",
           got > 0 ? text : "\n");
    if (fd >= 0) {
        close(fd);
    }
}

static void put(const char *text) {
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/timerslack_ns", leader);
    int fd = open(path, O_WRONLY);
    errno = 0;
    long r = write(fd, text, strlen(text));
    printf("write \"%s\": %ld %s; prctl %ld\n", text, r, r < 0 ? strerror(errno) : "",
           (long)prctl(PR_GET_TIMERSLACK));
    close(fd);
}

static void *other_thread(void *unused) {
    show("other thread");
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/task/%ld/timerslack_ns", leader, (long)syscall(SYS_gettid));
    printf("in the thread's own directory: %d\n", access(path, F_OK));
    return unused;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    leader = getpid();
    show("start");
    prctl(PR_SET_TIMERSLACK, 12345);
    show("after prctl");
    const char *values[] = {"5000", "+7\n", "0", "-1", "12x", "", "18446744073709551616"};
    for (unsigned i = 0; i < sizeof values / sizeof *values; i++) {
        put(values[i]);
    }
    pthread_t thread;
    pthread_create(&thread, NULL, other_thread, NULL);
    pthread_join(thread, NULL);
    if (fork() == 0) {
        show("child");
        _exit(0);
    }
    wait(NULL);
    return 0;
}
