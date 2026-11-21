// Extended attributes and fallocate on files of the backend, in the working directory.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/capability.h>
#include <linux/falloc.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/xattr.h>
#include <unistd.h>

static void result(const char *what, long value) {
    if (value < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
    } else {
        printf("%s: %ld\n", what, value);
    }
}

static void names(const char *what, const char *path) {
    char list[256];
    ssize_t length = listxattr(path, list, sizeof list);
    result(what, length);
    for (ssize_t at = 0; at < length; at += strlen(list + at) + 1) {
        printf("  %s\n", list + at);
    }
}

static void size_of(const char *path) {
    struct stat st;
    stat(path, &st);
    printf("%s: size %ld blocks %ld\n", path, (long)st.st_size, (long)st.st_blocks);
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    int fd = open("attrs", O_CREAT | O_RDWR | O_TRUNC, 0644);
    char value[64];
    result("set user.a", setxattr("attrs", "user.a", "one", 3, 0));
    result("set user.b", setxattr("attrs", "user.b", "", 0, 0));
    result("length of user.a", getxattr("attrs", "user.a", 0, 0));
    memset(value, 0, sizeof value);
    result("get user.a", getxattr("attrs", "user.a", value, sizeof value));
    printf("  value %s\n", value);
    result("get user.a into 2", getxattr("attrs", "user.a", value, 2));
    result("get user.b", getxattr("attrs", "user.b", value, sizeof value));
    result("get user.missing", getxattr("attrs", "user.missing", value, sizeof value));
    result("create user.a again", setxattr("attrs", "user.a", "two", 3, XATTR_CREATE));
    result("replace user.c", setxattr("attrs", "user.c", "two", 3, XATTR_REPLACE));
    result("replace user.a", setxattr("attrs", "user.a", "three", 5, XATTR_REPLACE));
    result("fget user.a", fgetxattr(fd, "user.a", value, sizeof value));
    result("fset user.f", fsetxattr(fd, "user.f", "f", 1, 0));
    names("list", "attrs");
    result("list length", listxattr("attrs", 0, 0));
    char small[4];
    result("list into 4", listxattr("attrs", small, sizeof small));
    result("set trusted.t", setxattr("attrs", "trusted.t", "t", 1, 0));
    result("get trusted.t", getxattr("attrs", "trusted.t", value, sizeof value));
    result("set security.s", setxattr("attrs", "security.s", "s", 1, 0));
    result("set an unknown prefix", setxattr("attrs", "other.o", "o", 1, 0));
    result("get an unknown prefix", getxattr("attrs", "other.o", value, sizeof value));
    result("remove user.a", removexattr("attrs", "user.a"));
    result("remove user.a again", removexattr("attrs", "user.a"));
    result("fremove user.f", fremovexattr(fd, "user.f"));
    names("list after removing", "attrs");
    symlink("attrs", "link");
    result("lset user. on a link", lsetxattr("link", "user.l", "l", 1, 0));
    result("get through the link", getxattr("link", "user.b", value, sizeof value));
    mkdir("dir", 0755);
    result("set user. on a directory", setxattr("dir", "user.d", "d", 1, 0));
    names("list the directory", "dir");

    char page[4096];
    memset(page, 'x', sizeof page);
    write(fd, page, sizeof page);
    result("fallocate", fallocate(fd, 0, 0, 16384));
    size_of("attrs");
    result("fallocate keeping the size", fallocate(fd, FALLOC_FL_KEEP_SIZE, 16384, 8192));
    size_of("attrs");
    result("punch a hole", fallocate(fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE, 0, 4096));
    size_of("attrs");
    char first[1] = {1};
    pread(fd, first, 1, 0);
    printf("punched byte: %d\n", first[0]);
    result("punch without keeping the size", fallocate(fd, FALLOC_FL_PUNCH_HOLE, 0, 4096));
    result("zero a range", fallocate(fd, FALLOC_FL_ZERO_RANGE, 0, 4096));
    result("collapse a range", fallocate(fd, FALLOC_FL_COLLAPSE_RANGE, 0, 4096));
    result("fallocate a negative length", fallocate(fd, 0, 0, -1));
    int reader = open("attrs", O_RDONLY);
    result("fallocate read only", fallocate(reader, 0, 0, 4096));
    int directory = open("dir", O_RDONLY);
    result("fallocate a directory", fallocate(directory, 0, 0, 4096));
    return 0;
}
