// The paths /proc's links and getcwd give for removed directories and for directories outside the
// caller's root.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static void link_of(const char *what, const char *path) {
    char target[256];
    ssize_t length = readlink(path, target, sizeof target);
    printf("%s: %.*s (%zd%s)\n", what, length > 0 ? (int)length : 0, target, length,
           length < 0 ? strerror(errno) : "");
}

static void cwd(const char *what) {
    char path[256];
    printf("%s: %s\n", what, getcwd(path, sizeof path) ? path : strerror(errno));
}

int main(void) {
    mkdir("/dev/shm/gone", 0755);
    mkdir("/dev/shm/gone/inner", 0755);
    chdir("/dev/shm/gone/inner");
    int inner = open(".", O_RDONLY | O_DIRECTORY);
    int outer = open("..", O_RDONLY | O_DIRECTORY);
    rmdir("/dev/shm/gone/inner");
    link_of("removed cwd", "/proc/self/cwd");
    char path[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", inner);
    link_of("removed directory", path);
    snprintf(path, sizeof path, "/proc/self/fd/%d", outer);
    link_of("its parent", path);
    cwd("getcwd in removed");
    int dot = open(".", O_RDONLY | O_DIRECTORY);
    snprintf(path, sizeof path, "/proc/self/fd/%d", dot);
    link_of("dot opened in removed", path);
    chdir("/dev/shm/gone");
    rename("/dev/shm/gone", "/dev/shm/moved");
    link_of("renamed cwd", "/proc/self/cwd");
    snprintf(path, sizeof path, "/proc/self/fd/%d", outer);
    link_of("renamed descriptor", path);
    mkdir("/dev/shm/jail", 0755);
    mkdir("/dev/shm/jail/proc", 0755);
    int out = open("/dev/shm/moved", O_RDONLY | O_DIRECTORY);
    printf("chroot: %d\n", chroot("/dev/shm/jail"));
    cwd("getcwd outside the root");
    fchdir(out);
    cwd("getcwd outside the root again");
    chdir("/");
    printf("mount proc: no; chdir /: %s\n", getcwd(path, sizeof path));
    return 0;
}
