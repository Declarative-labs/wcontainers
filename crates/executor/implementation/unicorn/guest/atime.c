// When reads update a file's access time: relatime on /dev/shm, strictatime on /dev, O_NOATIME and
// the no-access-time inode flag, for each way of reading. Prints only whether the time moved.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/fs.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/sendfile.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/uio.h>
#include <time.h>
#include <unistd.h>

static struct timespec last;
static char buffer[64];

static struct timespec atime_of(const char *path) {
    struct stat st;
    lstat(path, &st);
    return st.st_atim;
}

static void pause_a_little(void) {
    struct timespec pause = {0, 5000000};
    nanosleep(&pause, 0);
}

static void start(const char *path) {
    last = atime_of(path);
    pause_a_little();
}

static void check(const char *what, const char *path) {
    struct timespec now = atime_of(path);
    int moved = now.tv_sec != last.tv_sec || now.tv_nsec != last.tv_nsec;
    printf("%-40s %s\n", what, moved ? "moved" : "kept");
    last = now;
    pause_a_little();
}

static void reads(const char *dir, const char *file) {
    char link[256];
    snprintf(link, sizeof(link), "%s/link", dir);
    int fd = open(file, O_RDWR | O_CREAT, 0644);
    write(fd, "0123456789", 10);
    printf("%s:\n", file);
    start(file);
    pread(fd, buffer, 4, 0);
    check("read after a write", file);
    pread(fd, buffer, 4, 0);
    check("read again", file);
    pread(fd, buffer, 0, 0);
    check("read of nothing", file);
    pread(fd, buffer, 4, 100);
    check("read past the end", file);
    write(fd, "x", 1);
    check("write", file);
    struct iovec part = {buffer, 4};
    preadv(fd, &part, 1, 0);
    check("readv after a write", file);
    write(fd, "x", 1);
    int out = open("/dev/null", O_WRONLY);
    off_t offset = 0;
    sendfile(out, fd, &offset, 4);
    check("sendfile from it", file);
    write(fd, "x", 1);
    int copy = open("/dev/shm/copy", O_RDWR | O_CREAT, 0644);
    loff_t in_offset = 0;
    copy_file_range(fd, &in_offset, copy, 0, 4, 0);
    check("copy_file_range from it", file);
    write(fd, "x", 1);
    void *map = mmap(0, 4096, PROT_READ, MAP_SHARED, fd, 0);
    check("mmap", file);
    munmap(map, 4096);
    write(fd, "x", 1);
    int noatime = open(file, O_RDONLY | O_NOATIME);
    pread(noatime, buffer, 4, 0);
    check("read with O_NOATIME", file);
    close(noatime);
    unsigned long flags = FS_NOATIME_FL;
    ioctl(fd, FS_IOC_SETFLAGS, &flags);
    check("setting the flag", file);
    write(fd, "x", 1);
    pread(fd, buffer, 4, 0);
    check("read with the no-atime flag", file);
    flags = 0;
    ioctl(fd, FS_IOC_SETFLAGS, &flags);
    pread(fd, buffer, 4, 0);
    check("read after the flag is cleared", file);
    close(fd);
    close(copy);
    close(out);
    symlink("target", link);
    printf("%s:\n", link);
    start(link);
    readlink(link, buffer, sizeof(buffer));
    check("readlink", link);
    readlink(link, buffer, sizeof(buffer));
    check("readlink again", link);
    printf("%s:\n", dir);
    start(dir);
    DIR *d = opendir(dir);
    while (readdir(d)) {
    }
    check("listing", dir);
    rewinddir(d);
    while (readdir(d)) {
    }
    check("listing again", dir);
    closedir(d);
}

int main(void) {
    mkdir("/dev/shm/t", 0755);
    reads("/dev/shm/t", "/dev/shm/t/f");
    mkdir("/dev/t", 0755);
    reads("/dev/t", "/dev/t/f");
    return 0;
}
