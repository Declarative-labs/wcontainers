// tmpfs directory listings after creating, removing, linking and renaming names: each listing whole
// (names and positions), then again one entry per call, and again from every position it gave.

#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

struct entry {
    uint64_t ino;
    int64_t off;
    uint16_t reclen;
    uint8_t type;
    char name[];
};

static long getdents(int fd, char *buffer, size_t size) {
    return syscall(SYS_getdents64, fd, buffer, size);
}

// Prints every entry from `start`, reading `size` bytes at a time, and keeps the positions seen.
static int walk(const char *dir, int64_t start, size_t size, int64_t *positions, int room) {
    int fd = open(dir, O_RDONLY | O_DIRECTORY);
    char buffer[4096];
    int seen = 0;
    lseek(fd, start, SEEK_SET);
    printf(" [%lld/%zu]", (long long)start, size);
    for (;;) {
        long got = getdents(fd, buffer, size);
        if (got <= 0) {
            if (got < 0) {
                printf(" error");
            }
            break;
        }
        for (long at = 0; at < got;) {
            struct entry *entry = (struct entry *)(buffer + at);
            printf(" %s:%lld:%d", entry->name, (long long)entry->off, entry->type);
            if (seen < room) {
                positions[seen++] = entry->off;
            }
            at += entry->reclen;
        }
    }
    printf("\n");
    close(fd);
    return seen;
}

static void list(const char *dir, const char *after) {
    int64_t positions[64];
    printf("%s:", after);
    int count = walk(dir, 0, sizeof positions, positions, 64);
    printf("  one at a time:");
    walk(dir, 0, 32, positions, 0);
    for (int index = 0; index < count; index++) {
        printf("  from:");
        walk(dir, positions[index], 4096, positions, 0);
    }
}

static void touch(const char *path) {
    close(open(path, O_CREAT | O_WRONLY, 0644));
}

int main(void) {
    mkdir("/dev/shm/order", 0755);
    mkdir("/dev/shm/other", 0755);
    chdir("/dev/shm/order");
    list(".", "empty");
    touch("a");
    touch("b");
    touch("c");
    list(".", "created a b c");
    rename("c", "a");
    list(".", "renamed c over a");
    touch("d");
    rename("b", "e");
    list(".", "created d, renamed b to a new name e");
    syscall(SYS_renameat2, AT_FDCWD, "a", AT_FDCWD, "d", 2);
    list(".", "exchanged a and d");
    link("e", "f");
    unlink("d");
    list(".", "linked e as f, removed d");
    mkdir("g", 0755);
    rename("../other", "g");
    list(".", "renamed a directory over the empty directory g");
    rename("f", "../f");
    rename("../f", "h");
    list(".", "moved f out and back as h");
    syscall(SYS_renameat2, AT_FDCWD, "h", AT_FDCWD, "a", 1);
    syscall(SYS_renameat2, AT_FDCWD, "h", AT_FDCWD, "h", 2);
    list(".", "refused renames");
    // RENAME_WHITEOUT leaves a device at the old name, which takes a position after the new name's.
    touch("i");
    syscall(SYS_renameat2, AT_FDCWD, "i", AT_FDCWD, "j", 4);
    syscall(SYS_renameat2, AT_FDCWD, "j", AT_FDCWD, "e", 4);
    list(".", "renamed i to j and j over e, each leaving a whiteout");
    // Positions set by lseek: 2 is the first entry, others resume at the entry at or below them.
    int64_t seeks[] = {2, 3, 5, 100, 2147483646, 2147483648LL, 1LL << 40};
    for (unsigned index = 0; index < sizeof seeks / sizeof *seeks; index++) {
        int64_t positions[1];
        printf("seek %lld:", (long long)seeks[index]);
        walk(".", seeks[index], 4096, positions, 0);
    }
    // A position at a removed entry resumes at the next older one still there.
    int fd = open(".", O_RDONLY | O_DIRECTORY);
    char buffer[64];
    getdents(fd, buffer, 32);
    getdents(fd, buffer, 32);
    long got = getdents(fd, buffer, 32);
    struct entry *entry = (struct entry *)buffer;
    printf("third entry %s, then removing it", got > 0 ? entry->name : "none");
    unlink(entry->name);
    printf("\n");
    for (;;) {
        got = getdents(fd, buffer, 32);
        if (got <= 0) {
            break;
        }
        entry = (struct entry *)buffer;
        printf(" %s:%lld", entry->name, (long long)entry->off);
    }
    printf("\n");
    close(fd);
    // A position below every remaining entry starts again from the first one.
    fd = open(".", O_RDONLY | O_DIRECTORY);
    int64_t last = 0;
    char *oldest = 0;
    char names[64][32];
    int count = 0;
    while ((got = getdents(fd, buffer, 32)) > 0) {
        entry = (struct entry *)buffer;
        strcpy(names[count], entry->name);
        last = entry->off;
        count++;
    }
    lseek(fd, 0, SEEK_SET);
    for (int index = 0; index < count - 1; index++) {
        got = getdents(fd, buffer, 32);
    }
    entry = (struct entry *)buffer;
    oldest = names[count - 1];
    printf("before the oldest %s (end %lld), removing it:", oldest, (long long)last);
    unlink(oldest);
    while ((got = getdents(fd, buffer, 32)) > 0) {
        entry = (struct entry *)buffer;
        printf(" %s:%lld", entry->name, (long long)entry->off);
    }
    printf("\n");
    close(fd);
    list(".", "at the end");
    return 0;
}
