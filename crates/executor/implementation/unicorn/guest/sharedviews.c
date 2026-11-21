// One shared file shown at two places of one process, written through one view and read or run
// through the other with no system call between. The argument picks the part: "data", "ring" (a ring
// of two pages mapped twice side by side), "ring1" (a ring of one page) or "code".
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE 4096

static int make_file(size_t size) {
    int fd = syscall(SYS_memfd_create, "views", 0);
    if (fd < 0 || ftruncate(fd, size) != 0) {
        return -1;
    }
    return fd;
}

static int data(void) {
    int fd = make_file(2 * PAGE);
    volatile unsigned char *a = mmap(0, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    volatile unsigned char *b = mmap(0, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (a == MAP_FAILED || b == MAP_FAILED) {
        printf("map failed\n");
        return 1;
    }
    unsigned seen[6];
    a[10] = 0x11;
    seen[0] = b[10];
    b[20] = 0x22;
    seen[1] = a[20];
    a[10] = 0x33;
    seen[2] = b[10];
    unsigned sum = 0;
    for (int i = 0; i < 2 * PAGE; i++) {
        a[i] = (unsigned char)(i * 3 + 1);
        sum += b[i];
    }
    seen[3] = sum;
    seen[4] = b[PAGE + 7];
    b[PAGE + 7] = 0x5a;
    seen[5] = a[PAGE + 7];
    printf("views: %02x %02x %02x sum %u %02x %02x\n", seen[0], seen[1], seen[2], seen[3], seen[4],
           seen[5]);

    // A child writes through one view; the parent reads through the other after waiting.
    pid_t child = fork();
    if (child == 0) {
        a[100] = 0x77;
        _exit(b[100] == 0x77 ? 0 : 1);
    }
    int status = 0;
    waitpid(child, &status, 0);
    printf("after the child: %02x %02x status %d\n", a[100], b[100], WEXITSTATUS(status));
    return 0;
}

static int ring(int pages) {
    size_t size = pages * PAGE;
    int fd = make_file(size);
    unsigned char *area = mmap(0, 2 * size, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    unsigned char *first = mmap(area, size, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_FIXED, fd, 0);
    unsigned char *second =
        mmap(area + size, size, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_FIXED, fd, 0);
    if (first == MAP_FAILED || second == MAP_FAILED) {
        printf("ring failed\n");
        return 1;
    }
    memcpy(area + size - 8, "0123456789abcdef", 16);
    asm volatile("" ::: "memory");
    char head[9];
    memcpy(head, area, 8);
    head[8] = 0;
    char across[17];
    memcpy(across, area + size - 8, 16);
    across[16] = 0;
    printf("ring: head %s across %s\n", head, across);
    return 0;
}

// mov w0, #value; ret
static void put_code(volatile uint32_t *at, unsigned value) {
    at[0] = 0x52800000u | ((value & 0xffff) << 5);
    at[1] = 0xd65f03c0u;
}

static void sync_code(void *from, size_t length) {
    __builtin___clear_cache((char *)from, (char *)from + length);
}

static int code(void) {
    // Written through a writable view, run through an executable view of the same file.
    int fd = make_file(PAGE);
    volatile uint32_t *w = mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    void *x = mmap(0, PAGE, PROT_READ | PROT_EXEC, MAP_SHARED, fd, 0);
    if (w == MAP_FAILED || x == MAP_FAILED) {
        printf("code views failed\n");
        return 1;
    }
    int (*f)(void) = (int (*)(void))x;
    int got[3];
    for (int i = 0; i < 3; i++) {
        put_code(w, i + 1);
        sync_code(x, 8);
        got[i] = f();
    }
    printf("code views: %d %d %d\n", got[0], got[1], got[2]);

    // Shared anonymous code, rewritten by a child and run again by the parent. The child does not run
    // what it wrote: on the reference it was seen to run the old code then, with the parent seeing the
    // new.
    volatile uint32_t *s =
        mmap(0, PAGE, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    if (s == MAP_FAILED) {
        printf("shared code failed\n");
        return 1;
    }
    int (*g)(void) = (int (*)(void))(uintptr_t)s;
    put_code(s, 7);
    sync_code((void *)s, 8);
    int before = g();
    pid_t child = fork();
    if (child == 0) {
        int theirs = g();
        put_code(s, 8);
        sync_code((void *)s, 8);
        _exit(theirs);
    }
    int status = 0;
    waitpid(child, &status, 0);
    int after = g();
    printf("shared code: parent %d child %d parent after %d\n", before, WEXITSTATUS(status), after);
    return 0;
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    const char *part = argc > 1 ? argv[1] : "";
    if (strcmp(part, "data") == 0) {
        return data();
    }
    if (strcmp(part, "ring") == 0) {
        return ring(2);
    }
    if (strcmp(part, "ring1") == 0) {
        return ring(1);
    }
    if (strcmp(part, "code") == 0) {
        return code();
    }
    printf("no part %s\n", part);
    return 2;
}
