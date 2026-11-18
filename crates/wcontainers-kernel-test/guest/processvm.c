// process_vm_readv and process_vm_writev on the caller's own memory: the checks, partial copies at
// pages the other side may not reach, and faults on the caller's side; then on a child's memory,
// as the same user and as another.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE 4096

static long vm(int write, long pid, struct iovec *local, unsigned long local_count, struct iovec *remote,
               unsigned long remote_count, unsigned long flags) {
    long result = syscall(write ? SYS_process_vm_writev : SYS_process_vm_readv, pid, local, local_count, remote,
                          remote_count, flags);
    return result < 0 ? -errno : result;
}

static unsigned sum(const unsigned char *bytes, size_t length) {
    unsigned total = 0;
    for (size_t at = 0; at < length; at++) {
        total = total * 31 + bytes[at];
    }
    return total;
}

static void show(const char *what, long result, const unsigned char *bytes, size_t length) {
    printf("%s: %ld sum %08x\n", what, result, sum(bytes, length));
}

// A child that keeps its copy of `area` (and a read-only page after it) until told to look, then
// prints what its memory holds; as `user` if not 0.
static void other_process(unsigned char *area, unsigned char *into) {
    for (int user = 0; user < 2; user++) {
        int go[2], ready[2];
        pipe(go);
        pipe(ready);
        fflush(stdout);
        pid_t child = fork();
        if (child == 0) {
            if (user)
                setresuid(65534, 65534, 65534);
            for (int at = 0; at < 4 * PAGE; at++)
                area[at] = (unsigned char)(at * 5 + user);
            mprotect(area + 4 * PAGE, PAGE, PROT_READ);
            write(ready[1], "r", 1);
            char byte;
            read(go[0], &byte, 1);
            printf("child %d holds sum %08x\n", user, sum(area, 5 * PAGE));
            _exit(0);
        }
        char byte;
        read(ready[0], &byte, 1);
        memset(into, 0, 8 * PAGE);
        struct iovec local[2] = {{into, 1000}, {into + 2000, 3 * PAGE}};
        struct iovec remote[2] = {{area + 100, 2 * PAGE}, {area + 3 * PAGE, 3 * PAGE}};
        show(user ? "read another user's child" : "read a child", vm(0, child, local, 2, remote, 2, 0), into, 8 * PAGE);
        for (int at = 0; at < 8 * PAGE; at++)
            into[at] = (unsigned char)(at * 3 + 1);
        show(user ? "write another user's child" : "write a child", vm(1, child, local, 2, remote, 2, 0), into, 8 * PAGE);
        write(go[1], "g", 1);
        waitpid(child, NULL, 0);
        close(go[0]);
        close(go[1]);
        close(ready[0]);
        close(ready[1]);
    }
}

int main(void) {
    long self = getpid();
    unsigned char *area = mmap(0, 8 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    unsigned char *into = mmap(0, 8 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    for (int at = 0; at < 8 * PAGE; at++) {
        area[at] = (unsigned char)(at * 7 + 3);
    }
    struct iovec local[3] = {{into, 100}, {into + 200, 5000}, {into + 6000, 300}};
    struct iovec remote[2] = {{area + 10, 3000}, {area + 4000, 9000}};

    // The checks, in Linux's order.
    show("flags", vm(0, self, local, 3, remote, 2, 1), into, 8 * PAGE);
    show("too many local", vm(0, self, local, 1025, remote, 2, 0), into, 8 * PAGE);
    show("too many remote", vm(0, self, local, 3, remote, 1025, 0), into, 8 * PAGE);
    show("no local", vm(0, self, local, 0, (struct iovec *)8, 1025, 0), into, 8 * PAGE);
    show("bad local list", vm(0, self, (struct iovec *)8, 1, remote, 2, 0), into, 8 * PAGE);
    show("bad remote list", vm(0, self, local, 3, (struct iovec *)8, 1, 0), into, 8 * PAGE);
    struct iovec empty[2] = {{area, 0}, {0, 0}};
    show("empty remote", vm(0, 99999, local, 3, empty, 2, 0), into, 8 * PAGE);
    show("no such process", vm(0, 99999, local, 3, remote, 2, 0), into, 8 * PAGE);
    show("process 0", vm(0, 0, local, 3, remote, 2, 0), into, 8 * PAGE);
    show("process -1", vm(0, -1, local, 3, remote, 2, 0), into, 8 * PAGE);
    struct iovec negative[1] = {{area, (size_t)-1}};
    show("negative local", vm(0, self, negative, 1, remote, 2, 0), into, 8 * PAGE);
    show("negative remote", vm(0, self, local, 3, negative, 1, 0), into, 8 * PAGE);
    struct iovec high[1] = {{(void *)(1UL << 48), 10}};
    show("high local", vm(0, self, high, 1, remote, 2, 0), into, 8 * PAGE);
    show("high remote", vm(0, self, local, 3, high, 1, 0), into, 8 * PAGE);
    struct iovec null_remote[1] = {{0, 10}};
    show("null remote", vm(0, self, local, 3, null_remote, 1, 0), into, 8 * PAGE);

    // Whole copies, by process and by thread number.
    show("read", vm(0, self, local, 3, remote, 2, 0), into, 8 * PAGE);
    memset(into, 0, 8 * PAGE);
    show("read by thread", vm(0, syscall(SYS_gettid), local, 3, remote, 2, 0), into, 8 * PAGE);
    memset(into, 0x55, 8 * PAGE);
    struct iovec out[2] = {{into + 1, 4000}, {into + 4500, 3000}};
    struct iovec from[1] = {{area + 100, 7000}};
    show("write", vm(1, self, from, 1, out, 2, 0), into, 8 * PAGE);

    // The other side stops at a page it may not reach.
    mprotect(area + 3 * PAGE, PAGE, PROT_NONE);
    memset(into, 0, 8 * PAGE);
    struct iovec across[1] = {{area + 2 * PAGE + 100, 3 * PAGE}};
    struct iovec big[1] = {{into, 8 * PAGE}};
    show("read to no access", vm(0, self, big, 1, across, 1, 0), into, 8 * PAGE);
    struct iovec at_none[2] = {{area + 3 * PAGE + 5, 10}, {area, 10}};
    show("read at no access", vm(0, self, big, 1, at_none, 2, 0), into, 8 * PAGE);
    struct iovec after_good[2] = {{area, 10}, {area + 3 * PAGE, 10}};
    show("read after good", vm(0, self, big, 1, after_good, 2, 0), into, 8 * PAGE);
    mprotect(area + 3 * PAGE, PAGE, PROT_READ);
    show("write to read only", vm(1, self, big, 1, across, 1, 0), area, 3 * PAGE);
    mprotect(area + 3 * PAGE, PAGE, PROT_WRITE);
    show("read write only", vm(0, self, big, 1, across, 1, 0), into, 8 * PAGE);
    show("write write only", vm(1, self, big, 1, across, 1, 0), area, 3 * PAGE);
    mprotect(area + 3 * PAGE, PAGE, PROT_EXEC);
    show("read execute only", vm(0, self, big, 1, across, 1, 0), into, 8 * PAGE);
    mprotect(area + 3 * PAGE, PAGE, PROT_READ | PROT_WRITE);
    munmap(area + 5 * PAGE, PAGE);
    struct iovec to_gap[1] = {{area + 4 * PAGE, 2 * PAGE}};
    show("read to gap", vm(0, self, big, 1, to_gap, 1, 0), into, 8 * PAGE);

    // The caller's own side faults part way.
    mprotect(into + 2 * PAGE, PAGE, PROT_READ);
    memset(into, 0, 2 * PAGE);
    struct iovec three[3] = {{into + 10, 100}, {into + PAGE + 3000, 3000}, {into + 3 * PAGE, 100}};
    struct iovec plenty[1] = {{area, 4 * PAGE}};
    show("local read only", vm(0, self, three, 3, plenty, 1, 0), into, 2 * PAGE);
    struct iovec first_bad[1] = {{into + 2 * PAGE, 100}};
    show("local read only first", vm(0, self, first_bad, 1, plenty, 1, 0), into, 2 * PAGE);
    mprotect(into + 2 * PAGE, PAGE, PROT_NONE);
    struct iovec source[2] = {{into + 2 * PAGE - 50, 100}, {into, 10}};
    struct iovec target[1] = {{area, 200}};
    show("local unreadable", vm(1, self, source, 2, target, 1, 0), area, 2 * PAGE);
    mprotect(into + 2 * PAGE, PAGE, PROT_READ | PROT_WRITE);

    // Past the end of a mapped file.
    int fd = memfd_create("file", 0);
    write(fd, area, 100);
    unsigned char *file = mmap(0, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    struct iovec past[1] = {{file + 50, 2 * PAGE}};
    show("read past file end", vm(0, self, big, 1, past, 1, 0), into, 8 * PAGE);
    show("write past file end", vm(1, self, big, 1, past, 1, 0), file, 100);

    // Below the stack, which another task's access never grows.
    unsigned char *stack = (unsigned char *)&fd;
    struct iovec below[1] = {{stack - 4 * 1024 * 1024, 10}};
    show("read below stack", vm(0, self, big, 1, below, 1, 0), into, 8 * PAGE);

    // Both sides in one buffer, overlapping.
    for (int at = 0; at < PAGE; at++) {
        into[at] = (unsigned char)at;
    }
    struct iovec ahead[1] = {{into + 1, 2000}};
    struct iovec behind[1] = {{into, 2000}};
    show("overlap forward", vm(0, self, ahead, 1, behind, 1, 0), into, PAGE);
    show("overlap backward", vm(0, self, behind, 1, ahead, 1, 0), into, PAGE);
    // Overlaps of many sizes and distances, in both directions.
    static const int sizes[] = {3, 15, 16, 17, 40, 63, 64, 65, 100, 127, 128, 129, 200, 300, 1000};
    static const int shifts[] = {1, 2, 3, 7, 8, 15, 16, 17, 24, 33, 64, 100, -1, -5, -16, -17, -40, -70};
    static const int starts[] = {0, 1, 9};
    for (int size = 0; size < 15; size++) {
        for (int shift = 0; shift < 18; shift++) {
            for (int start = 0; start < 3; start++) {
                for (int direction = 0; direction < 2; direction++) {
                    for (int at = 0; at < PAGE; at++) {
                        into[at] = (unsigned char)(at * 13 + 1);
                    }
                    struct iovec source[1] = {{into + 1200 + starts[start], sizes[size]}};
                    struct iovec target[1] = {{into + 1200 + starts[start] + shifts[shift], sizes[size]}};
                    long result = direction ? vm(1, self, source, 1, target, 1, 0) : vm(0, self, target, 1, source, 1, 0);
                    printf("overlap %d %d %d %d: %ld sum %08x\n", sizes[size], shifts[shift], starts[start], direction,
                           result, sum(into, PAGE));
                }
            }
        }
    }
    other_process(area, into);
    return 0;
}
