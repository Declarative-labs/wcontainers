// The namespace links of /proc/<pid>/ns: their listing, what they name, and the files they open.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <sys/xattr.h>
#include <unistd.h>

static const char *names[] = {"cgroup", "ipc", "mnt", "net", "pid", "pid_for_children",
                              "time", "time_for_children", "user", "uts"};
#define COUNT (sizeof names / sizeof names[0])

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

// The number in "type:[number]", or 0.
static unsigned long number(const char *link) {
    const char *open = strchr(link, '[');
    return open ? strtoul(open + 1, NULL, 10) : 0;
}

static void target(const char *path, char *out, size_t size) {
    ssize_t n = readlink(path, out, size - 1);
    out[n < 0 ? 0 : n] = 0;
}

int main(void) {
    // The listing, in getdents order.
    DIR *directory = opendir("/proc/self/ns");
    struct dirent *entry;
    while ((entry = readdir(directory)) != NULL) {
        printf("entry %s type %d\n", entry->d_name, entry->d_type);
    }
    closedir(directory);
    struct stat st;
    stat("/proc/self/ns", &st);
    printf("directory mode %o links %ld\n", st.st_mode, (long)st.st_nlink);

    char links[COUNT][64];
    struct stat first;
    stat("/proc/self/ns/uts", &first);
    for (size_t i = 0; i < COUNT; i++) {
        char path[64];
        snprintf(path, sizeof path, "/proc/self/ns/%s", names[i]);
        target(path, links[i], sizeof links[i]);
        struct stat link, named;
        lstat(path, &link);
        p("stat", stat(path, &named));
        // The number is the named file's inode; the init user and time namespaces have Linux's fixed
        // numbers, the others are the container's own.
        printf("%s link mode %o links %ld uid %d size %ld; prefix %.*s number is inode %d\n", names[i],
               link.st_mode, (long)link.st_nlink, link.st_uid, (long)link.st_size,
               (int)(strchr(links[i], '[') - links[i]), links[i], number(links[i]) == named.st_ino);
        printf("  named mode %o links %ld uid %d gid %d size %ld blksize %ld same device %d major %d\n",
               named.st_mode, (long)named.st_nlink, named.st_uid, named.st_gid, (long)named.st_size,
               (long)named.st_blksize, named.st_dev == first.st_dev, major(named.st_dev));
    }
    printf("user %s\n", links[8]);
    for (size_t i = 0; i < COUNT; i++) {
        for (size_t j = i + 1; j < COUNT; j++) {
            if (strcmp(links[i] + strcspn(links[i], "["), links[j] + strcspn(links[j], "[")) == 0) {
                printf("%s and %s name one namespace\n", names[i], names[j]);
            }
        }
    }

    // A file opened through a link.
    int fd = open("/proc/self/ns/uts", O_RDONLY);
    p("open", fd);
    struct stat opened;
    fstat(fd, &opened);
    printf("opened is named %d mode %o\n", opened.st_ino == first.st_ino && opened.st_dev == first.st_dev,
           opened.st_mode);
    char fdpath[64], fdlink[64];
    snprintf(fdpath, sizeof fdpath, "/proc/self/fd/%d", fd);
    target(fdpath, fdlink, sizeof fdlink);
    printf("descriptor link is the namespace's %d\n", strcmp(fdlink, links[9]) == 0);
    struct statfs fs;
    fstatfs(fd, &fs);
    printf("file system %lx\n", (unsigned long)fs.f_type);
    printf("flags %o\n", fcntl(fd, F_GETFL));
    char byte;
    p("read", read(fd, &byte, 1));
    p("write", write(fd, "x", 1));
    p("lseek", lseek(fd, 0, SEEK_SET));
    p("mmap", (long)mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, fd, 0) == -1 ? -1 : 0);
    p("getdents", syscall(61, fd, fdpath, sizeof fdpath));
    p("open for writing", open("/proc/self/ns/uts", O_WRONLY));
    p("open for reading and writing", open("/proc/self/ns/uts", O_RDWR));
    p("open truncating", open("/proc/self/ns/uts", O_RDONLY | O_TRUNC));
    p("chmod", chmod("/proc/self/ns/uts", 0600));
    p("chown", chown("/proc/self/ns/uts", 1, 1));
    p("utimes", utimensat(AT_FDCWD, "/proc/self/ns/uts", NULL, 0));
    p("truncate", truncate("/proc/self/ns/uts", 0));
    p("setxattr", setxattr("/proc/self/ns/uts", "user.a", "b", 1, 0));
    p("fchmod", fchmod(fd, 0600));
    p("lchown of the link", lchown("/proc/self/ns/uts", 0, 0));
    p("open as a directory", open("/proc/self/ns/uts", O_RDONLY | O_DIRECTORY));
    p("open with O_CREAT", open("/proc/self/ns/uts", O_RDONLY | O_CREAT, 0644));
    p("open with O_PATH and O_NOFOLLOW", open("/proc/self/ns/uts", O_PATH | O_NOFOLLOW) >= 0 ? 0 : -1);
    p("ioctl NS_GET_NSTYPE", ioctl(fd, 0xb703));
    struct { int handle_bytes; int handle_type; unsigned char f_handle[64]; } handle = {64};
    int mount;
    p("name_to_handle_at", name_to_handle_at(fd, "", (void *)&handle, &mount, AT_EMPTY_PATH));
    printf("handle type %d bytes %d\n", handle.handle_type, handle.handle_bytes);
    p("epoll", epoll_ctl(epoll_create1(0), EPOLL_CTL_ADD, fd, &(struct epoll_event){EPOLLIN}));
    p("fsync", fsync(fd));
    p("setns", setns(fd, CLONE_NEWUTS));
    p("setns any", setns(fd, 0));
    p("unshare", unshare(CLONE_NEWUTS));
    p("unlink", unlink("/proc/self/ns/uts"));
    p("symlink", symlink("x", "/proc/self/ns/new"));
    p("missing", open("/proc/self/ns/other", O_RDONLY));
    close(fd);

    // Another process and a thread's own directory name the same namespaces.
    pid_t child = fork();
    if (child == 0) {
        pause();
        _exit(0);
    }
    for (size_t i = 0; i < COUNT; i++) {
        char path[64], link[64];
        snprintf(path, sizeof path, "/proc/%d/ns/%s", child, names[i]);
        target(path, link, sizeof link);
        snprintf(path, sizeof path, "/proc/self/task/%d/ns/%s", getpid(), names[i]);
        char own[64];
        target(path, own, sizeof own);
        printf("%s child same %d thread same %d\n", names[i], strcmp(link, links[i]) == 0,
               strcmp(own, links[i]) == 0);
    }
    kill(child, SIGKILL);
    waitpid(child, NULL, 0);
    // A reaped process's directory is gone; a zombie's links still read.
    child = fork();
    if (child == 0) {
        _exit(0);
    }
    waitid(P_PID, child, &(siginfo_t){0}, WEXITED | WNOWAIT);
    char path[64], link[64];
    snprintf(path, sizeof path, "/proc/%d/ns/uts", child);
    ssize_t n = readlink(path, link, sizeof link);
    p("zombie readlink", n < 0 ? -1 : 0);
    snprintf(path, sizeof path, "/proc/%d/ns/pid", child);
    p("zombie pid readlink", readlink(path, link, sizeof link) < 0 ? -1 : 0);
    snprintf(path, sizeof path, "/proc/%d/ns/user", child);
    p("zombie user readlink", readlink(path, link, sizeof link) < 0 ? -1 : 0);
    snprintf(path, sizeof path, "/proc/%d/ns/pid_for_children", child);
    p("zombie pid_for_children readlink", readlink(path, link, sizeof link) < 0 ? -1 : 0);
    snprintf(path, sizeof path, "/proc/%d/ns/uts", child);
    p("zombie open", open(path, O_RDONLY) >= 0 ? 0 : -1);
    waitpid(child, NULL, 0);
    p("reaped readlink", readlink(path, link, sizeof link));
    // The user namespace that owns each (a new descriptor of /proc/self/ns/user, closed at exec),
    // and each one's parent, which none here may reach.
    struct stat user;
    stat("/proc/self/ns/user", &user);
    for (size_t i = 0; i < COUNT; i++) {
        char path[64];
        snprintf(path, sizeof path, "/proc/self/ns/%s", names[i]);
        int ns = open(path, O_RDONLY);
        int owner = ioctl(ns, 0xb701);
        if (owner >= 0) {
            struct stat st;
            fstat(owner, &st);
            printf("%s owner: the user namespace %d type %x fd flags %x file flags %o\n", names[i],
                   st.st_ino == user.st_ino && st.st_dev == user.st_dev, ioctl(owner, 0xb703),
                   fcntl(owner, F_GETFD), fcntl(owner, F_GETFL));
            close(owner);
        } else {
            printf("%s owner: %s\n", names[i], strerror(errno));
        }
        printf("%s parent: %d %s\n", names[i], ioctl(ns, 0xb702), strerror(errno));
        close(ns);
    }
    return 0;
}
