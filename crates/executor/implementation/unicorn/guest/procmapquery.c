// PROCMAP_QUERY on /proc/<pid>/maps: the mapping at or after an address, its permissions, file,
// offset and name, the query's filters, and its refusals of malformed queries.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/fs.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE 4096

static char *base;

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

// A query, and what it found relative to `base`.
static void query(int fd, const char *what, unsigned long address, unsigned long flags) {
    struct procmap_query q;
    char name[256] = "";
    memset(&q, 0, sizeof q);
    q.size = sizeof q;
    q.query_addr = address;
    q.query_flags = flags;
    q.vma_name_addr = (unsigned long)name;
    q.vma_name_size = sizeof name;
    long got = ioctl(fd, PROCMAP_QUERY, &q);
    if (got < 0) {
        result(what, got);
        return;
    }
    const char *last = strrchr(name, '/');
    printf("%s: %lx-%lx flags %llx page %llu offset %llx name %s size %u\n", what,
           (unsigned long)(q.vma_start - (unsigned long)base),
           (unsigned long)(q.vma_end - (unsigned long)base), (unsigned long long)q.vma_flags,
           (unsigned long long)q.vma_page_size, (unsigned long long)q.vma_offset,
           last ? last : name, q.vma_name_size);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    base = mmap((void *)0x50000000, 16 * PAGE, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE,
                -1, 0);
    int file = open("/tmp/mapquery", O_RDWR | O_CREAT | O_TRUNC, 0600);
    ftruncate(file, 8 * PAGE);
    mmap(base, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    munmap(base + 2 * PAGE, 2 * PAGE);
    mmap(base + 4 * PAGE, 3 * PAGE, PROT_READ, MAP_SHARED | MAP_FIXED, file, 2 * PAGE);
    mmap(base + 7 * PAGE, PAGE, PROT_READ | PROT_EXEC, MAP_PRIVATE | MAP_FIXED, file, 0);
    mmap(base + 8 * PAGE, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    munmap(base + 10 * PAGE, 6 * PAGE);

    int fd = open("/proc/self/maps", O_RDONLY);
    query(fd, "anonymous", (unsigned long)base + 100, 0);
    query(fd, "hole", (unsigned long)base + 2 * PAGE, 0);
    query(fd, "hole or next", (unsigned long)base + 2 * PAGE, PROCMAP_QUERY_COVERING_OR_NEXT_VMA);
    query(fd, "file", (unsigned long)base + 5 * PAGE, 0);
    query(fd, "file writable", (unsigned long)base + 5 * PAGE, PROCMAP_QUERY_VMA_WRITABLE);
    query(fd, "next writable", (unsigned long)base + 5 * PAGE,
          PROCMAP_QUERY_VMA_WRITABLE | PROCMAP_QUERY_COVERING_OR_NEXT_VMA);
    query(fd, "next executable", (unsigned long)base,
          PROCMAP_QUERY_VMA_EXECUTABLE | PROCMAP_QUERY_COVERING_OR_NEXT_VMA);
    query(fd, "next shared", (unsigned long)base,
          PROCMAP_QUERY_VMA_SHARED | PROCMAP_QUERY_COVERING_OR_NEXT_VMA);
    query(fd, "next file", (unsigned long)base,
          PROCMAP_QUERY_FILE_BACKED_VMA | PROCMAP_QUERY_COVERING_OR_NEXT_VMA);
    query(fd, "shared anonymous", (unsigned long)base + 9 * PAGE, 0);
    query(fd, "next shared file", (unsigned long)base + 8 * PAGE,
          PROCMAP_QUERY_VMA_SHARED | PROCMAP_QUERY_FILE_BACKED_VMA);
    query(fd, "the program", (unsigned long)main, 0);

    struct stat status;
    fstat(file, &status);
    struct procmap_query q;
    memset(&q, 0, sizeof q);
    q.size = sizeof q;
    q.query_addr = (unsigned long)base + 4 * PAGE;
    result("file query", ioctl(fd, PROCMAP_QUERY, &q));
    printf("inode matches %d device matches %d\n", q.inode == status.st_ino,
           q.dev_major == major(status.st_dev) && q.dev_minor == minor(status.st_dev));
    q.query_addr = (unsigned long)base;
    result("anonymous", ioctl(fd, PROCMAP_QUERY, &q));
    printf("inode %llu device %u:%u\n", (unsigned long long)q.inode, q.dev_major, q.dev_minor);

    char small[4];
    q.vma_name_addr = (unsigned long)small;
    q.vma_name_size = sizeof small;
    q.query_addr = (unsigned long)base + 4 * PAGE;
    result("name too long", ioctl(fd, PROCMAP_QUERY, &q));
    q.query_addr = (unsigned long)base;
    result("no name", ioctl(fd, PROCMAP_QUERY, &q));
    printf("name size %u\n", q.vma_name_size);
    char id[64];
    q.vma_name_size = 0;
    q.vma_name_addr = 0;
    q.build_id_addr = (unsigned long)id;
    q.build_id_size = sizeof id;
    result("build id of anonymous memory", ioctl(fd, PROCMAP_QUERY, &q));
    printf("build id size %u\n", q.build_id_size);
    q.build_id_size = 0;
    result("build id address without size", ioctl(fd, PROCMAP_QUERY, &q));
    q.build_id_addr = 0;
    q.vma_name_size = 10;
    result("name size without address", ioctl(fd, PROCMAP_QUERY, &q));
    q.vma_name_size = 0;
    q.query_flags = 1 << 10;
    result("unknown flag", ioctl(fd, PROCMAP_QUERY, &q));
    q.query_flags = 0;
    q.size = 16;
    result("too small", ioctl(fd, PROCMAP_QUERY, &q));
    q.size = 24;
    result("just the address", ioctl(fd, PROCMAP_QUERY, &q));
    printf("start past what was asked %lx\n", (unsigned long)q.vma_start);
    char big[8192];
    memset(big, 0, sizeof big);
    memcpy(big, &q, sizeof q);
    ((struct procmap_query *)big)->size = 200;
    result("larger with zeros", ioctl(fd, PROCMAP_QUERY, big));
    big[150] = 1;
    result("larger with more", ioctl(fd, PROCMAP_QUERY, big));
    ((struct procmap_query *)big)->size = 5000;
    result("too large", ioctl(fd, PROCMAP_QUERY, big));
    result("bad address", ioctl(fd, PROCMAP_QUERY, (void *)8));
    result("other request", ioctl(fd, FIONREAD, big));

    pid_t child = fork();
    if (child == 0) {
        pause();
        _exit(0);
    }
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/maps", child);
    int theirs = open(path, O_RDONLY);
    query(theirs, "child's file", (unsigned long)base + 5 * PAGE, 0);
    kill(child, SIGKILL);
    waitpid(child, NULL, 0);
    query(theirs, "child gone", (unsigned long)base + 5 * PAGE, 0);
    unlink("/tmp/mapquery");
    return 0;
}
