// Reading access control lists and other attributes of backend files after giving up root.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/xattr.h>
#include <unistd.h>

static const char *names[] = {"system.posix_acl_access", "system.posix_acl_default", "user.a",
                              "trusted.a", "security.a", "system.a"};

static void show(const char *who) {
    char value[64];
    const char *paths[] = {".", "file", "link"};
    for (int p = 0; p < 3; p++) {
        for (int n = 0; n < 6; n++) {
            long got = lgetxattr(paths[p], names[n], value, 1);
            printf("%s lgetxattr %s %s: %ld %s\n", who, paths[p], names[n], got,
                   got < 0 ? strerror(errno) : "");
            got = getxattr(paths[p], names[n], value, sizeof value);
            printf("%s getxattr %s %s: %ld %s\n", who, paths[p], names[n], got,
                   got < 0 ? strerror(errno) : "");
        }
    }
}

int main(void) {
    mkdir("w", 0777);
    chdir("w");
    close(open("file", O_CREAT | O_WRONLY, 0644));
    symlink("file", "link");
    show("root");
    prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
    setresuid(1000, 1000, -1);
    show("user");
    // A descriptor's inode after the open checked it, and again after its attributes were asked.
    char value[64];
    int fd = open("file", O_RDONLY);
    for (int round = 0; round < 2; round++) {
        for (int n = 0; n < 2; n++) {
            long got = fgetxattr(fd, names[n], value, sizeof value);
            printf("user fgetxattr file %s round %d: %ld %s\n", names[n], round, got,
                   got < 0 ? strerror(errno) : "");
        }
        struct stat st;
        fstat(fd, &st);
    }
    close(fd);
    return 0;
}
