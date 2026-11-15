// Extended attributes on the in-memory file systems, file capabilities, and the credentials a program
// runs with after execve: set-user and set-group bits, file capabilities, secure mode and dumpability.
// The program runs again from copies of itself, with the stage to run as its argument.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/xattr.h>
#include <unistd.h>

static void show(const char *what, long result) {
    printf("%s %ld %d\n", what, result, result < 0 ? errno : 0);
}

static void show_bytes(const char *what, const char *bytes, long length) {
    printf("%s %ld:", what, length);
    for (long i = 0; i < length; i++) printf(" %02x", (unsigned char)bytes[i]);
    printf("\n");
}

static void attributes(void) {
    char buffer[256];
    mkdir("/dev/shm/x", 0777);
    close(open("/dev/shm/x/f", O_CREAT | O_RDWR, 0644));
    mkdir("/dev/shm/x/d", 01777);
    symlink("f", "/dev/shm/x/l");
    const char *names[] = {"user.b", "user.a", "user.zz", "trusted.a", "security.a", "system.a", "other.a",
                           "user.", "security.", "system.posix_acl_access", "system.posix_acl_default", ""};
    const char *paths[] = {"/dev/shm/x/f", "/dev/shm/x/d", "/dev/null", "/proc/self/status", "/proc/self",
                           "/dev/pts/ptmx", "/proc/sys/kernel/hostname"};
    for (unsigned p = 0; p < sizeof paths / sizeof *paths; p++) {
        for (unsigned n = 0; n < sizeof names / sizeof *names; n++) {
            printf("%s %s: ", paths[p], names[n]);
            show("set", setxattr(paths[p], names[n], "value", 5, 0));
            show("  get", getxattr(paths[p], names[n], buffer, sizeof buffer));
        }
        long length = listxattr(paths[p], buffer, sizeof buffer);
        show_bytes("  list", buffer, length);
    }
    show("create existing", setxattr("/dev/shm/x/f", "user.a", "1", 1, XATTR_CREATE));
    show("replace missing", setxattr("/dev/shm/x/f", "user.none", "1", 1, XATTR_REPLACE));
    show("bad flags", setxattr("/dev/shm/x/f", "user.a", "1", 1, 4));
    show("empty value", setxattr("/dev/shm/x/f", "user.a", "", 0, 0));
    show("size of empty", getxattr("/dev/shm/x/f", "user.a", NULL, 0));
    show("size of user.b", getxattr("/dev/shm/x/f", "user.b", NULL, 0));
    show("small buffer", getxattr("/dev/shm/x/f", "user.b", buffer, 2));
    show("list size", listxattr("/dev/shm/x/f", NULL, 0));
    show("small list", listxattr("/dev/shm/x/f", buffer, 3));
    show("remove", removexattr("/dev/shm/x/f", "user.b"));
    show("remove again", removexattr("/dev/shm/x/f", "user.b"));
    show("remove trusted", removexattr("/dev/shm/x/f", "trusted.a"));
    show("remove security", removexattr("/dev/shm/x/f", "security.a"));
    show("link set", lsetxattr("/dev/shm/x/l", "user.a", "1", 1, 0));
    show("link get", lgetxattr("/dev/shm/x/l", "user.a", buffer, sizeof buffer));
    show("link list", llistxattr("/dev/shm/x/l", buffer, sizeof buffer));
    show("through link", getxattr("/dev/shm/x/l", "user.a", buffer, sizeof buffer));
    show("long name", setxattr("/dev/shm/x/f", "user.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "1", 1, 0));
    show("too big", setxattr("/dev/shm/x/f", "user.a", buffer, 65537, 0));
    show("bad name", getxattr("/dev/shm/x/f", (const char *)8, buffer, sizeof buffer));
    show("bad value", getxattr("/dev/shm/x/f", "user.zz", (char *)8, sizeof buffer));
    show("missing", getxattr("/dev/shm/x/nope", "user.a", buffer, sizeof buffer));
    int fd = open("/dev/shm/x/f", O_RDONLY);
    show("fd set", fsetxattr(fd, "user.c", "fd", 2, 0));
    show("fd get", fgetxattr(fd, "user.c", buffer, sizeof buffer));
    show("fd list", flistxattr(fd, buffer, sizeof buffer));
    show("fd remove", fremovexattr(fd, "user.c"));
    show("bad fd", fgetxattr(99, "user.c", buffer, sizeof buffer));
    show("pipe set", fsetxattr(1, "user.c", "1", 1, 0));
    show("pipe list", flistxattr(1, buffer, sizeof buffer));
    int path_fd = open("/dev/shm/x/f", O_PATH);
    show("path fd", fgetxattr(path_fd, "user.a", buffer, sizeof buffer));
    unsigned char acl[28] = {2, 0, 0, 0, 1, 0, 6, 0, 0xff, 0xff, 0xff, 0xff, 4, 0, 4, 0,
                             0xff, 0xff, 0xff, 0xff, 0x20, 0, 4, 0, 0xff, 0xff, 0xff, 0xff};
    show("acl", setxattr("/dev/shm/x/f", "system.posix_acl_access", acl, sizeof acl, 0));
    show("acl short", setxattr("/dev/shm/x/f", "system.posix_acl_access", acl, 6, 0));
    acl[4] = 0x40;
    show("acl bad tag", setxattr("/dev/shm/x/f", "system.posix_acl_access", acl, sizeof acl, 0));
    acl[0] = 1;
    show("acl version", setxattr("/dev/shm/x/f", "system.posix_acl_access", acl, sizeof acl, 0));
    show("acl remove", removexattr("/dev/shm/x/f", "system.posix_acl_access"));

    // File capabilities: the short form, the long form with a root, and both read back.
    unsigned char v2[20] = {0, 0, 0, 2, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
    unsigned char v3[24] = {1, 0, 0, 3, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xe8, 3, 0, 0};
    show("caps v2", setxattr("/dev/shm/x/f", "security.capability", v2, sizeof v2, 0));
    show_bytes("caps read", buffer, getxattr("/dev/shm/x/f", "security.capability", buffer, sizeof buffer));
    show("caps v3", setxattr("/dev/shm/x/f", "security.capability", v3, sizeof v3, 0));
    show_bytes("caps read", buffer, getxattr("/dev/shm/x/f", "security.capability", buffer, sizeof buffer));
    v3[20] = 0;
    v3[21] = 0;
    show("caps v3 root", setxattr("/dev/shm/x/f", "security.capability", v3, sizeof v3, 0));
    show_bytes("caps read", buffer, getxattr("/dev/shm/x/f", "security.capability", buffer, sizeof buffer));
    show("caps v2 short", setxattr("/dev/shm/x/f", "security.capability", v2, 16, 0));
    v3[3] = 1;
    show("caps v1", setxattr("/dev/shm/x/f", "security.capability", v3, sizeof v3, 0));
    show_bytes("list", buffer, listxattr("/dev/shm/x/f", buffer, sizeof buffer));
    fd = open("/dev/shm/x/f", O_WRONLY);
    show("write", write(fd, "x", 1));
    show("caps after write", getxattr("/dev/shm/x/f", "security.capability", buffer, sizeof buffer));
    show("caps v2", setxattr("/dev/shm/x/f", "security.capability", v2, sizeof v2, 0));
    show("chown", chown("/dev/shm/x/f", -1, -1));
    show("caps after chown", getxattr("/dev/shm/x/f", "security.capability", buffer, sizeof buffer));
    show("caps empty", setxattr("/dev/shm/x/f", "security.capability", "", 0, 0));
    show("caps empty read", getxattr("/dev/shm/x/f", "security.capability", buffer, sizeof buffer));
    show("truncate", truncate("/dev/shm/x/f", 0));
    show("caps empty after truncate", getxattr("/dev/shm/x/f", "security.capability", NULL, 0));
}

static void credentials(const char *stage) {
    unsigned uid[3], gid[3];
    struct {
        unsigned version;
        int pid;
    } header = {0x20080522, 0};
    struct {
        unsigned effective, permitted, inheritable;
    } data[2];
    getresuid(&uid[0], &uid[1], &uid[2]);
    getresgid(&gid[0], &gid[1], &gid[2]);
    syscall(SYS_capget, &header, data);
    printf("%s: uid %u %u %u gid %u %u %u caps %x %x %x secure %lu dumpable %d\n", stage, uid[0], uid[1],
           uid[2], gid[0], gid[1], gid[2], data[0].effective, data[0].permitted, data[0].inheritable,
           getauxval(AT_SECURE), prctl(PR_GET_DUMPABLE));
}

static void copy(const char *from, const char *to, mode_t mode, uid_t owner, gid_t group) {
    static char bytes[4 << 20];
    int in = open(from, O_RDONLY);
    long length = read(in, bytes, sizeof bytes);
    close(in);
    int out = open(to, O_CREAT | O_WRONLY | O_TRUNC, 0755);
    write(out, bytes, length);
    close(out);
    chown(to, owner, group);
    chmod(to, mode);
}

static void run(const char *path, const char *stage) {
    fflush(stdout);
    char *arguments[] = {(char *)path, (char *)stage, NULL};
    char *environment[] = {NULL};
    execve(path, arguments, environment);
    printf("execve %s %d\n", path, errno);
}

int main(int argc, char **argv) {
    if (argc < 2) {
        attributes();
        // Raise CAP_NET_RAW and CAP_KILL (in the low word), made effective.
        unsigned char caps[20] = {1, 0, 0, 2, 0x20, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
        copy(argv[0], "/tmp/plain", 0755, 0, 0);
        copy(argv[0], "/tmp/setgid", 02755, 0, 1000);
        copy(argv[0], "/tmp/setuid", 04755, 1000, 0);
        copy(argv[0], "/tmp/unreadable", 0711, 0, 0);
        copy(argv[0], "/tmp/capable", 0755, 0, 0);
        copy(argv[0], "/tmp/root", 04755, 0, 0);
        copy(argv[0], "/tmp/both", 04755, 0, 0);
        show("set caps", setxattr("/tmp/capable", "security.capability", caps, sizeof caps, 0));
        show("set caps", setxattr("/tmp/both", "security.capability", caps, sizeof caps, 0));
        // A stored set this caller cannot grant: CAP_SYS_ADMIN.
        caps[6] = 0x20;
        copy(argv[0], "/tmp/toomuch", 0755, 0, 0);
        show("set caps", setxattr("/tmp/toomuch", "security.capability", caps, sizeof caps, 0));
        credentials("start");
        run("/tmp/plain", "plain");
    } else if (!strcmp(argv[1], "plain")) {
        credentials(argv[1]);
        run("/tmp/setgid", "setgid");
    } else if (!strcmp(argv[1], "setgid")) {
        credentials(argv[1]);
        run("/tmp/setuid", "setuid");
    } else if (!strcmp(argv[1], "setuid")) {
        credentials(argv[1]);
        show("setresuid", setresuid(1000, 1000, 1000));
        show("setresgid", setresgid(1000, 1000, 1000));
        credentials("user");
        run("/tmp/unreadable", "unreadable");
    } else if (!strcmp(argv[1], "unreadable")) {
        credentials(argv[1]);
        run("/tmp/toomuch", "toomuch");
        run("/tmp/capable", "capable");
    } else if (!strcmp(argv[1], "capable")) {
        credentials(argv[1]);
        run("/tmp/both", "both");
    } else if (!strcmp(argv[1], "both")) {
        credentials(argv[1]);
        run("/tmp/root", "root");
    } else if (!strcmp(argv[1], "root")) {
        credentials(argv[1]);
    }
    return 0;
}
