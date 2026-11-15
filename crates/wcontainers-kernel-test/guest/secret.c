// memfd_secret: flags, what the file says of itself, what it refuses, its one size change, and its
// shared, locked mapping (munlock leaves it locked, so MADV_DONTNEED refuses it).

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <unistd.h>

#define PAGE 4096
#define SYS_memfd_secret 447

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    result("unknown flag", syscall(SYS_memfd_secret, 1));
    result("cloexec and more", syscall(SYS_memfd_secret, O_CLOEXEC | 1));
    int fd = syscall(SYS_memfd_secret, O_CLOEXEC);
    result("cloexec", fd);
    printf("descriptor flags: %d\n", fcntl(fd, F_GETFD));
    printf("status flags: %#o\n", fcntl(fd, F_GETFL));
    struct stat st;
    fstat(fd, &st);
    printf("mode %o size %ld links %ld uid %d blksize %ld\n", st.st_mode, (long)st.st_size, (long)st.st_nlink, st.st_uid,
           (long)st.st_blksize);
    struct statfs fs;
    fstatfs(fd, &fs);
    printf("magic %#lx\n", (unsigned long)fs.f_type);
    char link[256] = "";
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    readlink(path, link, sizeof link - 1);
    printf("link %s\n", link);
    char byte = 1;
    result("write", write(fd, &byte, 1));
    result("read", read(fd, &byte, 1));
    result("pread", pread(fd, &byte, 1, 0));
    result("lseek", lseek(fd, 0, SEEK_END));
    result("fallocate", fallocate(fd, 0, 0, PAGE));
    result("fsync", fsync(fd));
    result("map before size", mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0) == MAP_FAILED ? -1 : 0);
    result("size", ftruncate(fd, 4 * PAGE));
    result("size again", ftruncate(fd, 8 * PAGE));
    result("shrink", ftruncate(fd, PAGE));
    fstat(fd, &st);
    printf("size %ld blocks %ld\n", (long)st.st_size, (long)st.st_blocks);
    result("private", mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0) == MAP_FAILED ? -1 : 0);
    unsigned char *a = mmap(0, 4 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    result("shared", a == MAP_FAILED ? -1 : 0);
    a[5] = 0x42;
    unsigned char *b = mmap(0, 4 * PAGE, PROT_READ, MAP_SHARED, fd, 0);
    printf("second mapping sees %#x\n", b[5]);
    result("munlock", munlock(a, 4 * PAGE));
    result("madvise dontneed", madvise(a, PAGE, MADV_DONTNEED));
    printf("after dontneed %#x\n", a[5]);
    int out = open("/dev/shm/x", O_RDWR | O_CREAT | O_TRUNC, 0644);
    result("write from it", write(out, a, 8));
    result("read into it", pread(out, a + 8, 4, 0));
    struct iovec local = {&byte, 1}, remote = {a + 5, 1};
    result("process_vm_readv", syscall(SYS_process_vm_readv, getpid(), &local, 1, &remote, 1, 0));
    char line[512];
    FILE *maps = fopen("/proc/self/maps", "r");
    while (fgets(line, sizeof line, maps)) {
        char permissions[8], path[256] = "";
        unsigned long start, end, offset, inode;
        char device[16];
        if (strstr(line, "secret") && sscanf(line, "%lx-%lx %7s %lx %15s %lu %255[^\n]", &start, &end, permissions,
                                             &offset, device, &inode, path) == 7) {
            printf("map: %lu pages %s %lx %s\n", (end - start) / PAGE, permissions, offset, path);
        }
    }
    fclose(maps);
    munmap(b, 4 * PAGE);
    munmap(a, 4 * PAGE);
    struct rlimit limit = {PAGE, PAGE};
    setrlimit(RLIMIT_MEMLOCK, &limit);
    result("over the lock limit", mmap(0, 2 * PAGE, PROT_READ, MAP_SHARED, fd, 0) == MAP_FAILED ? -1 : 0);
    unsigned char *c = mmap(0, 2 * PAGE, PROT_READ, MAP_SHARED, fd, PAGE);
    result("past the end, within the limit", c == MAP_FAILED ? -1 : 0);
    return 0;
}
