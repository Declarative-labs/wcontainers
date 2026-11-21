// Permission checks on the process information tree: access, removing and making names, as root and
// after giving up the effective root user.

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

static void show(const char *what, const char *path, int result) {
    printf("%s %s %d\n", what, path, result ? errno : 0);
}

int main(void) {
    static const char *paths[] = {"/proc/self/", "/proc/self/fd/", "/proc/self/comm", "/proc/self/status",
                                  "/proc/", "/proc/sys/", "/proc/self/cwd", "/proc/cpuinfo", "/proc/self/fd/1"};
    for (int pass = 0; pass < 2; pass++) {
        for (unsigned i = 0; i < sizeof paths / sizeof paths[0]; i++) {
            show("access w", paths[i], access(paths[i], W_OK));
            show("access rx", paths[i], access(paths[i], R_OK | X_OK));
            show("faccessat eaccess w", paths[i], faccessat(AT_FDCWD, paths[i], W_OK, AT_EACCESS));
            show("rmdir", paths[i], unlinkat(AT_FDCWD, paths[i], AT_REMOVEDIR));
            show("unlink", paths[i], unlinkat(AT_FDCWD, paths[i], 0));
        }
        show("mkdir", "/proc/self/new", mkdirat(AT_FDCWD, "/proc/self/new", 0777));
        show("mkdir", "/proc/new", mkdirat(AT_FDCWD, "/proc/new", 0777));
        show("create", "/proc/self/new", open("/proc/self/new", O_CREAT | O_WRONLY, 0644) < 0);
        show("unlink missing", "/proc/self/nope", unlinkat(AT_FDCWD, "/proc/self/nope", 0));
        setreuid(0, 1000);
    }
    return 0;
}
