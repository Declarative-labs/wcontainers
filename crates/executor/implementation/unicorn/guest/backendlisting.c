// Listing a backend directory while removing from it, which shows how many entries each answer
// holds (Linux asks for attributes with the first page), and O_TMPFILE's checks before a file
// would be made.
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

static void listing_while_removing(const char *at, int buffer) {
    char path[64];
    mkdir(at, 0755);
    for (int i = 0; i < 40; i++) {
        snprintf(path, sizeof path, "%s/f%02d", at, i);
        close(open(path, O_CREAT | O_WRONLY, 0644));
    }
    int fd = open(at, O_RDONLY | O_DIRECTORY);
    char buf[32768];
    long n;
    int listed = 0, seen = 0, calls = 0;
    while ((n = syscall(SYS_getdents64, fd, buf, buffer)) > 0) {
        calls++;
        for (long off = 0; off < n; off += ((struct dirent *)(buf + off))->d_reclen) {
            struct dirent *entry = (struct dirent *)(buf + off);
            if (entry->d_name[0] != 'f') continue;
            listed++;
            if (++seen == 6) {
                for (int i = 0; i < 40; i += 2) {
                    snprintf(path, sizeof path, "%s/f%02d", at, i);
                    unlink(path);
                }
            }
        }
    }
    close(fd);
    printf("%s, buffer %d: listed while removing %d\n", at, buffer, listed);
    for (int i = 0; i < 40; i++) {
        snprintf(path, sizeof path, "%s/f%02d", at, i);
        unlink(path);
    }
    rmdir(at);
}

static void open_tmpfile(const char *path) {
    int fd = open(path, O_RDWR | O_TMPFILE, 0600);
    printf("O_TMPFILE %s: %d %s\n", path, fd < 0 ? -1 : 0, fd < 0 ? strerror(errno) : "");
    if (fd >= 0) close(fd);
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    listing_while_removing("/data/ls", 32768);
    listing_while_removing("/data/ls", 2048);
    listing_while_removing("/data/ls", 200);
    close(open("/data/file", O_CREAT | O_WRONLY, 0644));
    open_tmpfile("/data/missing");
    open_tmpfile("/data/missing/deeper");
    open_tmpfile("/data/file");
    open_tmpfile("/data");
    open_tmpfile("/proc/missing");
    unlink("/data/file");
    return 0;
}
