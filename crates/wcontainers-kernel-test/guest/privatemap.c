// Private file mappings: pages not yet written show the file as it changes (by write, by a shared
// mapping, by truncation); written pages are the process's own, including pages the kernel writes
// (read into a mapping), populated or locked pages, and pages copied before a fork; MADV_DONTNEED
// drops the copies; mremap takes copies along; truncation drops copies past the end (SIGBUS there).

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE 4096
#ifndef MADV_POPULATE_READ
#define MADV_POPULATE_READ 22
#define MADV_POPULATE_WRITE 23
#endif

static sigjmp_buf back;

static void on_fault(int signal, siginfo_t *info, void *context) {
    (void)info;
    (void)context;
    siglongjmp(back, signal);
}

// The first byte of each of `pages` pages, or the signal touching it raised.
static void show(const char *what, volatile unsigned char *map, int pages) {
    printf("%s:", what);
    for (int page = 0; page < pages; page++) {
        int signal = sigsetjmp(back, 1);
        if (signal == 0) {
            printf(" %c", map[page * PAGE] ? map[page * PAGE] : '0');
        } else {
            printf(" %s", signal == SIGBUS ? "BUS" : "SEGV");
        }
    }
    printf("\n");
}

static void put(int fd, int page, char byte) {
    if (pwrite(fd, &byte, 1, (off_t)page * PAGE) != 1) {
        printf("pwrite failed: %d\n", errno);
    }
}

static int file_of(int pages, char first) {
    int fd = open("/dev/shm/privatemap", O_RDWR | O_CREAT | O_TRUNC, 0600);
    for (int page = 0; page < pages; page++) {
        put(fd, page, (char)(first + page));
    }
    if (ftruncate(fd, (off_t)pages * PAGE) != 0) {
        printf("ftruncate failed: %d\n", errno);
    }
    return fd;
}

static void later_writes(void) {
    int fd = file_of(4, 'a');
    unsigned char *map = mmap(NULL, 4 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    show("mapped", map, 4);
    put(fd, 0, 'A');
    show("after pwrite to page 0", map, 4);
    map[1 * PAGE] = 'x';
    put(fd, 1, 'B');
    show("page 1 written, then pwrite to it", map, 4);
    // A shared mapping of the same file writes page 2.
    unsigned char *shared = mmap(NULL, 4 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    shared[2 * PAGE] = 'C';
    show("shared mapping wrote page 2", map, 4);
    // The kernel writes page 3 of the private mapping (read into it): its own copy now.
    char byte = 'r';
    int pipes[2];
    pipe(pipes);
    write(pipes[1], &byte, 1);
    printf("read into the mapping: %ld\n", (long)read(pipes[0], map + 3 * PAGE, 1));
    put(fd, 3, 'D');
    show("after read into page 3 and pwrite to it", map, 4);
    show("shared view", shared, 4);
    // Dropping the copies shows the file again.
    printf("madvise dontneed: %d\n", madvise(map, 4 * PAGE, MADV_DONTNEED));
    show("after MADV_DONTNEED", map, 4);
    munmap(shared, 4 * PAGE);
    munmap(map, 4 * PAGE);
    close(pipes[0]);
    close(pipes[1]);
    close(fd);
}

static void protections(void) {
    int fd = file_of(2, 'a');
    unsigned char *map = mmap(NULL, 2 * PAGE, PROT_READ, MAP_PRIVATE, fd, 0);
    put(fd, 0, 'P');
    show("read only, after pwrite", map, 2);
    printf("mprotect: %d\n", mprotect(map, 2 * PAGE, PROT_READ | PROT_WRITE));
    map[0] = 'w';
    put(fd, 0, 'Q');
    put(fd, 1, 'R');
    show("made writable, page 0 written, pwrite to both", map, 2);
    printf("mprotect: %d\n", mprotect(map, 2 * PAGE, PROT_READ));
    put(fd, 1, 'S');
    show("read only again", map, 2);
    munmap(map, 2 * PAGE);
    close(fd);
}

static void populated(void) {
    int fd = file_of(3, 'a');
    unsigned char *populated = mmap(NULL, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_POPULATE, fd, 0);
    unsigned char *read_only = mmap(NULL, 3 * PAGE, PROT_READ, MAP_PRIVATE | MAP_POPULATE, fd, 0);
    unsigned char *advised = mmap(NULL, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    printf("populate write: %d\n", madvise(advised + PAGE, PAGE, MADV_POPULATE_WRITE));
    printf("populate read: %d\n", madvise(advised + 2 * PAGE, PAGE, MADV_POPULATE_READ));
    unsigned char *locked = mmap(NULL, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    printf("mlock: %d\n", mlock(locked, PAGE));
    put(fd, 0, 'X');
    put(fd, 1, 'Y');
    put(fd, 2, 'Z');
    show("MAP_POPULATE writable", populated, 3);
    show("MAP_POPULATE read only", read_only, 3);
    show("MADV_POPULATE_WRITE page 1, READ page 2", advised, 3);
    show("mlock page 0", locked, 3);
    munmap(populated, 3 * PAGE);
    munmap(read_only, 3 * PAGE);
    munmap(advised, 3 * PAGE);
    munlock(locked, PAGE);
    munmap(locked, 3 * PAGE);
    close(fd);
}

static void truncation(void) {
    int fd = file_of(4, 'a');
    unsigned char *map = mmap(NULL, 4 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    map[2 * PAGE] = 'w';
    map[3 * PAGE] = 'v';
    map[0] = 'u';
    printf("ftruncate to 1.5 pages: %d\n", ftruncate(fd, PAGE + PAGE / 2));
    show("after shrinking", map, 4);
    printf("byte past the end in page 1: %d\n", map[PAGE + PAGE / 2 + 1]);
    printf("ftruncate to 4 pages: %d\n", ftruncate(fd, 4 * PAGE));
    show("after growing", map, 4);
    put(fd, 3, 'T');
    show("after pwrite to page 3", map, 4);
    // Punching a hole keeps copies.
    map[1 * PAGE] = 'h';
    printf("punch: %d\n", fallocate(fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE, 0, 4 * PAGE));
    show("after punching the whole file", map, 4);
    munmap(map, 4 * PAGE);
    close(fd);
}

static void moved(void) {
    int fd = file_of(3, 'a');
    unsigned char *map = mmap(NULL, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    map[PAGE] = 'm';
    unsigned char *to = mmap(NULL, 6 * PAGE, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    unsigned char *now = mremap(map, 3 * PAGE, 3 * PAGE, MREMAP_MAYMOVE | MREMAP_FIXED, to + PAGE);
    printf("moved to where asked: %d\n", now == to + PAGE);
    put(fd, 0, 'N');
    put(fd, 1, 'O');
    show("moved, after pwrite to pages 0 and 1", now, 3);
    now[2 * PAGE] = 'k';
    put(fd, 2, 'K');
    show("page 2 written after the move", now, 3);
    munmap(to, 6 * PAGE);
    close(fd);
}

static void forked(void) {
    int fd = file_of(3, 'a');
    unsigned char *map = mmap(NULL, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    map[0] = 'p';
    int ready[2], done[2];
    pipe(ready);
    pipe(done);
    pid_t child = fork();
    if (child == 0) {
        char byte;
        read(ready[0], &byte, 1);
        show("child, after the parent wrote page 1 and the file pages 0 to 2", map, 3);
        map[2 * PAGE] = 'c';
        write(done[1], "x", 1);
        _exit(0);
    }
    map[PAGE] = 'q';
    put(fd, 0, 'F');
    put(fd, 1, 'G');
    put(fd, 2, 'H');
    write(ready[1], "x", 1);
    char byte;
    read(done[0], &byte, 1);
    waitpid(child, NULL, 0);
    show("parent, after the child wrote page 2", map, 3);
    munmap(map, 3 * PAGE);
    close(fd);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    struct sigaction action = {0};
    action.sa_sigaction = on_fault;
    action.sa_flags = SA_SIGINFO;
    sigaction(SIGBUS, &action, NULL);
    sigaction(SIGSEGV, &action, NULL);
    later_writes();
    protections();
    populated();
    truncation();
    moved();
    forked();
    unlink("/dev/shm/privatemap");
    return 0;
}
