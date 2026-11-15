// name_to_handle_at on tmpfs, devices, /proc and devpts: flags, sizes, types, and what the handle and
// mount number hold compared with stat and statx (the numbers themselves differ between machines).

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

#ifndef AT_HANDLE_MNT_ID_UNIQUE
#define AT_HANDLE_MNT_ID_UNIQUE 1
#endif
#ifndef AT_HANDLE_CONNECTABLE
#define AT_HANDLE_CONNECTABLE 2
#endif
#ifndef AT_HANDLE_FID
#define AT_HANDLE_FID 0x200
#endif

struct handle {
    uint32_t bytes;
    int32_t type;
    uint32_t words[32];
};

static void show(const char *what, int dirfd, const char *path, uint32_t size, int flags) {
    struct handle handle;
    memset(&handle, 0xee, sizeof handle);
    handle.bytes = size;
    uint64_t mount = 0xeeeeeeeeeeeeeeee;
    long result = syscall(SYS_name_to_handle_at, dirfd, path, &handle, &mount, flags);
    printf("%s: %ld errno %d: bytes %u type %#x", what, result, result ? errno : 0, handle.bytes, handle.type);
    if (handle.bytes <= 128 && size <= 128) {
        unsigned words = (size < handle.bytes ? size : handle.bytes) / 4;
        printf(" next %#x", words < 32 ? handle.words[words] : 0);
    }
    // Which words are the inode number, and whether the mount number is statx's.
    // statx by its raw layout: the C library's has no mount number.
    uint64_t stx[32];
    int empty = path[0] == 0 ? AT_EMPTY_PATH : 0;
    int follow = flags & AT_SYMLINK_FOLLOW ? 0 : AT_SYMLINK_NOFOLLOW;
    unsigned mask = 0x100 | (flags & AT_HANDLE_MNT_ID_UNIQUE ? 0x4000 : 0x1000);
    if (result == 0 && syscall(SYS_statx, dirfd, path, empty | follow, mask, stx) == 0) {
        uint64_t ino = stx[4], mount_number = stx[18];
        for (unsigned i = 0; i < handle.bytes / 4; i++) {
            if (handle.words[i] == (uint32_t)ino) {
                printf(" ino-low@%u", i);
            } else if (handle.words[i] == (uint32_t)(ino >> 32)) {
                printf(" ino-high@%u", i);
            } else {
                printf(" other@%u", i);
            }
        }
        uint64_t expected = flags & AT_HANDLE_MNT_ID_UNIQUE ? mount_number : (uint32_t)mount_number;
        uint64_t got = flags & AT_HANDLE_MNT_ID_UNIQUE ? mount : (uint32_t)mount;
        printf(" mount %s upper %#x", got == expected ? "same" : "differs", (unsigned)(mount >> 32));
    } else if (result != 0) {
        printf(" mount %s", mount == 0xeeeeeeeeeeeeeeee ? "untouched" : "written");
    }
    printf("\n");
}

int main(void) {
    int fd = open("/dev/shm/h", O_RDWR | O_CREAT, 0644);
    mkdir("/dev/shm/d", 0755);
    symlink("h", "/dev/shm/l");
    show("file", AT_FDCWD, "/dev/shm/h", 128, 0);
    show("exact size", AT_FDCWD, "/dev/shm/h", 12, 0);
    show("too small", AT_FDCWD, "/dev/shm/h", 8, 0);
    show("none", AT_FDCWD, "/dev/shm/h", 0, 0);
    show("too large", AT_FDCWD, "/dev/shm/h", 129, 0);
    show("odd size", AT_FDCWD, "/dev/shm/h", 13, 0);
    show("directory", AT_FDCWD, "/dev/shm/d", 128, 0);
    show("link", AT_FDCWD, "/dev/shm/l", 128, 0);
    show("link followed", AT_FDCWD, "/dev/shm/l", 128, AT_SYMLINK_FOLLOW);
    show("missing", AT_FDCWD, "/dev/shm/nope", 128, 0);
    show("empty path", fd, "", 128, AT_EMPTY_PATH);
    show("empty path, no flag", fd, "", 128, 0);
    show("relative", AT_FDCWD, "h", 128, 0);
    show("unique mount", AT_FDCWD, "/dev/shm/h", 128, AT_HANDLE_MNT_ID_UNIQUE);
    show("fid", AT_FDCWD, "/dev/shm/h", 128, AT_HANDLE_FID);
    show("connectable", AT_FDCWD, "/dev/shm/h", 128, AT_HANDLE_CONNECTABLE);
    show("connectable directory", AT_FDCWD, "/dev/shm/d", 128, AT_HANDLE_CONNECTABLE);
    show("connectable fid", AT_FDCWD, "/dev/shm/h", 128, AT_HANDLE_CONNECTABLE | AT_HANDLE_FID);
    show("connectable empty", fd, "", 128, AT_HANDLE_CONNECTABLE | AT_EMPTY_PATH);
    show("unknown flag", AT_FDCWD, "/dev/shm/h", 128, 4);
    show("removedir flag", AT_FDCWD, "/dev/shm/h", 128, AT_REMOVEDIR);
    show("device", AT_FDCWD, "/dev/null", 128, 0);
    show("dev", AT_FDCWD, "/dev", 128, 0);
    show("proc", AT_FDCWD, "/proc/self/status", 128, 0);
    show("proc fid", AT_FDCWD, "/proc/self/status", 128, AT_HANDLE_FID);
    show("proc fid small", AT_FDCWD, "/proc/self/status", 4, AT_HANDLE_FID);
    show("proc root fid", AT_FDCWD, "/proc", 128, AT_HANDLE_FID);
    show("proc sys fid", AT_FDCWD, "/proc/sys/kernel/hostname", 128, AT_HANDLE_FID);
    show("devpts", AT_FDCWD, "/dev/pts/ptmx", 128, 0);
    show("devpts fid", AT_FDCWD, "/dev/pts/ptmx", 128, AT_HANDLE_FID);
    show("shm", AT_FDCWD, "/dev/shm", 128, 0);

    // Two handles of one file are the same; of two files, they differ.
    struct handle first, second, other;
    uint64_t mount;
    first.bytes = second.bytes = other.bytes = 128;
    syscall(SYS_name_to_handle_at, AT_FDCWD, "/dev/shm/h", &first, &mount, 0);
    syscall(SYS_name_to_handle_at, AT_FDCWD, "/dev/shm/h", &second, &mount, 0);
    syscall(SYS_name_to_handle_at, AT_FDCWD, "/dev/shm/d", &other, &mount, 0);
    printf("same file: %d\n", memcmp(first.words, second.words, first.bytes) == 0);
    printf("other file: %d\n", memcmp(first.words, other.words, first.bytes) == 0);

    long result = syscall(SYS_name_to_handle_at, AT_FDCWD, "/dev/shm/h", (void *)8, &mount, 0);
    printf("bad handle: %ld errno %d\n", result, errno);
    struct handle handle = {.bytes = 128};
    result = syscall(SYS_name_to_handle_at, AT_FDCWD, "/dev/shm/h", &handle, (void *)8, 0);
    printf("bad mount: %ld errno %d, bytes %u\n", result, errno, handle.bytes);
    handle.bytes = 0;
    result = syscall(SYS_name_to_handle_at, AT_FDCWD, "/dev/shm/h", &handle, (void *)8, 0);
    printf("bad mount, too small: %ld errno %d, bytes %u\n", result, errno, handle.bytes);
    result = syscall(SYS_name_to_handle_at, AT_FDCWD, (void *)8, &handle, &mount, 0);
    printf("bad path: %ld errno %d\n", result, errno);
    return 0;
}
