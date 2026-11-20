// The embedder's hooks at their edges; on the reference no hook runs, so every answer is Linux's own.
//   alone:  a worker alone on the processor writes a page in a loop until a signal's handler runs;
//           the main thread marks three moments with getppid (with "self" it sends the worker the
//           signal itself at the second) and then joins it.
//   taken:  a worker meets a watched region outside any call, by a store ("store") or by its
//           signal's frame on that region as its alternate stack ("frame"), then waits in
//           epoll_wait and nanosleep; the main thread names the region in a getppid, which the
//           embedder watches, and joins the worker.
//   fatal:  children each kill themselves with one signal whose default action ends them (with
//           and without a core dump; core dumps are limited to none) and are waited for.
//   ends:   shared mappings of a file and of a memory file start and end in every way that cuts
//           or joins them; each step is marked with sched_yield and shows /proc/self/maps.
//   writer: a parent maps a file shared and writable and waits for its child, which maps the same
//           file read-only, marks it with sched_yield, shows /proc/self/maps and exits.
//   memory: a forked child, a vfork child that runs this program again, and a child cloned with
//           CLONE_VM, each waited for.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sched.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum {
    EDGES_PAGES = 64,
    EDGES_FILE_PAGES = 4,
    EDGES_SETTLE_MS = 50,
    EDGES_CHILD_MS = 100,
    EDGES_SENT_VALUE = 21,
    EDGES_STACK = 64 * 1024,
    EDGES_LINE = 256,
    MS = 1000 * 1000,
    SECOND_MS = 1000,
};

// Iterations the worker goes on for after the last mark before it gives up on the signal.
static const unsigned long EDGES_CAP = 10UL * 1000 * 1000;
// Where the mappings go: inside the 32-bit space both sides give a program.
static const uintptr_t EDGES_BASE = 0x36000000;
static const char *EDGES_FILE = "/tmp/edges-file";

static long page;

static void edges_sleep(long ms) {
    struct timespec time = {ms / SECOND_MS, (ms % SECOND_MS) * MS};
    nanosleep(&time, NULL);
}

static void *at(int index) {
    return (void *)(EDGES_BASE + (uintptr_t)index * (uintptr_t)page);
}

// Prints the lines of /proc/self/maps inside the region, as range, permissions and offset.
static void show(const char *name) {
    printf("step %s\n", name);
    FILE *maps = fopen("/proc/self/maps", "r");
    char line[EDGES_LINE];
    while (fgets(line, sizeof line, maps)) {
        uintptr_t start, end;
        char permissions[8];
        unsigned long offset;
        if (sscanf(line, "%lx-%lx %7s %lx", &start, &end, permissions, &offset) != 4) {
            continue;
        }
        if (start >= EDGES_BASE && end <= (uintptr_t)at(EDGES_PAGES)) {
            printf("%lx-%lx %s %lx\n", start, end, permissions, offset);
        }
    }
    fclose(maps);
    fflush(stdout);
    sched_yield();
}

static volatile sig_atomic_t handled;
static volatile int seen_code, seen_value;
static volatile int after_marks;
static volatile pid_t worker_tid;

static void handler(int signal, siginfo_t *info, void *context) {
    (void)signal;
    (void)context;
    seen_code = info->si_code;
    seen_value = info->si_value.sival_int;
    handled = 1;
}

static void *alone_worker(void *argument) {
    volatile char *spin = argument;
    worker_tid = (pid_t)syscall(SYS_gettid);
    unsigned long counted = 0;
    while (!handled) {
        spin[0]++;
        if (after_marks && ++counted >= EDGES_CAP) {
            break;
        }
    }
    printf("worker handled %d capped %d\n", (int)handled, counted >= EDGES_CAP);
    return NULL;
}

static int edges_alone(const char *how) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_sigaction = handler;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGUSR1, &action, NULL);
    void *spin = mmap(at(0), page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    pthread_t worker;
    pthread_create(&worker, NULL, alone_worker, spin);
    edges_sleep(EDGES_SETTLE_MS);
    getppid();  // the embedder watches the page
    edges_sleep(EDGES_SETTLE_MS);
    if (strcmp(how, "self") == 0) {
        siginfo_t info;
        memset(&info, 0, sizeof info);
        info.si_signo = SIGUSR1;
        info.si_code = SI_QUEUE;
        info.si_value.sival_int = EDGES_SENT_VALUE;
        syscall(SYS_rt_tgsigqueueinfo, getpid(), worker_tid, SIGUSR1, &info);
    }
    getppid();  // the embedder sends the signal
    edges_sleep(EDGES_SETTLE_MS);
    getppid();  // the embedder ends the watch
    after_marks = 1;
    pthread_join(worker, NULL);
    printf("handled %d code %d value %d\n", (int)handled, seen_code, seen_value);
    return 0;
}

static void taken_handler(int signal) {
    (void)signal;
    handled = 1;
}

// Meets the embedder's watch outside any call, by its own store or by its signal's frame on the
// watched alternate stack, then waits in two calls that only a signal could interrupt.
static void *taken_worker(void *argument) {
    const char *how = argument;
    edges_sleep(EDGES_SETTLE_MS);
    if (strcmp(how, "frame") == 0) {
        stack_t stack = {.ss_sp = at(0), .ss_flags = 0, .ss_size = EDGES_STACK};
        sigaltstack(&stack, NULL);
        syscall(SYS_tgkill, getpid(), syscall(SYS_gettid), SIGUSR1);
    } else {
        *(volatile char *)at(0) = 1;
    }
    int poll = epoll_create1(0);
    struct epoll_event event;
    int answer = epoll_wait(poll, &event, 1, EDGES_CHILD_MS);
    printf("epoll %d %s\n", answer, answer < 0 ? strerror(errno) : "");
    struct timespec time = {0, EDGES_SETTLE_MS * MS};
    answer = nanosleep(&time, NULL);
    printf("sleep %d %s handled %d\n", answer, answer < 0 ? strerror(errno) : "", (int)handled);
    return NULL;
}

static int edges_taken(const char *how) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = taken_handler;
    action.sa_flags = SA_ONSTACK;
    sigaction(SIGUSR1, &action, NULL);
    mmap(at(0), EDGES_STACK, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    syscall(SYS_getppid, at(0), EDGES_STACK);  // the embedder watches the region it names
    pthread_t worker;
    pthread_create(&worker, NULL, taken_worker, (void *)how);
    pthread_join(worker, NULL);
    printf("joined\n");
    return 0;
}

// Children each send themselves one signal whose default action ends them, some dumping core.
static int edges_fatal(void) {
    struct rlimit none = {0, 0};
    setrlimit(RLIMIT_CORE, &none);
    const int signals[] = {SIGQUIT, SIGSEGV, SIGTERM, SIGABRT, SIGUSR2};
    for (size_t index = 0; index < sizeof signals / sizeof signals[0]; index++) {
        pid_t child = fork();
        if (child == 0) {
            kill(getpid(), signals[index]);
            _exit(0);
        }
        int status;
        waitpid(child, &status, 0);
        printf("signal %d ended %d\n", signals[index], WIFSIGNALED(status) ? WTERMSIG(status) : -1);
    }
    return 0;
}

static void *shared(void *where, int pages, int fd, int index) {
    return mmap(where, (size_t)pages * page, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_FIXED, fd,
                (off_t)index * page);
}

// Maps two pages of `fd` side by side by two calls, then grows one in place, puts shared
// anonymous memory over the middle of another, and cuts a hole in a third; each is then unmapped
// whole.
static void edges_cuts(int fd, const char *kind) {
    char name[EDGES_LINE];
    shared(at(0), 1, fd, 0);
    shared(at(1), 1, fd, 1);
    snprintf(name, sizeof name, "%s-adjacent", kind);
    show(name);
    munmap(at(0), 2 * page);

    shared(at(4), 1, fd, 0);
    mremap(at(4), page, 3 * page, 0);
    snprintf(name, sizeof name, "%s-grown", kind);
    show(name);
    munmap(at(4), 3 * page);

    shared(at(8), EDGES_FILE_PAGES, fd, 0);
    mmap(at(9), 2 * page, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    snprintf(name, sizeof name, "%s-covered", kind);
    show(name);
    munmap(at(8), EDGES_FILE_PAGES * page);

    shared(at(12), EDGES_FILE_PAGES, fd, 0);
    munmap(at(13), page);
    snprintf(name, sizeof name, "%s-hole", kind);
    show(name);
    munmap(at(12), EDGES_FILE_PAGES * page);

    snprintf(name, sizeof name, "%s-gone", kind);
    show(name);
}

static int edges_ends(void) {
    int file = open(EDGES_FILE, O_RDWR | O_CREAT | O_TRUNC, 0600);
    ftruncate(file, EDGES_FILE_PAGES * page);
    edges_cuts(file, "file");
    int memory = memfd_create("edges", 0);
    ftruncate(memory, EDGES_FILE_PAGES * page);
    edges_cuts(memory, "memfd");
    return 0;
}

static int edges_writer(void) {
    int file = open(EDGES_FILE, O_RDWR | O_CREAT | O_TRUNC, 0600);
    ftruncate(file, EDGES_FILE_PAGES * page);
    pid_t child = fork();
    if (child == 0) {
        edges_sleep(EDGES_CHILD_MS);
        int reader = open(EDGES_FILE, O_RDONLY);
        mmap(at(4), page, PROT_READ, MAP_SHARED | MAP_FIXED, reader, 0);
        show("reader");
        _exit(0);
    }
    shared(at(0), 1, file, 0);
    int status;
    waitpid(child, &status, 0);
    show("writer");
    return 0;
}

static int edges_cloned(void *argument) {
    (void)argument;
    syscall(SYS_exit, 0);
    return 0;
}

static int edges_memory(const char *self) {
    int status;
    pid_t child = fork();
    if (child == 0) {
        _exit(0);
    }
    waitpid(child, &status, 0);
    child = vfork();
    if (child == 0) {
        execl(self, self, "nothing", (char *)NULL);
        _exit(1);
    }
    waitpid(child, &status, 0);
    char *stack = malloc(EDGES_STACK);
    child = clone(edges_cloned, stack + EDGES_STACK, CLONE_VM | SIGCHLD, NULL);
    waitpid(child, &status, 0);
    printf("children %d\n", WEXITSTATUS(status));
    return 0;
}

int main(int count, char **arguments) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    page = sysconf(_SC_PAGESIZE);
    const char *mode = count > 1 ? arguments[1] : "";
    const char *how = count > 2 ? arguments[2] : "";
    if (strcmp(mode, "alone") == 0) {
        return edges_alone(how);
    }
    if (strcmp(mode, "fatal") == 0) {
        return edges_fatal();
    }
    if (strcmp(mode, "taken") == 0) {
        return edges_taken(how);
    }
    if (strcmp(mode, "ends") == 0) {
        return edges_ends();
    }
    if (strcmp(mode, "writer") == 0) {
        return edges_writer();
    }
    if (strcmp(mode, "memory") == 0) {
        return edges_memory(arguments[0]);
    }
    if (strcmp(mode, "nothing") == 0) {
        return 0;
    }
    fprintf(stderr, "unknown mode %s\n", mode);
    return 1;
}
