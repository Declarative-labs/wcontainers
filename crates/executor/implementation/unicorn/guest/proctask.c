// Threads in /proc: the task directory's listing, each thread's own directory, names set per thread
// (PR_SET_NAME and a write to another thread's comm), what stat, status and comm show for each
// thread, thread-self, and a process whose first thread has ended while another goes on. Threads
// are printed by the order they were made in (the reference's numbers are not ours).

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

struct entry {
    uint64_t ino;
    int64_t off;
    uint16_t reclen;
    uint8_t type;
    char name[];
};

static int pid;
static pthread_barrier_t started, done;
static volatile int tids[3];

static int gettid_(void) { return (int)syscall(SYS_gettid); }

// A thread's place among the process's threads, from its number.
static int index_of(int tid) {
    for (int index = 0; index < 3; index++) {
        if (tids[index] == tid) {
            return index;
        }
    }
    return -1;
}

static void read_file(const char *path, char *out, size_t size) {
    out[0] = 0;
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        snprintf(out, size, "error %d", errno);
        return;
    }
    ssize_t got = read(fd, out, size - 1);
    out[got < 0 ? 0 : got] = 0;
    close(fd);
}

// Prints the lines of a status file that name and place the thread.
static void status_lines(const char *path) {
    char text[4096];
    read_file(path, text, sizeof text);
    for (char *line = strtok(text, "\n"); line; line = strtok(NULL, "\n")) {
        int value;
        if (sscanf(line, "Pid:\t%d", &value) == 1 || sscanf(line, "Tgid:\t%d", &value) == 1) {
            printf("  %.4s %d\n", line, index_of(value));
        } else if (!strncmp(line, "Name:", 5) || !strncmp(line, "State:", 6) ||
                   !strncmp(line, "Threads:", 8) || !strncmp(line, "SigBlk:", 7) ||
                   !strncmp(line, "Umask:", 6) || !strncmp(line, "FDSize:", 7)) {
            printf("  %s\n", line);
        }
    }
}

// Prints a stat file's number (relative), name, state and thread count.
static void stat_line(const char *path) {
    char text[4096];
    read_file(path, text, sizeof text);
    int number;
    char *open_paren = strchr(text, '('), *close_paren = strrchr(text, ')');
    if (sscanf(text, "%d", &number) != 1 || !open_paren || !close_paren) {
        printf("  stat: %s\n", text);
        return;
    }
    char state;
    long fields[20];
    sscanf(close_paren + 2, "%c %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld %ld",
           &state, &fields[0], &fields[1], &fields[2], &fields[3], &fields[4], &fields[5],
           &fields[6], &fields[7], &fields[8], &fields[9], &fields[10], &fields[11], &fields[12],
           &fields[13], &fields[14], &fields[15], &fields[16]);
    printf("  stat %d (%.*s) %c threads %ld\n", index_of(number), (int)(close_paren - open_paren - 1),
           open_paren + 1, state, fields[16]);
}

static void describe(int tid) {
    char path[128], text[256];
    printf("thread %d\n", index_of(tid));
    snprintf(path, sizeof path, "/proc/self/task/%d/comm", tid);
    read_file(path, text, sizeof text);
    printf("  task comm %s", text);
    snprintf(path, sizeof path, "/proc/%d/comm", tid);
    read_file(path, text, sizeof text);
    printf("  own comm %s", text);
    snprintf(path, sizeof path, "/proc/self/task/%d/stat", tid);
    stat_line(path);
    snprintf(path, sizeof path, "/proc/%d/stat", tid);
    stat_line(path);
    snprintf(path, sizeof path, "/proc/self/task/%d/status", tid);
    status_lines(path);
}

// A directory's entries in listing order, numbers relative, with the position after each, its
// type and what lstat says; only the names in `only`, when given.
static void list(const char *dir, const char *const *only) {
    int fd = open(dir, O_RDONLY | O_DIRECTORY);
    if (fd < 0) {
        printf("%s: error %d\n", dir, errno);
        return;
    }
    char buffer[8192];
    long got;
    while ((got = syscall(SYS_getdents64, fd, buffer, sizeof buffer)) > 0) {
        for (long at = 0; at < got; at += ((struct entry *)(buffer + at))->reclen) {
            struct entry *entry = (struct entry *)(buffer + at);
            if (only) {
                int wanted = 0;
                for (const char *const *name = only; *name; name++) {
                    wanted |= !strcmp(*name, entry->name);
                }
                if (!wanted) {
                    continue;
                }
            }
            struct stat st;
            fstatat(fd, entry->name, &st, AT_SYMLINK_NOFOLLOW);
            // `/proc` counts every process on the machine.
            if (!strcmp(entry->name, "..")) {
                st.st_nlink = 0;
            }
            int number = atoi(entry->name);
            if (number > 0) {
                printf("  %d off %ld type %d mode %o links %ld\n", index_of(number), (long)entry->off,
                       entry->type, st.st_mode, (long)st.st_nlink);
            } else {
                printf("  %s off %ld type %d mode %o links %ld\n", entry->name, (long)entry->off,
                       entry->type, st.st_mode, (long)st.st_nlink);
            }
        }
    }
    close(fd);
}

static void thread_self(const char *who) {
    char target[64];
    ssize_t length = readlink("/proc/thread-self", target, sizeof target - 1);
    target[length < 0 ? 0 : length] = 0;
    int group, thread;
    if (sscanf(target, "%d/task/%d", &group, &thread) == 2) {
        printf("%s thread-self %d/task/%d\n", who, index_of(group), index_of(thread));
    } else {
        printf("%s thread-self %s\n", who, target);
    }
    char text[64];
    read_file("/proc/thread-self/comm", text, sizeof text);
    printf("%s thread-self comm %s", who, text);
}

// The state stat shows for a thread.
static char state_of(int tid) {
    char path[64], text[512];
    snprintf(path, sizeof path, "/proc/self/task/%d/stat", tid);
    read_file(path, text, sizeof text);
    char *close_paren = strrchr(text, ')');
    return close_paren ? close_paren[2] : '?';
}

static void *worker(void *argument) {
    int index = (int)(intptr_t)argument;
    tids[index] = gettid_();
    if (index == 1) {
        prctl(PR_SET_NAME, "first-worker-with-a-long-name");
        thread_self("worker");
    } else {
        sigset_t set;
        sigemptyset(&set);
        sigaddset(&set, SIGUSR1);
        sigaddset(&set, SIGTERM);
        pthread_sigmask(SIG_BLOCK, &set, NULL);
    }
    pthread_barrier_wait(&started);
    pthread_barrier_wait(&done);
    return NULL;
}

// The first thread ends while a second goes on and looks at the process.
static void *survivor(void *argument) {
    (void)argument;
    tids[1] = gettid_();
    struct timespec pause = {0, 20 * 1000 * 1000};
    char path[64], text[256];
    // Waits until the first thread has ended (its state is no longer running or sleeping).
    for (int tries = 0; tries < 500; tries++) {
        snprintf(path, sizeof path, "/proc/%d/stat", pid);
        read_file(path, text, sizeof text);
        char *close_paren = strrchr(text, ')');
        if (close_paren && close_paren[2] == 'Z') {
            break;
        }
        nanosleep(&pause, NULL);
    }
    printf("after the first thread ended\n");
    snprintf(path, sizeof path, "/proc/%d/stat", pid);
    stat_line(path);
    snprintf(path, sizeof path, "/proc/%d/status", pid);
    status_lines(path);
    snprintf(path, sizeof path, "/proc/%d/comm", pid);
    read_file(path, text, sizeof text);
    printf("  first comm %s", text);
    snprintf(path, sizeof path, "/proc/%d/task", pid);
    list(path, NULL);
    describe(gettid_());
    fflush(stdout);
    exit(5);
}

int main(void) {
    setvbuf(stdout, NULL, _IOFBF, 1 << 16);
    pid = getpid();
    prctl(PR_SET_NAME, "main-thread");
    tids[0] = pid;
    pthread_barrier_init(&started, NULL, 3);
    pthread_barrier_init(&done, NULL, 3);
    pthread_t threads[3];
    for (int index = 1; index < 3; index++) {
        pthread_create(&threads[index], NULL, worker, (void *)(intptr_t)index);
    }
    pthread_barrier_wait(&started);
    // Until both workers wait at the second barrier.
    struct timespec pause = {0, 1000 * 1000};
    for (int index = 1; index < 3; index++) {
        while (state_of(tids[index]) != 'S') {
            nanosleep(&pause, NULL);
        }
    }
    printf("setname %d\n", pthread_setname_np(threads[2], "second-worker"));
    printf("setname too long %d\n", pthread_setname_np(threads[2], "a-name-of-sixteen"));
    thread_self("main");

    printf("task\n");
    list("/proc/self/task", NULL);
    struct stat st;
    stat("/proc/self/task", &st);
    printf("task links %ld\n", (long)st.st_nlink);
    for (int index = 0; index < 3; index++) {
        describe(tids[index]);
    }
    char path[128], text[256];
    printf("self comm ");
    read_file("/proc/self/comm", text, sizeof text);
    printf("%s", text);

    static const char *const built[] = {"auxv",   "cgroup", "cmdline", "comm",  "environ",
                                        "limits", "maps",   "mounts",  "stat",  "statm",
                                        "status", "cwd",    "exe",     "root",  "fd",
                                        "task",   ".",      "..",      NULL};
    printf("own directory\n");
    snprintf(path, sizeof path, "/proc/self/task/%d", tids[1]);
    list(path, built);
    printf("a thread's directory in /proc\n");
    snprintf(path, sizeof path, "/proc/%d", tids[1]);
    list(path, built);
    printf("its task\n");
    snprintf(path, sizeof path, "/proc/%d/task", tids[1]);
    list(path, NULL);
    snprintf(path, sizeof path, "/proc/%d/task/%d", tids[1], tids[1]);
    printf("thread under thread %d\n", stat(path, &st) == 0 ? 0 : errno);
    snprintf(path, sizeof path, "/proc/self/task/%d/task", tids[1]);
    printf("task under task %d\n", stat(path, &st) == 0 ? 0 : errno);
    snprintf(path, sizeof path, "/proc/self/task/%d", pid + 1000);
    printf("no such thread %d\n", stat(path, &st) == 0 ? 0 : errno);
    snprintf(path, sizeof path, "/proc/self/task/%d/fd/1", tids[2]);
    printf("thread's descriptor %d\n", stat(path, &st) == 0 ? 0 : errno);

    // Renamed through the thread's own comm file, which takes the whole count.
    snprintf(path, sizeof path, "/proc/self/task/%d/comm", tids[1]);
    int fd = open(path, O_WRONLY);
    printf("write comm %zd\n", write(fd, "renamed\0ignored", 15));
    close(fd);
    read_file(path, text, sizeof text);
    printf("renamed %s", text);
    read_file("/proc/self/comm", text, sizeof text);
    printf("main still %s", text);

    pthread_barrier_wait(&done);
    for (int index = 1; index < 3; index++) {
        pthread_join(threads[index], NULL);
    }
    // A joined thread's number is cleared before Linux lets go of the thread, so it may be listed
    // a little longer.
    while (stat("/proc/self/task", &st) == 0 && st.st_nlink != 3) {
        nanosleep(&pause, NULL);
    }
    snprintf(path, sizeof path, "/proc/self/task/%d", tids[1]);
    printf("joined thread %d\n", stat(path, &st) == 0 ? 0 : errno);
    list("/proc/self/task", NULL);
    fflush(stdout);

    pid_t child = fork();
    if (child == 0) {
        pid = getpid();
        tids[0] = pid;
        tids[1] = tids[2] = 0;
        prctl(PR_SET_NAME, "first-of-child");
        pthread_t thread;
        pthread_create(&thread, NULL, survivor, NULL);
        syscall(SYS_exit, 0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("child status %x\n", status);
    return 0;
}
