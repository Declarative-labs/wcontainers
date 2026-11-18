// Files opened with O_TMPFILE: made with no name, given one later through /proc/self/fd, or never.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/inotify.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

static void show(const char *what, int fd) {
    struct stat st;
    fstat(fd, &st);
    char path[64], link[256];
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    ssize_t n = readlink(path, link, sizeof link - 1);
    link[n < 0 ? 0 : n] = 0;
    // The link names the directory and "#" and the inode number.
    char expected[256];
    snprintf(expected, sizeof expected, "/dev/shm/t/#%lu (deleted)", (unsigned long)st.st_ino);
    printf("%s: mode %o links %ld uid %d size %ld; link is the number %d flags %o\n", what, st.st_mode,
           (long)st.st_nlink, st.st_uid, (long)st.st_size, strcmp(link, expected) == 0,
           fcntl(fd, F_GETFL));
}

static void events(int watch) {
    char buffer[4096];
    ssize_t n = read(watch, buffer, sizeof buffer);
    for (char *at = buffer; n > 0 && at < buffer + n;) {
        struct inotify_event *event = (struct inotify_event *)at;
        // A file with no name is named "#" and its inode number, which differs between machines.
        const char *name = event->len ? event->name : "";
        printf("event %x name %s\n", event->mask, name[0] == '#' ? "#number" : name);
        at += sizeof *event + event->len;
    }
}

int main(void) {
    mkdir("/dev/shm/t", 0755);
    int watch = inotify_init1(IN_NONBLOCK);
    inotify_add_watch(watch, "/dev/shm/t", IN_ALL_EVENTS);
    umask(022);

    p("read only", open("/dev/shm/t", O_TMPFILE | O_RDONLY, 0600));
    p("with O_CREAT", open("/dev/shm/t", O_TMPFILE | O_CREAT | O_RDWR, 0600));
    p("not a directory", open("/etc/passwd", O_TMPFILE | O_RDWR, 0600));
    p("missing", open("/dev/shm/none", O_TMPFILE | O_RDWR, 0600));
    p("in /proc", open("/proc", O_TMPFILE | O_RDWR, 0600));
    p("in /dev/pts", open("/dev/pts", O_TMPFILE | O_RDWR, 0600));
    p("in /dev/mqueue", open("/dev/mqueue", O_TMPFILE | O_RDWR, 0600));

    int fd = open("/dev/shm/t", O_TMPFILE | O_RDWR, 0666);
    p("open", fd >= 0 ? 0 : -1);
    show("new", fd);
    p("write", write(fd, "hello", 5));
    show("written", fd);
    events(watch);
    // Given a name through its descriptor's link.
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    p("linkat", linkat(AT_FDCWD, path, AT_FDCWD, "/dev/shm/t/named", AT_SYMLINK_FOLLOW));
    show("named", fd);
    p("linkat again", linkat(AT_FDCWD, path, AT_FDCWD, "/dev/shm/t/second", AT_SYMLINK_FOLLOW));
    events(watch);
    char buffer[16] = {0};
    int again = open("/dev/shm/t/named", O_RDONLY);
    p("read named", read(again, buffer, sizeof buffer));
    printf("read %s\n", buffer);
    close(again);
    unlink("/dev/shm/t/named");
    unlink("/dev/shm/t/second");
    p("linkat after unlinking", linkat(AT_FDCWD, path, AT_FDCWD, "/dev/shm/t/third", AT_SYMLINK_FOLLOW));
    close(fd);

    // With O_EXCL it never takes a name.
    fd = open("/dev/shm/t", O_TMPFILE | O_RDWR | O_EXCL, 0600);
    snprintf(path, sizeof path, "/proc/self/fd/%d", fd);
    show("exclusive", fd);
    p("linkat exclusive", linkat(AT_FDCWD, path, AT_FDCWD, "/dev/shm/t/never", AT_SYMLINK_FOLLOW));
    struct statfs before, after;
    statfs("/dev/shm", &before);
    p("ftruncate", ftruncate(fd, 1 << 20));
    p("fallocate", fallocate(fd, 0, 0, 1 << 20));
    statfs("/dev/shm", &after);
    printf("blocks taken %ld\n", (long)(before.f_bfree - after.f_bfree));
    close(fd);
    statfs("/dev/shm", &after);
    printf("blocks back %d\n", before.f_bfree == after.f_bfree);
    events(watch);
    p("directory entries", rmdir("/dev/shm/t") == 0 ? 0 : -1);
    return 0;
}
