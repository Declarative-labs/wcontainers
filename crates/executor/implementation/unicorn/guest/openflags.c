// The flags fcntl(F_GETFL) reports for the standard descriptors and for files opened in different
// ways.

#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <linux/openat2.h>

static void show(const char *what, int fd) {
    printf("%s: %#o\n", what, fd < 0 ? -1 : fcntl(fd, F_GETFL));
}

int main(void) {
    show("stdin", 0);
    show("stdout", 1);
    show("stderr", 2);
    show("read", open("/dev/null", O_RDONLY));
    show("write append", open("/dev/shm/f", O_WRONLY | O_CREAT | O_APPEND | O_TRUNC | O_EXCL, 0600));
    show("cloexec nonblock", open("/dev/zero", O_RDWR | O_CLOEXEC | O_NONBLOCK | O_NOCTTY));
    show("path", open("/dev/shm/f", O_PATH));
    show("path directory nofollow", open("/dev/shm", O_PATH | O_DIRECTORY | O_NOFOLLOW | O_RDWR | O_APPEND));
    show("directory", open("/dev/shm", O_RDONLY | O_DIRECTORY));
    show("sync", open("/dev/shm/f", O_RDWR | O_SYNC | O_NOATIME));
    show("reopened", open("/proc/self/fd/0", O_RDONLY));
    struct open_how how = {.flags = O_RDONLY};
    show("openat2", syscall(SYS_openat2, AT_FDCWD, "/dev/shm/f", &how, sizeof how));
    how.flags = O_PATH;
    show("openat2 path", syscall(SYS_openat2, AT_FDCWD, "/dev/shm/f", &how, sizeof how));
    show("memfd", memfd_create("m", 0));
    int fd = open("/dev/shm/f", O_RDONLY);
    fcntl(fd, F_SETFL, O_APPEND | O_NONBLOCK | O_RDWR);
    show("after F_SETFL", fd);
    fcntl(fd, F_SETFL, 0);
    show("after F_SETFL 0", fd);
    show("dup", dup(fd));
    return 0;
}
