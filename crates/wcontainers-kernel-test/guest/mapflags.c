// mmap's flags as Linux's do_mmap checks them (validated sharing with huge page sizes, grows-down and
// droppable memory, huge pages of a plain file), mprotect's PROT_GROWSDOWN, a grows-down mapping
// growing under a copy, madvise's advice, statx's unique mount number and per file system rules, the
// stat calls' null and empty paths, and a sysctl read across segments.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <unistd.h>

#ifndef MAP_DROPPABLE
#define MAP_DROPPABLE 0x08
#endif
#ifndef MADV_DONTNEED_LOCKED
#define MADV_DONTNEED_LOCKED 24
#endif
#ifndef MADV_COLLAPSE
#define MADV_COLLAPSE 25
#endif
#ifndef MADV_GUARD_INSTALL
#define MADV_GUARD_INSTALL 102
#define MADV_GUARD_REMOVE 103
#endif
#define STATX_MNT_ID_UNIQUE_BIT 0x4000

static void show(const char *what, long result) {
    printf("%s = %ld errno %d\n", what, result < 0 ? -1L : result, result < 0 ? errno : 0);
}

static long map(void *at, size_t length, int prot, int flags, int fd) {
    void *got = mmap(at, length, prot, flags, fd, 0);
    if (got == MAP_FAILED) {
        return -1;
    }
    munmap(got, length);
    return 0;
}

static void flags_on(const char *name, int fd) {
    static const struct { const char *name; int flags; } cases[] = {
        {"validate huge 2mb", MAP_SHARED_VALIDATE | MAP_HUGE_2MB},
        {"validate huge 1gb", MAP_SHARED_VALIDATE | MAP_HUGE_1GB},
        {"validate growsdown", MAP_SHARED_VALIDATE | MAP_GROWSDOWN},
        {"validate sync", MAP_SHARED_VALIDATE | MAP_SYNC},
        {"validate noreplace", MAP_SHARED_VALIDATE | MAP_FIXED_NOREPLACE},
        {"validate unknown", MAP_SHARED_VALIDATE | 0x200000},
        {"shared unknown", MAP_SHARED | 0x200000},
        {"shared growsdown", MAP_SHARED | MAP_GROWSDOWN},
        {"private growsdown", MAP_PRIVATE | MAP_GROWSDOWN},
        {"private hugetlb", MAP_PRIVATE | MAP_HUGETLB},
        {"shared hugetlb", MAP_SHARED | MAP_HUGETLB},
        {"droppable", MAP_DROPPABLE},
        {"kind 0", 0},
        {"kind 15", 15},
    };
    for (unsigned index = 0; index < sizeof cases / sizeof cases[0]; index++) {
        char what[128];
        snprintf(what, sizeof what, "%s %s", name, cases[index].name);
        show(what, map(NULL, 8192, PROT_READ, cases[index].flags, fd));
    }
}

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    // Advice not built here: making huge pages now and guard markers.
    if (argc > 1 && strcmp(argv[1], "unbuilt") == 0) {
        char *memory = mmap(NULL, 4 << 20, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        memory[0] = 1;
        show("collapse", madvise(memory, 4 << 20, MADV_COLLAPSE));
        show("guard install", madvise(memory, 4096, MADV_GUARD_INSTALL));
        show("guard remove", madvise(memory, 4096, MADV_GUARD_REMOVE));
        return 0;
    }
    int file = open("/dev/shm/m", O_RDWR | O_CREAT, 0600);
    ftruncate(file, 16384);
    flags_on("file", file);
    int zero = open("/dev/zero", O_RDWR);
    flags_on("zero", zero);
    flags_on("anonymous", -1);
    static const int anonymous[] = {
        MAP_SHARED | MAP_GROWSDOWN, MAP_PRIVATE | MAP_GROWSDOWN, MAP_DROPPABLE, MAP_DROPPABLE | MAP_LOCKED,
        MAP_DROPPABLE | MAP_GROWSDOWN, MAP_DROPPABLE | MAP_POPULATE, MAP_SHARED_VALIDATE, MAP_PRIVATE | MAP_HUGE_2MB,
        MAP_PRIVATE | MAP_SYNC, MAP_PRIVATE | 0x200000, MAP_SHARED | MAP_SYNC,
    };
    for (unsigned index = 0; index < sizeof anonymous / sizeof anonymous[0]; index++) {
        char what[64];
        snprintf(what, sizeof what, "anonymous flags %#x", anonymous[index]);
        show(what, map(NULL, 8192, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | anonymous[index], -1));
    }

    // A grows-down mapping: a copy below it grows it, mprotect with PROT_GROWSDOWN reaches its start.
    char *region = mmap(NULL, 64 << 20, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
    munmap(region, 64 << 20);
    char *top = region + (32 << 20);
    char *down = mmap(top, 2 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_GROWSDOWN | MAP_FIXED, -1, 0);
    show("growsdown mapped", down == top ? 0 : -1);
    int null = open("/dev/zero", O_RDONLY);
    show("read below it", read(null, top - 3 * 4096, 4096));
    show("read far below it", read(null, top - (16 << 20), 4096));
    show("mprotect growsdown", mprotect(top + 4096, 4096, PROT_READ | PROT_GROWSDOWN));
    show("write after mprotect", write(file, top - 3 * 4096, 16));
    show("read into it after", read(null, top - 3 * 4096, 16));
    show("mprotect growsdown below", mprotect(top - (8 << 20), 4096, PROT_READ | PROT_GROWSDOWN));
    show("mprotect growsup", mprotect(top, 4096, PROT_READ | PROT_GROWSUP));
    show("mprotect both", mprotect(top, 4096, PROT_READ | PROT_GROWSDOWN | PROT_GROWSUP));
    char *plain = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    show("mprotect growsdown plain", mprotect(plain, 4096, PROT_READ | PROT_GROWSDOWN));
    show("mprotect high bit", syscall(SYS_mprotect, plain, 4096, PROT_READ | (1UL << 32)));
    // A mapping placed below it keeps the guard gap clear.
    char *hint = mmap(top - 4 * 4096 - (64 << 10), 64 << 10, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    show("hint in the gap taken", hint == top - 4 * 4096 - (64 << 10) ? 1 : 0);
    char *bump = mmap(top - (2 << 20), 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    show("read below with a mapping in reach", read(null, top - 5 * 4096, 4096));
    munmap(bump, 4096);
    FILE *maps = fopen("/proc/self/maps", "r");
    char line[256];
    unsigned long low = (unsigned long)(top - (4 << 20)), high = (unsigned long)(top + (1 << 20));
    while (maps && fgets(line, sizeof line, maps)) {
        unsigned long start, end;
        char perms[8];
        if (sscanf(line, "%lx-%lx %7s", &start, &end, perms) == 3 && end > low && start < high) {
            printf("maps: %+ld %+ld %s\n", (long)(start - (unsigned long)top), (long)(end - (unsigned long)top), perms);
        }
    }

    // Droppable memory keeps what is written while nothing presses on memory; mlock leaves it unlocked.
    char *drop = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_DROPPABLE, -1, 0);
    drop[0] = 7;
    show("droppable mlock", mlock(drop, 8192));
    show("droppable keeps", drop[0]);
    show("droppable dontneed", madvise(drop, 8192, MADV_DONTNEED));
    show("droppable after", drop[0]);

    // madvise: locked memory is dropped only by MADV_DONTNEED_LOCKED.
    char *locked = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    locked[0] = 3;
    show("mlock", mlock(locked, 8192));
    show("dontneed on locked", madvise(locked, 8192, MADV_DONTNEED));
    show("dontneed locked", madvise(locked, 8192, MADV_DONTNEED_LOCKED));
    show("after", locked[0]);
    char *holed = mmap(NULL, 3 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    munmap(holed + 4096, 4096);
    show("dontneed locked hole", madvise(holed, 3 * 4096, MADV_DONTNEED_LOCKED));
    static const int advice[] = {12, 13, 26, 99, 101, 104};
    for (unsigned index = 0; index < sizeof advice / sizeof advice[0]; index++) {
        char what[32];
        snprintf(what, sizeof what, "advice %d", advice[index]);
        show(what, madvise(locked, 4096, advice[index]));
    }

    // statx: the unique mount number on request, and each file system's own rules.
    static const char *const paths[] = {"/dev/shm/m", "/dev/null", "/dev/pts", "/dev/pts/ptmx", "/proc/self", "/proc/self/comm", "/proc/sys/kernel/ostype", "/dev/shm"};
    static const unsigned masks[] = {0, 0x40, 0x800, 0x1000, 0x7ff, 0xfff, STATX_MNT_ID_UNIQUE_BIT, 0x47ff};
    for (unsigned path = 0; path < sizeof paths / sizeof paths[0]; path++) {
        for (unsigned mask = 0; mask < sizeof masks / sizeof masks[0]; mask++) {
            // `struct statx` by offsets: musl's lacks the mount number.
            unsigned char out[256];
            memset(out, 0, sizeof out);
            long result = syscall(SYS_statx, AT_FDCWD, paths[path], 0, masks[mask], out);
            uint32_t got, nlink;
            uint64_t attributes, attributes_mask, mount;
            memcpy(&got, out, 4);
            memcpy(&attributes, out + 8, 8);
            memcpy(&nlink, out + 16, 4);
            memcpy(&attributes_mask, out + 56, 8);
            memcpy(&mount, out + 144, 8);
            printf("statx %s %#x = %ld mask %#x attributes %#llx of %#llx nlink %u mount id %s\n", paths[path], masks[mask],
                   result, got, (unsigned long long)attributes, (unsigned long long)attributes_mask, nlink,
                   mount == 0 ? "none" : mount >= (1ULL << 31) ? "unique" : "plain");
        }
    }
    int pts = open("/dev/pts", O_RDONLY | O_DIRECTORY);
    struct stat before, after;
    fstat(pts, &before);
    show("devpts root inode", before.st_ino);

    // Null and empty paths: with AT_EMPTY_PATH and a descriptor, the other flags are not looked at.
    unsigned char buffer[512];
    int pathfd = open("/dev/shm", O_PATH);
    chdir("/dev/shm");
    struct { const char *name; int dirfd; const char *path; int flags; } stats[] = {
        {"fd null", file, NULL, AT_EMPTY_PATH},
        {"fd null junk", file, NULL, AT_EMPTY_PATH | 0x80000000},
        {"fd empty removedir", file, "", AT_EMPTY_PATH | 0x200},
        {"cwd null", AT_FDCWD, NULL, AT_EMPTY_PATH},
        {"cwd null junk", AT_FDCWD, NULL, AT_EMPTY_PATH | 0x80000000},
        {"bad fd null", -5, NULL, AT_EMPTY_PATH},
        {"closed fd null", 99, NULL, AT_EMPTY_PATH},
        {"null without flag", AT_FDCWD, NULL, 0},
        {"null junk without flag", AT_FDCWD, NULL, 0x80000000},
        {"bad path", AT_FDCWD, (const char *)-4096L, AT_EMPTY_PATH},
        {"bad path junk", AT_FDCWD, (const char *)-4096L, AT_EMPTY_PATH | 0x80000000},
        {"path fd null", pathfd, NULL, AT_EMPTY_PATH},
    };
    for (unsigned index = 0; index < sizeof stats / sizeof stats[0]; index++) {
        char what[64];
        snprintf(what, sizeof what, "newfstatat %s", stats[index].name);
        show(what, syscall(SYS_newfstatat, stats[index].dirfd, stats[index].path, buffer, stats[index].flags));
        snprintf(what, sizeof what, "statx %s", stats[index].name);
        show(what, syscall(SYS_statx, stats[index].dirfd, stats[index].path, stats[index].flags, 0x7ff, buffer));
    }
    show("statx fd null reserved mask", syscall(SYS_statx, file, NULL, AT_EMPTY_PATH, 0x80000000, buffer));
    show("statx fd null both syncs", syscall(SYS_statx, file, NULL, AT_EMPTY_PATH | 0x6000, 0x7ff, buffer));
    fstat(pts, &after);

    // A sysctl is read whole, across every segment, or not at all.
    int ostype = open("/proc/sys/kernel/ostype", O_RDONLY);
    char *edge = mmap(NULL, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    munmap(edge + 4096, 4096);
    struct iovec parts[2] = {{edge + 4096 - 2, 2}, {edge + 4096, 100}};
    show("sysctl split at a fault", readv(ostype, parts, 2));
    show("sysctl position", lseek(ostype, 0, SEEK_CUR));
    show("sysctl read at a fault", read(ostype, edge + 4096 - 2, 100));
    struct iovec large[2] = {{edge, 12}, {(void *)13, 0x70002a00}};
    show("sysctl too large in all", readv(ostype, large, 2));
    struct iovec fine[2] = {{edge, 2}, {edge + 100, 100}};
    show("sysctl across two", readv(ostype, fine, 2));
    printf("sysctl bytes %.2s|%.5s\n", edge, edge + 100);
    show("sysctl position after", lseek(ostype, 0, SEEK_CUR));
    return 0;
}
