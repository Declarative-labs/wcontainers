// How the out-of-memory killer would weigh a process: oom_score_adj, the older oom_adj, and
// oom_score at the scale's ends, read and written by the process, its child, a thread's own
// directory, after an exec, and by a user.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static void show(const char *label, pid_t pid, const char *name) {
    char path[64], text[32] = "";
    snprintf(path, sizeof path, "/proc/%d/%s", pid, name);
    int fd = open(path, O_RDONLY);
    long got = fd < 0 ? -1 : read(fd, text, sizeof text - 1);
    text[got > 0 ? got : 0] = 0;
    if (fd >= 0) {
        close(fd);
    }
    printf("%s %s: %s", label, name, fd < 0 ? "cannot open\n" : text);
}

static void put(pid_t pid, const char *name, const char *text) {
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/%s", pid, name);
    int fd = open(path, O_WRONLY);
    if (fd < 0) {
        printf("open %s for writing: %s\n", name, strerror(errno));
        return;
    }
    errno = 0;
    long r = write(fd, text, strlen(text));
    printf("write %s \"%s\": %ld %s\n", name, text, r, r < 0 ? strerror(errno) : "");
    close(fd);
}

static void both(const char *label, pid_t pid) {
    show(label, pid, "oom_score_adj");
    show(label, pid, "oom_adj");
}

static void *thread(void *unused) {
    char name[64];
    snprintf(name, sizeof name, "task/%ld/oom_score_adj", (long)syscall(SYS_gettid));
    char path[96];
    snprintf(path, sizeof path, "/proc/%d/%s", getpid(), name);
    int fd = open(path, O_WRONLY);
    printf("thread's own file: write %ld\n", fd < 0 ? -1L : (long)write(fd, "300", 3));
    close(fd);
    return unused;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    if (argc > 1) {
        both("after exec", getpid());
        return 0;
    }
    pid_t self = getpid();
    both("start", self);
    show("start", self, "oom_score");
    put(self, "oom_score_adj", "500");
    both("set", self);
    pid_t child = fork();
    if (child == 0) {
        both("child", getpid());
        const char *values[] = {"100", "-1", "2000", "1000", " 42 \n", "0x10", "010", " 7x", "",
                                "-0", "1000000000000"};
        for (unsigned i = 0; i < sizeof values / sizeof *values; i++) {
            put(getpid(), "oom_score_adj", values[i]);
        }
        both("child after", getpid());
        const char *legacy[] = {"-5", "20", "15", "8", "-17", "0", "-1"};
        for (unsigned i = 0; i < sizeof legacy / sizeof *legacy; i++) {
            put(getpid(), "oom_adj", legacy[i]);
            both("child", getpid());
        }
        show("child", getpid(), "oom_score");
        _exit(0);
    }
    waitpid(child, NULL, 0);
    both("parent after child", self);
    pthread_t t;
    pthread_create(&t, NULL, thread, NULL);
    pthread_join(t, NULL);
    both("after thread", self);
    child = fork();
    if (child == 0) {
        execl("/proc/self/exe", "oomadj", "exec", (char *)NULL);
        _exit(1);
    }
    waitpid(child, NULL, 0);
    child = fork();
    if (child == 0) {
        setgid(1000);
        setuid(1000);
        put(getppid(), "oom_score_adj", "600");
        both("user", getpid());
        put(getpid(), "oom_score_adj", "600");
        put(getpid(), "oom_score_adj", "200");
        _exit(0);
    }
    waitpid(child, NULL, 0);
    return 0;
}
