// Shared file mappings: what a mapping writes the file shows and the other way round, other mappings
// of the same file, size changes, mremap, mprotect, seals, MADV_REMOVE and /proc/self/maps. Ends with a
// touch of a page past the file's end, which is SIGBUS.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

#define PAGE 4096

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

static void file_bytes(const char *what, int fd, off_t offset, size_t count) {
    unsigned char bytes[64];
    memset(bytes, 0xee, sizeof bytes);
    long got = pread(fd, bytes, count, offset);
    printf("%s: %ld:", what, got);
    for (long i = 0; i < (long)count; i++) {
        printf(" %02x", bytes[i]);
    }
    printf("\n");
}

static void memory_bytes(const char *what, const unsigned char *at, size_t count) {
    printf("%s:", what);
    for (size_t i = 0; i < count; i++) {
        printf(" %02x", at[i]);
    }
    printf("\n");
}

static int compare_lines(const void *a, const void *b) {
    return strcmp((const char *)a, (const char *)b);
}

// The size, permissions, offset and path of each shared mapping in /proc/self/maps, in sorted order:
// where each lies depends on the layout, which differs (no randomization, and no vDSO yet).
static void maps(const char *what) {
    static char lines[16][300];
    int count = 0;
    char line[512];
    FILE *file = fopen("/proc/self/maps", "r");
    printf("%s:\n", what);
    while (fgets(line, sizeof line, file) && count < 16) {
        char permissions[8], device[16], path[256] = "";
        unsigned long start, end, offset, inode;
        if (sscanf(line, "%lx-%lx %7s %lx %15s %lu %255s", &start, &end, permissions, &offset, device, &inode,
                   path) >= 6 &&
            permissions[3] == 's') {
            snprintf(lines[count++], sizeof lines[0], "%lx %lu pages %s %s", offset, (end - start) / PAGE,
                     permissions, path);
        }
    }
    fclose(file);
    qsort(lines, count, sizeof lines[0], compare_lines);
    for (int i = 0; i < count; i++) {
        printf("  %s\n", lines[i]);
    }
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    static unsigned char pattern[10000];
    for (int i = 0; i < (int)sizeof pattern; i++) {
        pattern[i] = (unsigned char)(i * 7);
    }
    int fd = open("/dev/shm/s", O_RDWR | O_CREAT, 0644);
    write(fd, pattern, sizeof pattern);
    unsigned char *a = mmap(0, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    result("map", a == MAP_FAILED ? -1 : 0);
    memory_bytes("mapped", a + 96, 8);
    memset(a + 100, 0x41, 4);
    file_bytes("file after a mapping write", fd, 96, 12);
    pwrite(fd, "BBBB", 4, 200);
    memory_bytes("mapping after a file write", a + 196, 12);
    write(fd, "end", 3);
    memory_bytes("mapping after an append", a + 9998, 6);

    // A second mapping of the same file, at another offset.
    unsigned char *b = mmap(0, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, PAGE);
    b[5] = 0x55;
    memory_bytes("first mapping sees the second", a + PAGE + 4, 3);
    a[PAGE + 6] = 0x66;
    memory_bytes("second mapping sees the first", b + 4, 3);

    // Bytes written past the end in the last page are not the file's.
    a[10010] = 0x77;
    file_bytes("past the end", fd, 10008, 4);
    result("grow", ftruncate(fd, 10020));
    file_bytes("grown", fd, 10008, 12);
    result("shrink", ftruncate(fd, 9000));
    memory_bytes("shrunk, last page", a + 8996, 8);
    result("grow again", ftruncate(fd, 10020));
    memory_bytes("grown again", a + 8996, 8);
    file_bytes("grown again", fd, 8996, 8);

    // A write from a buffer that is the mapping itself.
    result("write from the mapping", pwrite(fd, a + 100, 8, 300));
    memory_bytes("mapping after it", a + 300, 8);

    result("msync", msync(a, 3 * PAGE, MS_SYNC));
    result("dontneed", madvise(a, PAGE, MADV_DONTNEED));
    memory_bytes("after dontneed", a + 100, 4);

    // Growing it past the file's end, then growing the file under it.
    unsigned char *c = mremap(a, 3 * PAGE, 5 * PAGE, MREMAP_MAYMOVE);
    result("mremap", c == MAP_FAILED ? -1 : 0);
    memory_bytes("moved", c + 100, 4);
    result("grow under it", ftruncate(fd, 5 * PAGE));
    c[4 * PAGE + 1] = 0x99;
    file_bytes("written in the new page", fd, 4 * PAGE, 4);
    maps("maps");
    result("munmap", munmap(c, 5 * PAGE));
    file_bytes("after munmap", fd, 4 * PAGE, 4);
    memory_bytes("second still maps", b + 4, 3);
    result("munmap second", munmap(b, 2 * PAGE));
    file_bytes("after both", fd, 96, 12);

    // Growing a mapping over a file that already reaches further shows the file there.
    result("file of three pages", ftruncate(fd, 3 * PAGE));
    unsigned char *one = mmap(0, PAGE, PROT_READ, MAP_SHARED, fd, 0);
    unsigned char *three = mremap(one, PAGE, 3 * PAGE, MREMAP_MAYMOVE);
    result("mremap over the file", three == MAP_FAILED ? -1 : 0);
    if (three != MAP_FAILED) {
        memory_bytes("grown over the file", three + 2 * PAGE, 4);
        munmap(three, 3 * PAGE);
    }

    // Punching through a mapping.
    unsigned char *d = mmap(0, 3 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    // Only the change in blocks: the pages a mapping's faults bring in are not known (counted).
    struct stat st;
    fstat(fd, &st);
    long before = st.st_blocks;
    result("remove", madvise(d + PAGE, PAGE, MADV_REMOVE));
    file_bytes("removed", fd, PAGE, 4);
    fstat(fd, &st);
    printf("blocks given back: %ld\n", before - (long)st.st_blocks);
    memory_bytes("still mapped", d + 96, 8);
    munmap(d, 3 * PAGE);

    // Files open for reading only.
    int read_only = open("/dev/shm/s", O_RDONLY);
    result("writable, read only", mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, read_only, 0) == MAP_FAILED ? -1 : 0);
    unsigned char *e = mmap(0, PAGE, PROT_READ, MAP_SHARED, read_only, 0);
    result("readable, read only", e == MAP_FAILED ? -1 : 0);
    result("mprotect to write", mprotect(e, PAGE, PROT_READ | PROT_WRITE));
    result("mprotect to run", mprotect(e, PAGE, PROT_READ | PROT_EXEC));
    pwrite(fd, "CC", 2, 10);
    memory_bytes("read only sees", e + 8, 4);
    maps("read only maps");
    munmap(e, PAGE);
    unsigned char *f = mmap(0, PAGE, PROT_READ, MAP_PRIVATE, read_only, 0);
    result("private, mprotect to write", mprotect(f, PAGE, PROT_READ | PROT_WRITE));
    munmap(f, PAGE);
    result("noexec, run", mmap(0, PAGE, PROT_READ | PROT_EXEC, MAP_SHARED, fd, 0) == MAP_FAILED ? -1 : 0);
    unsigned char *g = mmap(0, PAGE, PROT_READ, MAP_PRIVATE, fd, 0);
    result("noexec, mprotect to run", mprotect(g, PAGE, PROT_READ | PROT_EXEC));
    munmap(g, PAGE);

    // Validated sharing.
    result("validate", mmap(0, PAGE, PROT_READ, MAP_SHARED_VALIDATE, fd, 0) == MAP_FAILED ? -1 : 0);
    result("validate, sync", mmap(0, PAGE, PROT_READ, MAP_SHARED_VALIDATE | MAP_SYNC, fd, 0) == MAP_FAILED ? -1 : 0);
    result("validate, noreplace",
           mmap(0, PAGE, PROT_READ, MAP_SHARED_VALIDATE | MAP_FIXED_NOREPLACE, fd, 0) == MAP_FAILED ? -1 : 0);
    result("validate, populate",
           mmap(0, PAGE, PROT_READ, MAP_SHARED_VALIDATE | MAP_POPULATE, fd, 0) == MAP_FAILED ? -1 : 0);
    result("validate, 0x40", mmap(0, PAGE, PROT_READ, MAP_SHARED_VALIDATE | 0x40, fd, 0) == MAP_FAILED ? -1 : 0);

    // Seals.
    int sealed = memfd_create("sealed", MFD_ALLOW_SEALING);
    ftruncate(sealed, PAGE);
    unsigned char *h = mmap(0, PAGE, PROT_READ, MAP_SHARED, sealed, 0);
    result("seal write, read mapping", fcntl(sealed, F_ADD_SEALS, F_SEAL_WRITE));
    munmap(h, PAGE);
    result("seal write, unmapped", fcntl(sealed, F_ADD_SEALS, F_SEAL_WRITE));
    result("sealed, writable", mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, sealed, 0) == MAP_FAILED ? -1 : 0);
    unsigned char *i = mmap(0, PAGE, PROT_READ, MAP_SHARED, sealed, 0);
    result("sealed, readable", i == MAP_FAILED ? -1 : 0);
    result("sealed, mprotect to write", mprotect(i, PAGE, PROT_READ | PROT_WRITE));
    unsigned char *j = mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE, sealed, 0);
    result("sealed, private", j == MAP_FAILED ? -1 : 0);

    int future = memfd_create("future", MFD_ALLOW_SEALING);
    ftruncate(future, PAGE);
    unsigned char *k = mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, future, 0);
    result("seal future write", fcntl(future, F_ADD_SEALS, F_SEAL_FUTURE_WRITE));
    k[0] = 0x12;
    file_bytes("written after the seal", future, 0, 2);
    result("future sealed, writable",
           mmap(0, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, future, 0) == MAP_FAILED ? -1 : 0);
    result("remove, future sealed", madvise(k, PAGE, MADV_REMOVE));

    // A memory file mapped before it has any size, then grown.
    int grown = memfd_create("grown", 0);
    unsigned char *l = mmap(0, 4 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, grown, 0);
    result("map empty", l == MAP_FAILED ? -1 : 0);
    ftruncate(grown, 4 * PAGE);
    l[3 * PAGE] = 0x34;
    file_bytes("grown memory file", grown, 3 * PAGE, 2);
    ftruncate(grown, PAGE);
    file_bytes("final bytes", fd, 0, 16);

    printf("touching a page past the end\n");
    l[2 * PAGE] = 1;
    printf("not reached\n");
    return 0;
}
