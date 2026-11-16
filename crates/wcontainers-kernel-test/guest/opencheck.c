// Opens with a trailing slash, opens at the descriptor limit, copies between file systems, device
// positions and changes to /proc's own attributes, printing each call and what it gave.

#define _GNU_SOURCE
#include <errno.h>
#include <grp.h>
#include <sys/fsuid.h>
#include <fcntl.h>
#include <linux/openat2.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

#ifndef SYS_memfd_secret
#define SYS_memfd_secret 447
#endif

#define R(expr)                                                                                    \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long r_ = (long)(expr);                                                                    \
        printf("%-64s = %ld (%s)\n", #expr, r_, r_ < 0 ? strerror(errno) : "-");                 \
    } while (0)

static void mode_of(const char *path) {
    struct stat st;
    if (lstat(path, &st) == 0) {
        printf("%s: mode %o owner %d group %d\n", path, st.st_mode, st.st_uid, st.st_gid);
    } else {
        printf("%s: %s\n", path, strerror(errno));
    }
}

int main(void) {
    char buffer[512];
    R(mkdir("/dev/shm/t", 0755));
    R(chdir("/dev/shm/t"));
    R(close(open("f", O_CREAT | O_WRONLY, 0644)));
    R(mkdir("d", 0755));
    R(symlink("f", "lf"));
    R(symlink("d", "ld"));
    // A trailing slash asks for a directory.
    R(open("f/", O_RDONLY));
    R(open("f/", O_WRONLY));
    R(open("f/", O_PATH));
    R(open("f/", O_CREAT | O_WRONLY, 0644));
    R(open("g/", O_CREAT | O_WRONLY, 0644));
    R(open("f/", O_RDONLY | O_NOFOLLOW));
    R(open("lf/", O_RDONLY));
    R(open("lf/", O_RDONLY | O_NOFOLLOW));
    R(open("lf/", O_PATH | O_NOFOLLOW));
    R(close(open("d/", O_RDONLY)));
    R(close(open("ld/", O_RDONLY | O_NOFOLLOW)));
    R(close(open("ld/", O_PATH | O_NOFOLLOW)));
    R(open("/dev/null/", O_RDONLY));
    R(open("/etc/passwd/", O_RDONLY));
    R(open("/proc/self/status/", O_RDONLY));
    R(open("/proc/self/exe/", O_RDONLY));
    R(close(open("/proc/self/cwd/", O_RDONLY)));
    R(access("f/", F_OK));
    R(truncate("f/", 0));
    // Copies need one superblock: memory files share one, apart from /dev/shm's.
    int memory = memfd_create("m", 0);
    int other = memfd_create("n", 0);
    int secret = syscall(SYS_memfd_secret, 0);
    int file = open("f", O_RDWR);
    R(write(memory, "abcdef", 6));
    R(write(file, "ghijkl", 6));
    R(copy_file_range(memory, &(off_t){0}, other, &(off_t){0}, 6, 0));
    R(copy_file_range(memory, &(off_t){0}, file, &(off_t){0}, 6, 0));
    R(copy_file_range(file, &(off_t){0}, memory, &(off_t){0}, 6, 0));
    R(copy_file_range(memory, &(off_t){0}, secret, &(off_t){0}, 6, 0));
    R(copy_file_range(file, &(off_t){0}, file, &(off_t){10}, 6, 0));
    R(copy_file_range(memory, &(off_t){0}, file, &(off_t){0}, 0, 0));
    // linkat with AT_EMPTY_PATH walks from a descriptor only with the credentials it was opened with:
    // each committed change of credentials makes new ones.
    int before = open("f", O_RDONLY);
    R(linkat(before, "", AT_FDCWD, "l1", AT_EMPTY_PATH));
    R(setresuid(-1, -1, -1));
    R(linkat(before, "", AT_FDCWD, "l2", AT_EMPTY_PATH));
    R(setuid(0));
    R(linkat(before, "", AT_FDCWD, "l3", AT_EMPTY_PATH));
    R(linkat(before, "/dev/shm/t/f", AT_FDCWD, "l4", AT_EMPTY_PATH));
    R(linkat(before, "x", AT_FDCWD, "l5", AT_EMPTY_PATH));
    R(linkat(before, "", AT_FDCWD, "l5", 0));
    R(linkat(99, "", AT_FDCWD, "l5", AT_EMPTY_PATH));
    int after = open("f", O_RDONLY);
    R(linkat(after, "", AT_FDCWD, "l6", AT_EMPTY_PATH));
    R(setfsuid(0));
    R(linkat(after, "", AT_FDCWD, "l7", AT_EMPTY_PATH));
    R(setgroups(0, NULL));
    R(linkat(after, "", AT_FDCWD, "l8", AT_EMPTY_PATH));
    int dir = open(".", O_RDONLY | O_DIRECTORY);
    R(linkat(dir, "f", AT_FDCWD, "l9", AT_EMPTY_PATH));
    R(linkat(dir, "f", AT_FDCWD, "l9", 0));
    R(close(before));
    R(close(after));
    R(close(dir));
    // The random devices keep their position; the zero devices always seek to 0.
    const char *devices[] = {"/dev/urandom", "/dev/random", "/dev/zero", "/dev/full", "/dev/null"};
    for (int index = 0; index < 5; index++) {
        int device = open(devices[index], O_RDWR);
        printf("%s\n", devices[index]);
        R(read(device, buffer, 300));
        R(lseek(device, 0, SEEK_CUR));
        R(lseek(device, 10, SEEK_SET));
        R(lseek(device, 0, SEEK_CUR));
        R(write(device, buffer, 20));
        R(lseek(device, 5, SEEK_END));
        R(close(device));
    }
    // /proc keeps changes to its own attributes; a process's directory and sysctls refuse them.
    mode_of("/proc");
    R(chmod("/proc", 0751));
    mode_of("/proc");
    R(syscall(SYS_fchmodat2, AT_FDCWD, "/proc", 05140, AT_SYMLINK_NOFOLLOW));
    mode_of("/proc");
    R(chown("/proc", 1000, 1000));
    mode_of("/proc");
    R(lchown("/proc/self", 1000, 1000));
    mode_of("/proc/self");
    R(chmod("/proc/cpuinfo", 0400));
    mode_of("/proc/cpuinfo");
    R(chmod("/proc/self/", 0700));
    R(chmod("/proc/sys/kernel/hostname", 0600));
    R(chmod("/proc", 0555));
    R(chown("/proc", 0, 0));
    R(chmod("/proc/cpuinfo", 0444));
    R(lchown("/proc/self", 0, 0));
    mode_of("/proc");
    mode_of("/proc/cpuinfo");
    // At the descriptor limit the descriptor is refused before the path is looked at.
    struct rlimit limit = {6, 4096};
    R(setrlimit(RLIMIT_NOFILE, &limit));
    for (int fd = 0; fd < 6; fd++) {
        if (fcntl(fd, F_GETFD) < 0) {
            open("/dev/null", O_RDONLY);
        }
    }
    struct open_how how = {.flags = O_RDONLY};
    R(open("f", O_RDONLY));
    R(openat(99, "f", O_RDONLY));
    R(open((char *)1, O_RDONLY));
    R(open("nope/x", O_RDONLY));
    R(open("f", O_RDONLY | O_TMPFILE));
    R(open(".", O_RDWR | O_TMPFILE, 0600));
    R(open("f", O_CREAT | O_DIRECTORY));
    R(syscall(SYS_openat2, AT_FDCWD, "f", &how, sizeof how));
    R(syscall(SYS_openat2, 99, (char *)1, &how, sizeof how));
    R(memfd_create("x", 0));
    R(syscall(SYS_memfd_secret, 0));
    R(dup(0));
    return 0;
}
