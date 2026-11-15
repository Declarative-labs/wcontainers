// Page protection and faults: each argument names one case; the program prints what it saw and may die
// of the fault the case provokes.

#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

static volatile char sink;

static int depth(int n) {
    volatile char frame[4096];
    frame[0] = (char)n;
    if (n == 0) {
        return frame[0];
    }
    return depth(n - 1) + frame[0];
}

static int forever(int n) {
    volatile char frame[4096];
    frame[0] = (char)n;
    return forever(n + 1) + frame[0];
}

int main(int argc, char **argv) {
    const char *kind = argc > 1 ? argv[1] : "";
    setvbuf(stdout, NULL, _IONBF, 0);
    long page = sysconf(_SC_PAGESIZE);
    if (!strcmp(kind, "protect")) {
        char *p = mmap(0, page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        p[0] = 1;
        printf("mprotect %d\n", mprotect(p, page, PROT_READ));
        printf("read %d\n", p[0]);
        p[0] = 2;
    } else if (!strcmp(kind, "none")) {
        char *p = mmap(0, page, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        printf("mapped\n");
        sink = p[0];
    } else if (!strcmp(kind, "unmap")) {
        char *p = mmap(0, 3 * page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        p[0] = p[2 * page] = 5;
        printf("munmap %d\n", munmap(p + page, page));
        printf("ends %d %d\n", p[0], p[2 * page]);
        sink = p[page];
    } else if (!strcmp(kind, "zero")) {
        // Fresh memory is zero, also where something was mapped before.
        char *p = mmap(0, page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        memset(p, 7, page);
        munmap(p, page);
        char *q = mmap(p, page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
        int sum = 0;
        for (long i = 0; i < page; i++) {
            sum += q[i];
        }
        printf("same %d sum %d\n", q == p, sum);
        memset(q, 9, page);
        printf("dontneed %d first %d\n", madvise(q, page, MADV_DONTNEED), q[0]);
    } else if (!strcmp(kind, "beyond")) {
        int fd = open("/dev/shm/file", O_CREAT | O_RDWR, 0644);
        write(fd, "hello", 5);
        char *p = mmap(0, 2 * page, PROT_READ, MAP_PRIVATE, fd, 0);
        printf("%.5s %d %d\n", p, p[5], p[page - 1]);
        sink = p[page];
    } else if (!strcmp(kind, "private")) {
        int fd = open("/dev/shm/file", O_CREAT | O_RDWR, 0644);
        write(fd, "hello", 5);
        char *p = mmap(0, page, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
        p[0] = 'j';
        char back[6] = {0};
        pread(fd, back, 5, 0);
        printf("%.5s %s\n", p, back);
    } else if (!strcmp(kind, "grow")) {
        printf("depth %d\n", depth(1000) & 0xff);
    } else if (!strcmp(kind, "overflow")) {
        printf("start\n");
        forever(0);
    } else if (!strcmp(kind, "brk")) {
        char *start = sbrk(0);
        char *grown = sbrk(3 * page);
        grown[3 * page - 1] = 1;
        char *end = sbrk(0);
        sbrk(-2 * page);
        printf("grown %ld moved %d shrunk %ld\n", (long)(end - start), grown == start, (long)((char *)sbrk(0) - start));
    } else if (!strcmp(kind, "execute")) {
        // `ret`, run from a page made executable, then from one that no longer is.
        unsigned *code = mmap(0, page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        code[0] = 0xd65f03c0;
        printf("mprotect %d\n", mprotect(code, page, PROT_READ | PROT_EXEC));
        ((void (*)(void))code)();
        printf("ran\n");
        mprotect(code, page, PROT_READ);
        ((void (*)(void))code)();
    }
    printf("end\n");
    return 0;
}
