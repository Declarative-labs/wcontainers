// Links to missing files and named pipes on the backend: creating through a dangling link, and
// extended attributes of a named pipe reached by path, by descriptor and through /proc.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/xattr.h>
#include <unistd.h>

#define SHOW(what, value)                                                                          \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long v_ = (long)(value);                                                                   \
        printf("%s: %ld errno %d\n", what, v_, v_ < 0 ? errno : 0);                                \
    } while (0)

int main(void) {
    char cwd[256], absolute[300], buf[256], name[64];
    struct stat st;
    getcwd(cwd, sizeof cwd);

    symlink("target-missing", "dangle");
    SHOW("creat through dangling", open("dangle", O_CREAT | O_WRONLY, 0600));
    SHOW("target", stat("target-missing", &st));
    SHOW("target mode", st.st_mode);
    snprintf(absolute, sizeof absolute, "%s/absolute-missing", strcmp(cwd, "/") ? cwd : "");
    symlink(absolute, "dangle2");
    SHOW("creat through absolute dangling", open("dangle2", O_CREAT | O_WRONLY, 0600));
    unlink(absolute);
    SHOW("excl through dangling", open("dangle2", O_CREAT | O_EXCL | O_WRONLY, 0600));
    SHOW("nofollow", openat(AT_FDCWD, "dangle", O_CREAT | O_WRONLY | O_NOFOLLOW, 0600));
    symlink("missing-dir/file", "dangle3");
    SHOW("creat through link to missing directory", open("dangle3", O_CREAT | O_WRONLY, 0600));
    mkdir("sub", 0755);
    symlink("../made-from-sub", "sub/up");
    SHOW("creat through relative link in a subdirectory", open("sub/up", O_CREAT | O_RDWR, 0600));
    SHOW("made from sub", stat("made-from-sub", &st));

    SHOW("mkfifo", mknod("p", S_IFIFO | 0600, 0));
    int fd = open("p", O_RDONLY | O_NONBLOCK);
    SHOW("open fifo", fd >= 0);
    snprintf(name, sizeof name, "/proc/self/fd/%d", fd);
    SHOW("getxattr cap via proc", getxattr(name, "security.capability", buf, 65537));
    SHOW("getxattr cap path", getxattr("p", "security.capability", buf, sizeof buf));
    SHOW("fgetxattr cap", fgetxattr(fd, "security.capability", buf, sizeof buf));
    SHOW("fgetxattr user", fgetxattr(fd, "user.none", buf, sizeof buf));
    SHOW("setxattr security.a", setxattr("p", "security.a", "v", 1, 0));
    SHOW("getxattr security.a", getxattr("p", "security.a", buf, sizeof buf));
    SHOW("fgetxattr security.a", fgetxattr(fd, "security.a", buf, sizeof buf));
    SHOW("fsetxattr security.b", fsetxattr(fd, "security.b", "w", 1, 0));
    SHOW("getxattr security.b", getxattr("p", "security.b", buf, sizeof buf));
    SHOW("flistxattr", flistxattr(fd, buf, sizeof buf));
    SHOW("fremovexattr security.b", fremovexattr(fd, "security.b"));
    SHOW("setxattr user.a", setxattr("p", "user.a", "v", 1, 0));
    int regular = open("regular", O_CREAT | O_WRONLY, 0644);
    snprintf(name, sizeof name, "/proc/self/fd/%d", regular);
    SHOW("getxattr cap regular", getxattr("regular", "security.capability", buf, sizeof buf));
    SHOW("getxattr cap regular via proc", getxattr(name, "security.capability", buf, 65537));
    SHOW("getxattr cap directory", getxattr(".", "security.capability", buf, sizeof buf));
    return 0;
}
