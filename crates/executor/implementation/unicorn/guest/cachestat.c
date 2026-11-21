// cachestat on tmpfs files, memory files, huge page memory files, devices, /proc and directories. With the argument "mapped",
// a file read through a private mapping, whose faults bring in pages.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

struct range {
    uint64_t offset, length;
};
struct counts {
    uint64_t cache, dirty, writeback, evicted, recently_evicted;
};

static void show(const char *what, int fd, uint64_t offset, uint64_t length) {
    struct range range = {offset, length};
    struct counts counts;
    memset(&counts, 0xee, sizeof counts);
    long result = syscall(SYS_cachestat, fd, &range, &counts, 0);
    printf("%s: %ld errno %d: %llx %llx %llx %llx %llx\n", what, result, result ? errno : 0,
           (unsigned long long)counts.cache, (unsigned long long)counts.dirty, (unsigned long long)counts.writeback,
           (unsigned long long)counts.evicted, (unsigned long long)counts.recently_evicted);
}

int main(int argc, char **argv) {
    static char bytes[20000];
    memset(bytes, 1, sizeof bytes);
    int fd = open("/dev/shm/f", O_RDWR | O_CREAT, 0644);
    if (argc > 1) {
        ftruncate(fd, 40000);
        char *mapped = mmap(0, 40960, PROT_READ, MAP_PRIVATE, fd, 0);
        volatile char byte = mapped[33000];
        (void)byte;
        show("mapped read", fd, 0, 0);
        return 0;
    }
    show("bad descriptor", -1, 0, 0);
    show("empty", fd, 0, 0);
    write(fd, bytes, 10000);
    show("written", fd, 0, 0);
    ftruncate(fd, 40000);
    show("grown", fd, 0, 0);
    fallocate(fd, 0, 20000, 8192);
    show("allocated", fd, 0, 0);
    pread(fd, bytes, 4096, 36000);
    show("hole read", fd, 0, 0);
    show("one page", fd, 4096, 4096);
    show("two pages", fd, 4095, 2);
    show("past the end", fd, 1 << 20, 4096);
    show("wrapping", fd, 5, UINT64_MAX);
    show("to the end", fd, 8192, 0);
    ftruncate(fd, 5000);
    show("shrunk", fd, 0, 0);
    fallocate(fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE, 0, 4096);
    show("punched", fd, 0, 0);
    int zero = open("/dev/zero", O_RDONLY);
    show("device", zero, 0, 0);
    int status = open("/proc/self/status", O_RDONLY);
    show("proc", status, 0, 0);
    int directory = open("/dev/shm", O_RDONLY);
    show("directory", directory, 0, 0);
    int path = open("/dev/shm/f", O_PATH);
    show("path descriptor", path, 0, 0);
    int memory = memfd_create("m", 0);
    write(memory, bytes, 5000);
    show("memory file", memory, 0, 0);
    int huge = memfd_create("h", MFD_HUGETLB);
    printf("huge page memory file %d\n", huge >= 0);
    show("huge page memory file", huge, 0, 0);
    printf("huge, bad out: %ld errno %d\n",
           syscall(SYS_cachestat, huge, &(struct range){0, 0}, (void *)8, 0), errno);
    show("pipe", 1, 0, 0);
    struct range range = {0, 0};
    struct counts counts;
    printf("flags: %ld errno %d\n", syscall(SYS_cachestat, fd, &range, &counts, 1), errno);
    printf("bad range: %ld errno %d\n", syscall(SYS_cachestat, fd, (void *)8, &counts, 1), errno);
    printf("bad out: %ld errno %d\n", syscall(SYS_cachestat, fd, &range, (void *)8, 0), errno);
    // Permission: someone else's file opened for reading only.
    int other = open("/dev/shm/g", O_RDWR | O_CREAT, 0644);
    write(other, bytes, 100);
    int other_read = open("/dev/shm/g", O_RDONLY);
    int open_mode = open("/dev/shm/h", O_RDWR | O_CREAT, 0666);
    int open_read = open("/dev/shm/h", O_RDONLY);
    setresuid(1000, 1000, 1000);
    show("not owner", other_read, 0, 0);
    show("not owner, writable file", open_read, 0, 0);
    show("not owner, opened for writing", other, 0, 0);
    printf("not owner, flags: %ld errno %d\n", syscall(SYS_cachestat, other_read, &range, &counts, 1), errno);
    (void)open_mode;
    return 0;
}
