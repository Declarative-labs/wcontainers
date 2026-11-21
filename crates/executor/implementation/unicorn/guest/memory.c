// Page protection and faults: each argument names one case; the program prints what it saw and may die
// of the fault the case provokes.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/uio.h>
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
    } else if (!strcmp(kind, "wrap")) {
        // Lengths in the last page wrap to 0 when rounded up; the others overflow the address. Raw
        // calls, since the C library refuses some lengths itself.
        char *p = mmap(0, 2 * page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        unsigned char vector[2];
        long lengths[] = {-1, -4095, -4096, -8192, 1L << 62};
        for (int i = 0; i < 5; i++) {
            long n = lengths[i];
            printf("length %ld:", n);
            printf(" msync %ld", syscall(SYS_msync, p, n, MS_ASYNC) ? -errno : 0L);
            printf(" msync_invalidate %ld", syscall(SYS_msync, p, n, MS_SYNC | MS_INVALIDATE) ? -errno : 0L);
            printf(" mlock %ld", syscall(SYS_mlock, p, n) ? -errno : 0L);
            printf(" mlock_offset %ld", syscall(SYS_mlock, p + 1, n) ? -errno : 0L);
            printf(" mlock2 %ld", syscall(SYS_mlock2, p, n, 1) ? -errno : 0L);
            printf(" munlock %ld", syscall(SYS_munlock, p, n) ? -errno : 0L);
            printf(" mprotect %ld", syscall(SYS_mprotect, p, n, PROT_READ) ? -errno : 0L);
            printf(" madvise %ld", syscall(SYS_madvise, p, n, MADV_NORMAL) ? -errno : 0L);
            printf(" mincore %ld", syscall(SYS_mincore, p, n, vector) ? -errno : 0L);
            printf(" munmap %ld", syscall(SYS_munmap, p, n) ? -errno : 0L);
            long mapped = syscall(SYS_mmap, 0, n, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
            printf(" mmap %ld", mapped == -1 ? -errno : 0L);
            long moved = syscall(SYS_mremap, p, n, page, 0, 0);
            printf(" mremap_old %ld", moved == -1 ? -errno : (long)(moved == (long)p));
            moved = syscall(SYS_mremap, p, page, n, MREMAP_MAYMOVE, 0);
            printf(" mremap_new %ld", moved == -1 ? -errno : (long)(moved == (long)p));
            printf("\n");
        }
        printf("restore %d\n", mprotect(p, 2 * page, PROT_READ | PROT_WRITE));
        p[0] = 1;
        p[page] = 2;
        printf("still mapped %d %d\n", p[0], p[page]);
    } else if (!strcmp(kind, "tagged")) {
        // Memory calls given an address with a top byte tag, which arm64 Linux strips for them.
        char *p = mmap(0, 3 * page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        unsigned char vector[1];
        unsigned long tags[] = {0x5aUL << 56, 0xffUL << 56, 0x80UL << 48};
        for (int i = 0; i < 3; i++) {
            char *t = (char *)((unsigned long)p | tags[i]);
            printf("tag %d:", i);
            printf(" msync %ld", syscall(SYS_msync, t, page, MS_ASYNC) ? -errno : 0L);
            printf(" mlock %ld", syscall(SYS_mlock, t, page) ? -errno : 0L);
            printf(" munlock %ld", syscall(SYS_munlock, t, page) ? -errno : 0L);
            printf(" madvise %ld", syscall(SYS_madvise, t, page, MADV_WILLNEED) ? -errno : 0L);
            printf(" mincore %ld", syscall(SYS_mincore, t, page, vector) ? -errno : 0L);
            printf(" mprotect %ld", syscall(SYS_mprotect, t, page, PROT_READ | PROT_WRITE) ? -errno : 0L);
            printf(" pkey_mprotect %ld",
                   syscall(SYS_pkey_mprotect, t, page, PROT_READ | PROT_WRITE, -1) ? -errno : 0L);
            long moved = syscall(SYS_mremap, t, page, page, 0, 0);
            printf(" mremap %ld", moved == -1 ? -errno : (long)(moved == (long)p));
            char copy[8];
            struct iovec local = {copy, sizeof copy}, remote = {t, sizeof copy};
            long read = syscall(SYS_process_vm_readv, getpid(), &local, 1, &remote, 1, 0);
            printf(" process_vm_readv %ld", read == -1 ? -errno : read);
            read = syscall(SYS_process_vm_readv, getpid(), &remote, 1, &local, 1, 0);
            printf(" process_vm_readv_local %ld", read == -1 ? -errno : read);
            printf("\n");
        }
        char *last = (char *)((unsigned long)(p + 2 * page) | tags[0]);
        printf("munmap %ld", syscall(SYS_munmap, last, page) ? -errno : 0L);
        printf(" then mincore %ld\n", syscall(SYS_mincore, p + 2 * page, page, vector) ? -errno : 0L);
    }
    printf("end\n");
    return 0;
}
