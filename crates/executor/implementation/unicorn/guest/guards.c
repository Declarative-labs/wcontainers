// Guard pages (MADV_GUARD_INSTALL and MADV_GUARD_REMOVE): what touching one does, what the
// kernel's own copies make of them, and how they last through protection changes, dropping,
// moving, forking, locking and remapping.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/mman.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <unistd.h>

#ifndef MADV_POPULATE_READ
#define MADV_POPULATE_READ 22
#define MADV_POPULATE_WRITE 23
#endif
#ifndef MADV_GUARD_INSTALL
#define MADV_GUARD_INSTALL 102
#define MADV_GUARD_REMOVE 103
#endif

#define R(call)                                                                                    \
    do {                                                                                           \
        long r_ = (long)(call);                                                                    \
        printf("%-64s %ld %s\n", #call, r_, r_ < 0 ? strerror(errno) : "");                         \
    } while (0)

static long page;
static sigjmp_buf back;
static volatile int code;

static void on_fault(int signal, siginfo_t *info, void *context) {
    (void)context;
    code = signal * 100 + info->si_code;
    siglongjmp(back, 1);
}

// Reads (or writes) one byte, saying what came of it: the byte, or the signal and its code.
static void touch(const char *name, volatile char *at, int write) {
    code = 0;
    int got = -1;
    if (sigsetjmp(back, 1) == 0) {
        if (write)
            *at = 'w';
        got = *at;
    }
    if (code)
        printf("%s: signal %d code %d\n", name, code / 100, code % 100);
    else
        printf("%s: byte %d\n", name, got);
}

// The same in a child, which ends as the touch leaves it.
static void touch_in_child(const char *name, volatile char *at) {
    pid_t child = fork();
    if (child == 0) {
        signal(SIGSEGV, SIG_DFL);
        (void)*at;
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("%s: child %s %d\n", name, WIFSIGNALED(status) ? "killed by" : "ended",
           WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status));
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    page = sysconf(_SC_PAGESIZE);
    struct sigaction action = {0};
    action.sa_sigaction = on_fault;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGSEGV, &action, NULL);
    sigaction(SIGBUS, &action, NULL);

    char *anon = mmap(NULL, 4 * page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    memset(anon, 'a', 4 * page);
    R(madvise(anon + page, 2 * page, MADV_GUARD_INSTALL));
    touch("first page", anon, 0);
    touch("guarded read", anon + page, 0);
    touch("guarded write", anon + 2 * page + 7, 1);
    touch("last page", anon + 3 * page, 0);
    touch_in_child("guarded in a child", anon + page);

    // The kernel's own copies.
    int pipes[2];
    pipe(pipes);
    R(write(pipes[1], anon + page, 4));
    R(write(pipes[1], anon + page - 2, 4));
    write(pipes[1], "zz", 2);
    R(read(pipes[0], anon + page, 4));
    int mem = open("/proc/self/mem", O_RDWR);
    char byte;
    R(pread(mem, &byte, 1, (off_t)(anon + page)));
    struct iovec local = {&byte, 1}, remote = {anon + page, 1};
    R(process_vm_readv(getpid(), &local, 1, &remote, 1, 0));
    R(madvise(anon, 4 * page, MADV_POPULATE_READ));
    R(madvise(anon, 4 * page, MADV_POPULATE_WRITE));

    // Through protection changes and dropping, then removed: the pages were dropped.
    R(mprotect(anon, 4 * page, PROT_NONE));
    R(mprotect(anon, 4 * page, PROT_READ | PROT_WRITE));
    touch("after mprotect", anon + page, 0);
    R(madvise(anon, 4 * page, MADV_DONTNEED));
    touch("after dontneed", anon + page, 0);
    touch("dropped page", anon, 0);
    R(madvise(anon + page, page, MADV_GUARD_REMOVE));
    touch("removed", anon + page, 0);
    touch("still guarded", anon + 2 * page, 0);
    R(madvise(anon, 4 * page, MADV_GUARD_REMOVE));
    touch("all removed", anon + 2 * page, 0);

    // Moving takes them along.
    R(madvise(anon + page, page, MADV_GUARD_INSTALL));
    char *moved = mremap(anon, 4 * page, 4 * page, MREMAP_MAYMOVE | MREMAP_FIXED, anon + 16 * page);
    printf("moved %d\n", moved == anon + 16 * page);
    touch("moved guard", moved + page, 0);
    touch("moved neighbour", moved + 2 * page, 0);

    // Unmapped and mapped again, none is left.
    R(munmap(moved, 4 * page));
    char *again = mmap(moved, 4 * page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED,
                       -1, 0);
    touch("mapped again", again + page, 0);

    // A child keeps them, unless the memory is wiped on fork.
    R(madvise(again + page, page, MADV_GUARD_INSTALL));
    touch_in_child("forked guard", again + page);
    R(madvise(again, 4 * page, MADV_WIPEONFORK));
    touch_in_child("wiped guard", again + page);

    // Locked memory, shared memory, a file's private pages, gaps and the kernel's pages.
    char *locked = mmap(NULL, 2 * page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    R(mlock(locked, 2 * page));
    R(madvise(locked, page, MADV_GUARD_INSTALL));
    R(madvise(locked, page, MADV_GUARD_REMOVE));
    char *shared = mmap(NULL, 2 * page, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    memset(shared, 's', 2 * page);
    R(madvise(shared, page, MADV_GUARD_INSTALL));
    touch("shared guard", shared, 0);
    R(madvise(shared, page, MADV_GUARD_REMOVE));
    touch("shared after", shared, 0);
    int file = open("/tmp/guarded", O_RDWR | O_CREAT | O_TRUNC, 0644);
    char text[64];
    memset(text, 'f', sizeof text);
    write(file, text, sizeof text);
    char *private = mmap(NULL, page, PROT_READ | PROT_WRITE, MAP_PRIVATE, file, 0);
    private[0] = 'p';
    R(madvise(private, page, MADV_GUARD_INSTALL));
    R(madvise(private, page, MADV_GUARD_REMOVE));
    touch("private file after", private, 0);
    char *gap = mmap(NULL, 3 * page, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    munmap(gap + page, page);
    R(madvise(gap, 3 * page, MADV_GUARD_INSTALL));
    touch("before the gap", gap, 0);
    touch("after the gap", gap + 2 * page, 0);
    R(madvise(gap + 1, page, MADV_GUARD_INSTALL));
    char *vdso = (char *)getauxval(AT_SYSINFO_EHDR);
    R(madvise(vdso, page, MADV_GUARD_INSTALL));
    unlink("/tmp/guarded");
    return 0;
}
