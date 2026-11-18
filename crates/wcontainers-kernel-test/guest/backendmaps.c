// Mappings of files in the working directory, which is on the file backend, whose pages are read
// when first touched: far into a large sparse file, by the program and by the kernel's own copies
// (a path, a write from mapped bytes, a read into them), shared and private, across a fork, and
// with the file changed through the kernel while mapped.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE 4096L
#define GIB (1L << 30)

static unsigned long sum(const unsigned char *bytes, long length) {
    unsigned long total = 0;
    for (long i = 0; i < length; i++) {
        total = total * 31 + bytes[i];
    }
    return total;
}

static void marker(int fd, long offset, const char *text) {
    if (pwrite(fd, text, strlen(text), offset) != (ssize_t)strlen(text)) {
        printf("pwrite at %ld: %s\n", offset, strerror(errno));
    }
}

static sigjmp_buf back;
static void caught(int signal) { siglongjmp(back, signal); }

int main(void) {
    // A sparse file past 5 GiB with marks around 2 GiB, 4 GiB and 5 GiB.
    int fd = open("large", O_RDWR | O_CREAT | O_TRUNC, 0644);
    marker(fd, 2 * GIB - 6, "  DEARdear  ");
    marker(fd, 4 * GIB - 6, "  DEARdear  ");
    marker(fd, 5 * GIB + 0xFFFFFFF, "!");
    printf("size %ld\n", (long)lseek(fd, 0, SEEK_END));
    unsigned char *far = mmap(NULL, 0x10000000, PROT_READ, MAP_SHARED, fd, 5 * GIB);
    printf("far map %d byte %d first %d\n", far != MAP_FAILED, far[0xFFFFFFF], far[0]);
    for (long boundary = 2 * GIB; boundary <= 4 * GIB; boundary += 2 * GIB) {
        unsigned char *around = mmap(NULL, 2 * PAGE, PROT_READ, MAP_PRIVATE, fd, boundary - PAGE);
        printf("around %ldG %.12s\n", boundary / GIB, around + PAGE - 6);
        munmap(around, 2 * PAGE);
    }
    // A gigabyte mapped, a few pages touched.
    unsigned char *big = mmap(NULL, GIB, PROT_READ, MAP_SHARED, fd, GIB);
    printf("big %d %.4s %d\n", big[GIB - 4], (char *)big + GIB - 4, big[GIB / 2]);
    munmap(big, GIB);
    munmap(far, 0x10000000);
    close(fd);
    unlink("large");

    // A file of 64 pages, each page's bytes from its number.
    int data = open("data", O_RDWR | O_CREAT | O_TRUNC, 0644);
    static unsigned char page[PAGE];
    for (int i = 0; i < 64; i++) {
        for (long j = 0; j < PAGE; j++) {
            page[j] = (unsigned char)(i * 7 + j);
        }
        if (i == 40) {
            strcpy((char *)page + 100, "data");
        }
        write(data, page, PAGE);
    }
    unsigned char *shared = mmap(NULL, 64 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, data, 0);
    // The kernel reads mapped bytes no instruction touched: a path, then a write from them.
    int again = open((char *)shared + 40 * PAGE + 100, O_RDONLY);
    printf("path from the mapping %d\n", again >= 0);
    close(again);
    int pipes[2];
    pipe(pipes);
    printf("write from the mapping %zd\n", write(pipes[1], shared + 50 * PAGE, 3 * PAGE));
    static unsigned char got[3 * PAGE];
    read(pipes[0], got, sizeof got);
    printf("piped %lx\n", sum(got, sizeof got));
    struct iovec parts[2] = {{shared + 20 * PAGE, 100}, {shared + 60 * PAGE + 4000, 200}};
    printf("writev %zd\n", writev(pipes[1], parts, 2));
    read(pipes[0], got, 300);
    printf("vectors %lx\n", sum(got, 300));
    // The kernel writes into pages no instruction touched, then the program and read() see them.
    memset(got, 0x5a, PAGE);
    write(pipes[1], got, PAGE);
    printf("read into the mapping %zd\n", read(pipes[0], shared + 30 * PAGE + 10, PAGE));
    printf("shared %lx\n", sum(shared + 30 * PAGE, PAGE));
    pread(data, got, PAGE, 30 * PAGE);
    printf("file %lx\n", sum(got, PAGE));
    // Written through the file while mapped, untouched pages and touched ones.
    printf("first touch %d\n", shared[5 * PAGE]);
    memset(page, 0x33, PAGE);
    pwrite(data, page, 2 * 100, 5 * PAGE - 100);
    pwrite(data, page, 100, 9 * PAGE + 7);
    printf("after writes %lx %lx\n", sum(shared + 4 * PAGE, 2 * PAGE), sum(shared + 9 * PAGE, PAGE));
    // A child reads what the parent never touched, and writes some.
    pid_t child = fork();
    if (child == 0) {
        printf("child %lx\n", sum(shared + 12 * PAGE, 4 * PAGE));
        shared[13 * PAGE] = 0xee;
        _exit(0);
    }
    waitpid(child, NULL, 0);
    printf("parent sees %x\n", shared[13 * PAGE]);
    msync(shared, 64 * PAGE, MS_SYNC);
    pread(data, got, PAGE, 13 * PAGE);
    printf("file after msync %x %lx\n", got[0], sum(got, PAGE));
    // A private writable mapping: its own copies, the file's bytes elsewhere.
    unsigned char *own = mmap(NULL, 64 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, data, 0);
    own[45 * PAGE] = 1;
    write(pipes[1], "abc", 3);
    read(pipes[0], own + 46 * PAGE + 5, 3);
    pread(data, got, PAGE, 46 * PAGE);
    printf("private %lx %lx %lx file %lx\n", sum(own + 45 * PAGE, PAGE), sum(own + 46 * PAGE, PAGE),
           sum(own + 47 * PAGE, PAGE), sum(got, PAGE));
    // Shrunk while mapped: past the end is SIGBUS, before it the file's bytes.
    ftruncate(data, 32 * PAGE + 10);
    signal(SIGBUS, caught);
    int signal_number = sigsetjmp(back, 1);
    if (signal_number == 0) {
        printf("before the end %d\n", shared[32 * PAGE + 9]);
        printf("past the end %d\n", shared[34 * PAGE]);
    } else {
        printf("signal %d\n", signal_number);
    }
    munmap(own, 64 * PAGE);
    munmap(shared, 64 * PAGE);
    close(data);
    unlink("data");
    return 0;
}
