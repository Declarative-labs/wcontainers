// Opening device nodes made with mknod: Docker's device rules allow only the standard character
// devices, so the others are EPERM. With an argument "tun", it opens the network tunnel, which Docker
// allows and which is not built here.

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <unistd.h>

static void try(const char *path, mode_t kind, int major, int minor) {
    unlink(path);
    int made = mknod(path, kind | 0600, makedev(major, minor));
    int fd = open(path, O_RDWR | O_NONBLOCK);
    printf("%s %d:%d: mknod %d, open %s\n", kind == S_IFCHR ? "char" : "block", major, minor, made,
           fd < 0 ? strerror(errno) : "ok");
    if (fd >= 0) {
        close(fd);
    }
    unlink(path);
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "tun") == 0) {
        try("/tmp/device", S_IFCHR, 10, 200);
        return 0;
    }
    try("/tmp/device", S_IFCHR, 1, 3);
    try("/tmp/device", S_IFCHR, 1, 1);
    try("/tmp/device", S_IFCHR, 200, 7);
    try("/tmp/device", S_IFCHR, 4, 1);
    try("/tmp/device", S_IFBLK, 7, 0);
    try("/tmp/device", S_IFBLK, 1, 3);
    return 0;
}
