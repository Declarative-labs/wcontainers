// Resource limits and signals a process sends itself, run as a container's first process: each argument
// names one case; the program prints what it saw and may die of what the case provokes.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <unistd.h>

static void limit(int resource, rlim_t current, rlim_t maximum) {
    struct rlimit value = {current, maximum};
    printf("setrlimit %d\n", setrlimit(resource, &value));
}

static void show(const char *what, long result) {
    printf("%s %ld %s\n", what, result, result < 0 ? strerror(errno) : "");
}

static void pending(void) {
    sigset_t set;
    sigpending(&set);
    printf("pending term %d xfsz %d usr1 %d\n", sigismember(&set, SIGTERM), sigismember(&set, SIGXFSZ),
           sigismember(&set, SIGUSR1));
}

static void block(int signal, int how) {
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, signal);
    sigprocmask(how, &set, 0);
}

// Touches every page of a stack array of `size` bytes, from the top down, as the stack grows.
static int grow(long size) {
    volatile char frame[size];
    for (long at = size - 1; at >= 0; at -= 4096) {
        frame[at] = 1;
    }
    return frame[0];
}

int main(int argc, char **argv) {
    const char *kind = argc > 1 ? argv[1] : "";
    setvbuf(stdout, NULL, _IONBF, 0);
    char buffer[200];
    memset(buffer, 'x', sizeof buffer);
    if (!strcmp(kind, "fsize")) {
        int fd = open("/tmp/limited", O_CREAT | O_RDWR | O_TRUNC, 0644);
        limit(RLIMIT_FSIZE, 100, RLIM_INFINITY);
        show("write 80", write(fd, buffer, 80));
        show("write 50", write(fd, buffer, 50));
        show("write past", write(fd, buffer, 10));
        show("write nothing", write(fd, buffer, 0));
        show("pwrite past", pwrite(fd, buffer, 10, 500));
        show("ftruncate", ftruncate(fd, 200));
        show("ftruncate down", ftruncate(fd, 50));
        show("fallocate", fallocate(fd, 0, 0, 300));
        block(SIGXFSZ, SIG_BLOCK);
        show("write blocked", pwrite(fd, buffer, 10, 100));
        pending();
        block(SIGXFSZ, SIG_UNBLOCK);
        pending();
        printf("still here\n");
    } else if (!strcmp(kind, "kill")) {
        show("kill term", kill(getpid(), SIGTERM));
        show("kill kill", kill(getpid(), SIGKILL));
        show("kill stop", kill(getpid(), SIGSTOP));
        show("tkill term", syscall(SYS_tkill, getpid(), SIGTERM));
        show("kill group", kill(0, SIGUSR1));
        show("kill all", kill(-1, 0));
        show("kill missing", kill(99999, 0));
        show("kill missing bad", kill(99999, 99));
        show("kill bad", kill(getpid(), 99));
        block(SIGTERM, SIG_BLOCK);
        show("kill blocked", kill(getpid(), SIGTERM));
        pending();
        signal(SIGTERM, SIG_IGN);
        pending();
        show("kill ignored", kill(getpid(), SIGTERM));
        pending();
        signal(SIGTERM, SIG_DFL);
        show("kill blocked again", kill(getpid(), SIGTERM));
        block(SIGTERM, SIG_UNBLOCK);
        pending();
        printf("still here\n");
    } else if (!strcmp(kind, "memory")) {
        limit(RLIMIT_AS, 1L << 30, RLIM_INFINITY);
        void *p = mmap(0, 2L << 30, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        show("mmap past address space", p == MAP_FAILED ? -1 : 0);
        limit(RLIMIT_AS, RLIM_INFINITY, RLIM_INFINITY);
        limit(RLIMIT_DATA, 16 << 20, RLIM_INFINITY);
        p = mmap(0, 32 << 20, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        show("mmap data past limit", p == MAP_FAILED ? -1 : 0);
        p = mmap(0, 32 << 20, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        show("mmap read only", p == MAP_FAILED ? -1 : 0);
        show("mprotect to data", mprotect(p, 32 << 20, PROT_READ | PROT_WRITE));
        show("mprotect a little", mprotect(p, 1 << 20, PROT_READ | PROT_WRITE));
        void *q = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
        show("mmap shared", q == MAP_FAILED ? -1 : 0);
        void *r = mremap(p, 1 << 20, 64 << 20, MREMAP_MAYMOVE);
        show("mremap past limit", r == MAP_FAILED ? -1 : 0);
        void *top = sbrk(0);
        show("sbrk past limit", sbrk(64 << 20) == (void *)-1 ? -1 : 0);
        show("break kept", sbrk(0) == top);
    } else if (!strcmp(kind, "stack")) {
        limit(RLIMIT_STACK, 1 << 20, RLIM_INFINITY);
        printf("within %d\n", grow(512 << 10));
        printf("beyond %d\n", grow(2 << 20));
    }
    return 0;
}
