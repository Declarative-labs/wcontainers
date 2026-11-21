// Anonymous memory files (memfd_create) and the seals fcntl puts on them, and reads of the zero
// devices into buffers that end at an unmapped page.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/shm.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/vfs.h>
#include <sys/wait.h>
#include <unistd.h>

// Newer than the C library's headers (Linux's `linux/memfd.h` and `linux/fcntl.h`).
#define MFD_NOEXEC_SEAL 0x0008U
#define MFD_EXEC 0x0010U
#define MFD_HUGE_2MB (21U << 26)
#define F_SEAL_EXEC 0x0020

static void show(const char *what, long result) {
    printf("%s %ld %d\n", what, result, result < 0 ? errno : 0);
}

static void describe(const char *what, int fd) {
    struct stat st;
    char link[64], path[300];
    snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
    long length = readlink(link, path, sizeof path - 1);
    path[length > 0 ? length : 0] = 0;
    fstat(fd, &st);
    printf("%s: mode %o links %lu size %ld uid %u flags %o seals %d path %s\n", what, st.st_mode,
           (unsigned long)st.st_nlink, (long)st.st_size, st.st_uid, fcntl(fd, F_GETFL),
           fcntl(fd, F_GET_SEALS), path);
}

static long memfd(const char *name, unsigned flags) {
    return syscall(SYS_memfd_create, name, flags);
}

// Whether touching a byte of `at` ends a child with a signal, and which.
static void touch(const char *what, volatile char *at) {
    pid_t child = fork();
    if (child == 0) {
        at[0] = 1;
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("%s: %s %d\n", what, WIFSIGNALED(status) ? "signal" : "exit",
           WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status));
}

// Huge page memory files, on a machine that offers the sizes and reserves no pages.
static void huge(void) {
    // Each size Linux names, as log2 of the size in the flags' top bits.
    static const unsigned logs[] = {0, 16, 19, 21, 23, 24, 25, 28, 29, 30, 31, 34};
    for (unsigned i = 0; i < sizeof logs / sizeof logs[0]; i++) {
        char what[32];
        snprintf(what, sizeof what, "size log %u", logs[i]);
        long fd = memfd("x", MFD_HUGETLB | (logs[i] << 26));
        show(what, fd);
        if (fd >= 0) {
            struct stat st;
            fstat(fd, &st);
            printf("  blksize %ld blocks %ld\n", (long)st.st_blksize, (long)st.st_blocks);
            close(fd);
        }
    }
    show("noexec", memfd("x", MFD_HUGETLB | MFD_NOEXEC_SEAL));
    int fd = memfd("huge", MFD_HUGETLB | MFD_ALLOW_SEALING);
    describe("huge", fd);
    struct statfs fs;
    show("fstatfs", fstatfs(fd, &fs));
    printf("magic %lx bsize %ld blocks %ld free %ld files %ld\n", (unsigned long)fs.f_type,
           (long)fs.f_bsize, (long)fs.f_blocks, (long)fs.f_bfree, (long)fs.f_files);
    long huge_size = fs.f_bsize;
    show("write", write(fd, "abc", 3));
    show("truncate odd", ftruncate(fd, 4096));
    show("truncate", ftruncate(fd, 2 * huge_size));
    describe("truncated", fd);
    char bytes[8] = {9, 9, 9, 9, 9, 9, 9, 9};
    show("read", pread(fd, bytes, sizeof bytes, 0));
    printf("bytes %d %d\n", bytes[0], bytes[7]);
    show("read past end", pread(fd, bytes, sizeof bytes, 2 * huge_size));
    show("pwrite", pwrite(fd, "abc", 3, 0));
    show("seek end", lseek(fd, 0, SEEK_END));
    show("fallocate", fallocate(fd, 0, 0, huge_size));
    show("punch", fallocate(fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE, 0, huge_size));
    void *odd = mmap(NULL, huge_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 4096);
    show("map odd offset", odd == MAP_FAILED ? -1 : 0);
    char *shared = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    show("map shared", shared == MAP_FAILED ? -1 : 0);
    if (shared != MAP_FAILED) {
        printf("aligned %d\n", (int)((unsigned long)shared % huge_size == 0));
        touch("touch shared", shared);
        show("unmap part", munmap(shared, 4096));
        show("unmap", munmap(shared, huge_size));
    }
    char *private = mmap(NULL, huge_size, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    show("map private", private == MAP_FAILED ? -1 : 0);
    if (private != MAP_FAILED) {
        touch("touch private", private);
        munmap(private, huge_size);
    }
    // Anonymous huge pages and huge page segments, by the size their flags name.
    static const unsigned anonymous_logs[] = {0, 19, 21, 30};
    for (unsigned i = 0; i < sizeof anonymous_logs / sizeof anonymous_logs[0]; i++) {
        unsigned size = anonymous_logs[i] << 26;
        void *at = mmap(NULL, huge_size, PROT_READ | PROT_WRITE,
                        MAP_PRIVATE | MAP_ANONYMOUS | MAP_HUGETLB | size, -1, 0);
        printf("anonymous log %u: %d %d\n", anonymous_logs[i], at == MAP_FAILED ? -1 : 0,
               at == MAP_FAILED ? errno : 0);
        show("segment", shmget(IPC_PRIVATE, huge_size, SHM_HUGETLB | size | 0600));
    }
    show("grow seal", fcntl(fd, F_ADD_SEALS, F_SEAL_GROW));
    show("truncate sealed", ftruncate(fd, 4 * huge_size));
    show("truncate down", ftruncate(fd, huge_size));
    describe("shrunk", fd);
}

int main(int argc, char **argv) {
    (void)argv;
    if (argc > 1) {
        huge();
        return 0;
    }
    char name[300];
    memset(name, 'n', sizeof name);
    name[249] = 0;
    show("longest name", memfd(name, 0));
    name[249] = 'n';
    name[250] = 0;
    show("too long", memfd(name, 0));
    show("bad name", memfd((const char *)8, 0));
    show("bad flags", memfd("x", 0x100));
    show("exec and noexec", memfd("x", MFD_EXEC | MFD_NOEXEC_SEAL));
    show("huge size alone", memfd("x", MFD_HUGE_2MB));
    int plain = memfd("plain", 0);
    int sealing = memfd("sealing", MFD_ALLOW_SEALING | MFD_CLOEXEC);
    int noexec = memfd("noexec", MFD_NOEXEC_SEAL);
    int exec = memfd("exec", MFD_EXEC | MFD_ALLOW_SEALING);
    describe("plain", plain);
    describe("sealing", sealing);
    describe("noexec", noexec);
    describe("exec", exec);
    show("cloexec", fcntl(sealing, F_GETFD));
    struct statfs fs;
    show("fstatfs", fstatfs(plain, &fs));
    printf("magic %lx\n", (unsigned long)fs.f_type);

    show("add to plain", fcntl(plain, F_ADD_SEALS, F_SEAL_WRITE));
    show("unknown seal", fcntl(sealing, F_ADD_SEALS, 0x100));
    show("write", write(sealing, "hello world", 11));
    show("grow seal", fcntl(sealing, F_ADD_SEALS, F_SEAL_GROW));
    show("rewrite", pwrite(sealing, "HELLO", 5, 0));
    show("write past end", pwrite(sealing, "abcdef", 6, 8));
    show("truncate up", ftruncate(sealing, 100));
    show("truncate down", ftruncate(sealing, 4));
    show("fallocate past end", fallocate(sealing, 0, 0, 100));
    show("shrink seal", fcntl(sealing, F_ADD_SEALS, F_SEAL_SHRINK));
    show("truncate down", ftruncate(sealing, 2));
    show("truncate same", ftruncate(sealing, 4));
    show("write seal", fcntl(sealing, F_ADD_SEALS, F_SEAL_WRITE | F_SEAL_SEAL));
    show("write sealed", pwrite(sealing, "x", 1, 0));
    show("punch sealed", fallocate(sealing, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE, 0, 1));
    show("seal sealed", fcntl(sealing, F_ADD_SEALS, F_SEAL_GROW));
    describe("sealing", sealing);
    char buffer[16] = {0};
    show("read", pread(sealing, buffer, sizeof buffer, 0));
    printf("bytes %s\n", buffer);
    show("chmod noexec", fchmod(noexec, 0777));
    show("chmod noexec same", fchmod(noexec, 0644));
    show("exec seal", fcntl(exec, F_ADD_SEALS, F_SEAL_EXEC));
    describe("exec", exec);
    show("write exec sealed", write(exec, "x", 1));

    // Growing writes stop at the page that would grow the file.
    int grow = memfd("grow", MFD_ALLOW_SEALING);
    static char page[3 * 4096];
    show("fill", write(grow, page, 4096 + 100));
    show("seal", fcntl(grow, F_ADD_SEALS, F_SEAL_GROW));
    show("write across end", pwrite(grow, page, 8192, 10));
    show("write within", pwrite(grow, page, 4000, 10));

    int regular = open("/dev/shm/regular", O_CREAT | O_RDWR, 0644);
    show("regular seals", fcntl(regular, F_GET_SEALS));
    show("regular add", fcntl(regular, F_ADD_SEALS, F_SEAL_WRITE));
    int null = open("/dev/null", O_RDWR);
    show("device seals", fcntl(null, F_GET_SEALS));
    show("device add", fcntl(null, F_ADD_SEALS, F_SEAL_WRITE));
    int directory = open("/dev/shm", O_RDONLY);
    show("directory seals", fcntl(directory, F_GET_SEALS));
    char path[32];
    snprintf(path, sizeof path, "/proc/self/fd/%d", plain);
    int readonly = open(path, O_RDONLY);
    show("reopened", readonly);
    describe("reopened", readonly);
    show("add read only", fcntl(readonly, F_ADD_SEALS, F_SEAL_GROW));
    show("pipe seals", fcntl(1, F_GET_SEALS));

    // The zero devices clear eight bytes at a time up to an unmapped page.
    char *area = mmap(NULL, 3 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    munmap(area + 2 * 4096, 4096);
    int zero = open("/dev/zero", O_RDONLY);
    int full = open("/dev/full", O_RDONLY);
    for (int start = 0; start < 12; start++) {
        for (int count = 1; count < 20; count += 3) {
            char *at = area + 2 * 4096 - start;
            memset(area + 4096, 1, 4096);
            long got = read(start % 2 ? zero : full, at, count);
            int cleared = 0;
            while (cleared < start && at[cleared] == 0) cleared++;
            printf("zero %d %d: %ld %d\n", start, count, got, cleared);
        }
    }
    show("zero large", read(zero, area, 1 << 20));
    show("zero lseek", lseek(zero, 0, SEEK_CUR));
    // Direct I/O: not on the file memfd_create makes, but on the same file opened again.
    int direct = memfd("direct", 0);
    show("memfd set direct", fcntl(direct, F_SETFL, O_DIRECT));
    char reopen[64];
    snprintf(reopen, sizeof reopen, "/proc/self/fd/%d", direct);
    int again = open(reopen, O_RDWR);
    show("reopened set direct", fcntl(again, F_SETFL, O_DIRECT) < 0 ? -1 : 0);
    return 0;
}
