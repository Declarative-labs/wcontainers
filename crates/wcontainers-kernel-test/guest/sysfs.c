// /sys: the read-only sysfs a default container mounts there, its root's details, its file
// system, and what may not be done on it. With "lookups", looks up names Linux has under it and
// prints nothing.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/statvfs.h>
#include <sys/time.h>
#include <sys/xattr.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "lookups") == 0) {
        struct stat st;
        stat("/sys/kernel/mm/transparent_hugepage/hpage_pmd_size", &st);
        stat("/sys/fs/cgroup/cpu.max", &st);
        stat("/sys/devices/system/cpu/online", &st);
        stat("/sys/no-such-entry", &st);
        return 0;
    }
    struct stat st;
    p("stat", stat("/sys", &st));
    printf("mode %o uid %u gid %u ino %lu size %ld blocks %ld blksize %ld\n", st.st_mode, st.st_uid,
           st.st_gid, (unsigned long)st.st_ino, (long)st.st_size, (long)st.st_blocks,
           (long)st.st_blksize);
    struct stat root;
    stat("/", &root);
    printf("own device %d\n", st.st_dev != root.st_dev);

    struct statfs fs;
    p("statfs", statfs("/sys", &fs));
    printf("type %lx bsize %ld blocks %lu bfree %lu files %lu ffree %lu namelen %ld frsize %ld flags %lx\n",
           (unsigned long)fs.f_type, (long)fs.f_bsize, (unsigned long)fs.f_blocks,
           (unsigned long)fs.f_bfree, (unsigned long)fs.f_files, (unsigned long)fs.f_ffree,
           (long)fs.f_namelen, (long)fs.f_frsize, (unsigned long)fs.f_flags);

    // Read-only: nothing is made, changed or written there.
    p("mkdir", mkdir("/sys/made", 0755));
    p("creat", open("/sys/made", O_CREAT | O_WRONLY, 0644));
    p("mknod", mknod("/sys/fifo", S_IFIFO | 0644, 0));
    p("symlink", symlink("x", "/sys/link"));
    p("chmod", chmod("/sys", 0755));
    p("chown", chown("/sys", 1, 1));
    p("utimes", utimes("/sys", NULL));
    p("access w", access("/sys", W_OK));
    p("access r", access("/sys", R_OK | X_OK));
    p("rmdir", rmdir("/sys"));
    p("open dir write", open("/sys", O_RDWR));
    p("setxattr", setxattr("/sys", "user.x", "1", 1, 0));

    // Names it lacks in every machine.
    p("missing", stat("/sys/no-such-entry", &st));
    char longname[300];
    memset(longname, 'a', sizeof longname);
    memcpy(longname, "/sys/", 5);
    longname[sizeof longname - 1] = 0;
    p("long name", stat(longname, &st));
    p("missing under", stat("/sys/no-such-entry/x", &st));

    int fd = open("/sys", O_RDONLY | O_DIRECTORY);
    p("open", fd);
    struct statvfs vfs;
    p("fstatvfs", fstatvfs(fd, &vfs));
    printf("vfs flags %lx\n", vfs.f_flag);
    p("fchdir", fchdir(fd));
    char cwd[64];
    printf("cwd %s\n", getcwd(cwd, sizeof cwd));
    p("parent", stat("..", &st));
    printf("parent is root %d\n", st.st_ino == root.st_ino && st.st_dev == root.st_dev);
    return 0;
}
