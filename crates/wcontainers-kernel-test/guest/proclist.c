// Lists the directories its arguments name and prints, for each entry, its directory, name, listed type and what
// lstat says of it (type and mode, owner, group, link count), one entry per line.

#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
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

static void list(const char *dir) {
    int fd = open(dir, O_RDONLY | O_DIRECTORY);
    if (fd < 0) {
        printf("%s: cannot open\n", dir);
        return;
    }
    char buffer[8192];
    long got;
    while ((got = syscall(SYS_getdents64, fd, buffer, sizeof buffer)) > 0) {
        for (long at = 0; at < got;) {
            struct entry *entry = (struct entry *)(buffer + at);
            struct stat st;
            if (fstatat(fd, entry->name, &st, AT_SYMLINK_NOFOLLOW) == 0) {
                printf("%s %s %d %o %d %d %ld\n", dir, entry->name, entry->type, st.st_mode, st.st_uid,
                       st.st_gid, (long)st.st_nlink);
            } else {
                printf("%s %s %d lstat failed\n", dir, entry->name, entry->type);
            }
            at += entry->reclen;
        }
    }
    close(fd);
}

int main(int argc, char **argv) {
    for (int index = 1; index < argc; index++) {
        list(argv[index]);
    }
    return 0;
}
