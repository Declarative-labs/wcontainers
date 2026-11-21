// Mappings of backend files the backend opens for the page cache: every mapping, read and write of
// a file sees the same bytes, shared changes reach the backend, and the file's end moves the
// mappings' end.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static sigjmp_buf back;

static void caught(int signal) { siglongjmp(back, signal); }

// The byte at `at`, or the signal reading it raised.
static int peek(volatile char *at) {
    int signal = sigsetjmp(back, 1);
    if (signal) return -signal;
    return *at;
}

static char file_byte(const char *name, off_t at) {
    int fd = open(name, O_RDONLY);
    char byte = '?';
    pread(fd, &byte, 1, at);
    close(fd);
    return byte;
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    signal(SIGBUS, caught);
    signal(SIGSEGV, caught);
    int fd = open("shared", O_CREAT | O_RDWR | O_TRUNC, 0644);
    char page[3 * 4096];
    for (int i = 0; i < (int)sizeof page; i++) page[i] = 'a' + i / 4096;
    write(fd, page, 2 * 4096 + 100);

    char *shared = mmap(0, 3 * 4096, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    printf("shared mapping: %s\n", shared == MAP_FAILED ? strerror(errno) : "made");
    if (shared == MAP_FAILED) return 1;
    char *private = mmap(0, 2 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    printf("private mapping: %s\n", private == MAP_FAILED ? strerror(errno) : "made");
    if (private == MAP_FAILED) return 1;

    shared[0] = 'S';
    char byte = 0;
    pread(fd, &byte, 1, 0);
    printf("read sees the mapping's write: %c\n", byte);
    printf("private sees the shared write: %c\n", private[0]);
    pwrite(fd, "W", 1, 4096);
    printf("mapping sees the file's write: %c, private: %c\n", shared[4096], private[4096]);
    private[4096] = 'p';
    pwrite(fd, "X", 1, 4097);
    printf("private copy keeps its own: %c %c\n", private[4096], private[4097]);
    madvise(private, 2 * 4096, MADV_DONTNEED);
    printf("private dropped shows the file: %c %c\n", private[4096], private[4097]);
    printf("past the end in its page: %d %d\n", peek(shared + 2 * 4096 + 200),
           peek(shared + 3 * 4096 - 1));

    // Another descriptor's mapping of the same file, made apart from this one.
    int other = open("shared", O_RDWR);
    char *again = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, other, 0);
    again[1] = 'A';
    printf("two mappings share: %c\n", shared[1]);
    munmap(again, 4096);
    close(other);

    if (fork() == 0) {
        shared[2] = 'C';
        _exit(0);
    }
    wait(0);
    printf("child's shared write seen: %c, read: %c\n", shared[2], file_byte("shared", 2));

    printf("msync: %d\n", msync(shared, 3 * 4096, MS_SYNC));
    printf("fsync: %d\n", fsync(fd));

    // Shrinking the file: pages past the end fault, the rest stay; growing brings zeros back.
    printf("truncate: %d\n", ftruncate(fd, 4096 + 10));
    printf("after shrinking: %c %d %d\n", shared[4096], peek(shared + 4096 + 10),
           peek(shared + 2 * 4096));
    printf("grow: %d\n", ftruncate(fd, 3 * 4096));
    printf("after growing: %d %d\n", peek(shared + 4096 + 20), peek(shared + 2 * 4096 + 5));
    shared[2 * 4096 + 5] = 'G';
    printf("read past the old end: %c\n", file_byte("shared", 2 * 4096 + 5));

    // Growing the mapping reads the file's next pages.
    pwrite(fd, page, 4096, 3 * 4096);
    char *grown = mremap(private, 2 * 4096, 4 * 4096, MREMAP_MAYMOVE);
    printf("mremap grows: %d\n", grown != MAP_FAILED);
    if (grown != MAP_FAILED) printf("grown reads: %c %c\n", grown[2 * 4096 + 5], grown[3 * 4096]);

    // A write through a mapping reaches the backend when it is unmapped, or its process ends.
    shared[3] = 'U';
    munmap(shared, 3 * 4096);
    printf("unmapped write in the file: %c\n", file_byte("shared", 3));
    if (fork() == 0) {
        char *child = mmap(0, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
        child[4] = 'E';
        _exit(0);
    }
    wait(0);
    printf("exited child's write in the file: %c\n", file_byte("shared", 4));
    struct stat st;
    fstat(fd, &st);
    printf("size: %lld\n", (long long)st.st_size);
    close(fd);
    // With no mapping left, the bytes are the backend's.
    if (grown != MAP_FAILED) munmap(grown, 4 * 4096);
    printf("bytes: %c%c%c%c%c %c\n", file_byte("shared", 0), file_byte("shared", 1),
           file_byte("shared", 2), file_byte("shared", 3), file_byte("shared", 4),
           file_byte("shared", 2 * 4096 + 5));
    return 0;
}
