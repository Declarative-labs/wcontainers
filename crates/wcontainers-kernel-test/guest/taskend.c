// Tasks that end while the embedder holds them off the kernel, as stopped threads end in Linux. Each
// mode marks one moment with getppid, which nothing else calls: the test takes a task there (the
// caller, or the other thread of its process); on the reference nothing is taken and the task
// waits where it is.
//   exec-main:   a worker reads an empty pipe; at the main thread's mark the worker is taken, and
//                the main thread runs this program again ("new").
//   exec-worker: the main thread is taken at its mark (on the reference it reads an empty pipe);
//                the worker then runs this program again through /proc/self/exe.
//   new:         prints its thread count, sleeps, prints it again and ends its only thread.
//   exit:        a child's worker reads an empty pipe; at the child's mark the worker is taken and
//                the child exits; the parent waits for it, sleeps and ends.
//   proc:        a worker reads an empty pipe; at the main thread's mark it is taken, and the main
//                thread shows what /proc says of it.
//   leader-proc: the main thread is taken at its mark (on the reference it reads an empty pipe);
//                the worker shows what /proc says of the process.
//   robust:      a child's worker holds a robust lock in memory shared with the parent and reads an
//                empty pipe; at the child's mark the worker is taken and the child exits; the
//                parent then takes the lock.
//   robust-copy: as robust, but the parent only tries the lock (the test makes the child's end the
//                end of a copy of its context, its worker going on elsewhere).
//   vfork:       a vfork child is taken at its mark (on the reference it pauses); another child
//                kills it; the parent goes on and waits for both.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum {
    TASKEND_SETTLE_MS = 50,
    TASKEND_POLL_MS = 10,
    TASKEND_LINE = 256,
    MS = 1000 * 1000,
    SECOND_MS = 1000,
};

static const char *THREADS_FIELD = "Threads:";

static int idle_pipe[2];
static pid_t worker_tid;

static void taskend_sleep(long ms) {
    struct timespec time = {ms / SECOND_MS, (ms % SECOND_MS) * MS};
    nanosleep(&time, NULL);
}

static void mark(void) {
    syscall(SYS_getppid);
}

// Waits on a pipe nobody writes.
static void *reader(void *unused) {
    (void)unused;
    worker_tid = (pid_t)syscall(SYS_gettid);
    char byte;
    read(idle_pipe[0], &byte, sizeof byte);
    return NULL;
}

static pthread_t start_reader(void) {
    pipe(idle_pipe);
    pthread_t thread;
    pthread_create(&thread, NULL, reader, NULL);
    return thread;
}

static void wait_forever(void) {
    char byte;
    read(idle_pipe[0], &byte, sizeof byte);
}

static int thread_count(void) {
    FILE *status = fopen("/proc/self/status", "r");
    if (status == NULL) {
        return -1;
    }
    char line[TASKEND_LINE];
    int count = -1;
    size_t field = strlen(THREADS_FIELD);
    while (fgets(line, sizeof line, status) != NULL) {
        if (strncmp(line, THREADS_FIELD, field) == 0) {
            count = atoi(line + field);
        }
    }
    fclose(status);
    return count;
}

static int task_directories(void) {
    DIR *tasks = opendir("/proc/self/task");
    if (tasks == NULL) {
        return -1;
    }
    int count = 0;
    struct dirent *entry;
    while ((entry = readdir(tasks)) != NULL) {
        count += entry->d_name[0] != '.';
    }
    closedir(tasks);
    return count;
}

static int link_readable(const char *path) {
    char target[PATH_MAX];
    return readlink(path, target, sizeof target) > 0;
}

static int file_opens(const char *path) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        return 0;
    }
    close(fd);
    return 1;
}

static void new_program(void) {
    printf("new program threads %d\n", thread_count());
    fflush(stdout);
    taskend_sleep(TASKEND_SETTLE_MS);
    printf("done threads %d\n", thread_count());
    fflush(stdout);
    syscall(SYS_exit, 0);
}

static void exec_main(const char *self) {
    start_reader();
    taskend_sleep(TASKEND_SETTLE_MS);
    mark();
    fflush(stdout);
    execl(self, self, "new", (char *)NULL);
    printf("exec failed %s\n", strerror(errno));
}

static const char *program;

static void *exec_from_worker(void *unused) {
    (void)unused;
    taskend_sleep(TASKEND_SETTLE_MS);
    execl("/proc/self/exe", program, "new", (char *)NULL);
    printf("exec failed %s\n", strerror(errno));
    fflush(stdout);
    exit(1);
}

static void exec_worker(void) {
    pipe(idle_pipe);
    pthread_t thread;
    pthread_create(&thread, NULL, exec_from_worker, NULL);
    mark();
    wait_forever();
}

static void child_exits(void) {
    pid_t child = fork();
    if (child == 0) {
        start_reader();
        taskend_sleep(TASKEND_SETTLE_MS);
        mark();
        printf("exiting\n");
        exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("child exited %d\n", WEXITSTATUS(status));
    fflush(stdout);
    taskend_sleep(TASKEND_SETTLE_MS);
    printf("parent done\n");
}

static void proc_of_worker(void) {
    start_reader();
    taskend_sleep(TASKEND_SETTLE_MS);
    mark();
    char path[TASKEND_LINE];
    snprintf(path, sizeof path, "/proc/self/task/%d/exe", worker_tid);
    int exe = link_readable(path);
    snprintf(path, sizeof path, "/proc/self/task/%d/cwd", worker_tid);
    int cwd = link_readable(path);
    snprintf(path, sizeof path, "/proc/self/task/%d/status", worker_tid);
    int status = file_opens(path);
    snprintf(path, sizeof path, "/proc/%d/status", worker_tid);
    int found = file_opens(path);
    printf("threads %d task-dirs %d worker: exe %d cwd %d status %d found %d\n", thread_count(),
           task_directories(), exe, cwd, status, found);
    exit(0);
}

static void *leader_shown(void *unused) {
    (void)unused;
    taskend_sleep(TASKEND_SETTLE_MS);
    printf("leader away: exe %d cwd %d status %d maps %d threads %d task-dirs %d\n",
           link_readable("/proc/self/exe"), link_readable("/proc/self/cwd"),
           file_opens("/proc/self/status"), file_opens("/proc/self/maps"), thread_count(),
           task_directories());
    exit(0);
}

static void leader_proc(void) {
    pipe(idle_pipe);
    pthread_t thread;
    pthread_create(&thread, NULL, leader_shown, NULL);
    mark();
    wait_forever();
}

static int ready_pipe[2];
static pthread_mutex_t *shared_lock;

static void *holder(void *unused) {
    (void)unused;
    pthread_mutex_lock(shared_lock);
    char byte = 0;
    write(ready_pipe[1], &byte, sizeof byte);
    wait_forever();
    return NULL;
}

static void robust(int try_only) {
    shared_lock = mmap(NULL, sizeof *shared_lock, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS,
                       -1, 0);
    pthread_mutexattr_t attributes;
    pthread_mutexattr_init(&attributes);
    pthread_mutexattr_setpshared(&attributes, PTHREAD_PROCESS_SHARED);
    pthread_mutexattr_setrobust(&attributes, PTHREAD_MUTEX_ROBUST);
    pthread_mutex_init(shared_lock, &attributes);
    pid_t child = fork();
    if (child == 0) {
        pipe(idle_pipe);
        pipe(ready_pipe);
        pthread_t thread;
        pthread_create(&thread, NULL, holder, NULL);
        char byte;
        read(ready_pipe[0], &byte, sizeof byte);
        mark();
        exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    int answer = try_only ? pthread_mutex_trylock(shared_lock) : pthread_mutex_lock(shared_lock);
    printf("lock %s\n", answer == EOWNERDEAD ? "owner died" : answer == EBUSY ? "held" : strerror(answer));
}

static void vfork_killed(void) {
    volatile pid_t *slot = mmap(NULL, sizeof *slot, PROT_READ | PROT_WRITE,
                                MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    pid_t killer = fork();
    if (killer == 0) {
        while (*slot == 0) {
            taskend_sleep(TASKEND_POLL_MS);
        }
        taskend_sleep(TASKEND_SETTLE_MS);
        kill(*slot, SIGKILL);
        _exit(0);
    }
    pid_t child = vfork();
    if (child == 0) {
        *slot = getpid();
        mark();
        for (;;) {
            pause();
        }
    }
    int status;
    waitpid(child, &status, 0);
    printf("vfork child killed %d\n", WIFSIGNALED(status) ? WTERMSIG(status) : -1);
    waitpid(killer, &status, 0);
    printf("killer exited %d\n", WEXITSTATUS(status));
}

int main(int argc, char **argv) {
    program = argv[0];
    const char *mode = argc > 1 ? argv[1] : "";
    if (strcmp(mode, "new") == 0) {
        new_program();
    } else if (strcmp(mode, "exec-main") == 0) {
        exec_main(argv[0]);
    } else if (strcmp(mode, "exec-worker") == 0) {
        exec_worker();
    } else if (strcmp(mode, "exit") == 0) {
        child_exits();
    } else if (strcmp(mode, "proc") == 0) {
        proc_of_worker();
    } else if (strcmp(mode, "leader-proc") == 0) {
        leader_proc();
    } else if (strcmp(mode, "robust") == 0) {
        robust(0);
    } else if (strcmp(mode, "robust-copy") == 0) {
        robust(1);
    } else if (strcmp(mode, "vfork") == 0) {
        vfork_killed();
    } else {
        printf("unknown mode %s\n", mode);
        return 1;
    }
    return 0;
}
