// Transfers between backend files, pipes and memory files: sendfile, splice, tee, copy_file_range,
// with and without offsets, then fadvise, readahead and cachestat. Prints each call's result and the
// bytes left in the files.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/sendfile.h>
#include <sys/syscall.h>
#include <unistd.h>

static void result(const char *what, long n) {
    if (n < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
    } else {
        printf("%s: %ld\n", what, n);
    }
}

static void show(const char *name) {
    char buf[256];
    int fd = open(name, O_RDONLY);
    long n = read(fd, buf, sizeof buf - 1);
    close(fd);
    if (n < 0) n = 0;
    for (long i = 0; i < n; i++) {
        if (buf[i] == 0) buf[i] = '.';
    }
    buf[n] = 0;
    printf("%s holds %ld: %s\n", name, n, buf);
}

static void drain(int pipe_read, const char *what) {
    char buf[256];
    long n = read(pipe_read, buf, sizeof buf - 1);
    if (n < 0) n = 0;
    buf[n] = 0;
    printf("%s read back %ld: %s\n", what, n, buf);
}

struct cachestat_range {
    unsigned long long off, len;
};
struct cachestat {
    unsigned long long cache, dirty, writeback, evicted, recently_evicted;
};

int main(void) {
    int src = open("source", O_RDWR | O_CREAT | O_TRUNC, 0644);
    write(src, "the quick brown fox jumps over the lazy dog\n", 44);
    int dst = open("target", O_RDWR | O_CREAT | O_TRUNC, 0644);
    int p[2];
    pipe(p);

    lseek(src, 4, SEEK_SET);
    result("sendfile file to file at position", sendfile(dst, src, 0, 6));
    printf("source position %ld, target position %ld\n", (long)lseek(src, 0, SEEK_CUR),
           (long)lseek(dst, 0, SEEK_CUR));
    off_t at = 16;
    result("sendfile file to file at offset", sendfile(dst, src, &at, 4));
    printf("offset now %ld, source position %ld\n", (long)at, (long)lseek(src, 0, SEEK_CUR));
    result("sendfile file to pipe", sendfile(p[1], src, 0, 10));
    drain(p[0], "sendfile");
    show("target");

    loff_t in_off = 0;
    result("splice file to pipe at offset", splice(src, &in_off, p[1], 0, 9, 0));
    printf("offset now %ld\n", (long)in_off);
    int q[2];
    pipe(q);
    result("tee", tee(p[0], q[1], 64, SPLICE_F_NONBLOCK));
    drain(q[0], "tee");
    loff_t out_off = 30;
    result("splice pipe to file at offset", splice(p[0], 0, dst, &out_off, 9, 0));
    printf("offset now %ld\n", (long)out_off);
    write(p[1], "XYZ", 3);
    result("splice pipe to file at position", splice(p[0], 0, dst, 0, 3, 0));
    printf("target position %ld\n", (long)lseek(dst, 0, SEEK_CUR));
    show("target");
    result("splice file to pipe at end", splice(src, 0, p[1], 0, 64, SPLICE_F_NONBLOCK));
    lseek(src, 40, SEEK_SET);
    result("splice file to pipe at position", splice(src, 0, p[1], 0, 64, 0));
    drain(p[0], "splice");

    loff_t from = 10, to = 2;
    result("copy_file_range with offsets", copy_file_range(src, &from, dst, &to, 5, 0));
    printf("offsets now %ld %ld\n", (long)from, (long)to);
    lseek(src, 0, SEEK_SET);
    lseek(dst, 0, SEEK_END);
    result("copy_file_range at positions", copy_file_range(src, 0, dst, 0, 4, 0));
    printf("positions %ld %ld\n", (long)lseek(src, 0, SEEK_CUR), (long)lseek(dst, 0, SEEK_CUR));
    result("copy_file_range past the end", copy_file_range(src, &(loff_t){100}, dst, 0, 4, 0));
    result("copy_file_range onto itself", copy_file_range(src, &(loff_t){0}, src, &(loff_t){2}, 8, 0));
    int memory = memfd_create("memory", 0);
    write(memory, "from memory", 11);
    result("copy_file_range from memory", copy_file_range(memory, &(loff_t){0}, dst, 0, 11, 0));
    result("sendfile from memory", sendfile(dst, memory, &(off_t){5}, 6));
    result("copy_file_range to memory", copy_file_range(src, &(loff_t){0}, memory, &(loff_t){0}, 3, 0));
    show("target");
    show("source");

    result("fadvise normal", posix_fadvise(src, 0, 0, POSIX_FADV_NORMAL));
    result("fadvise willneed", posix_fadvise(src, 0, 4096, POSIX_FADV_WILLNEED));
    result("fadvise dontneed", posix_fadvise(src, 0, 0, POSIX_FADV_DONTNEED));
    result("fadvise noreuse", posix_fadvise(src, 0, 0, POSIX_FADV_NOREUSE));
    result("fadvise bad", posix_fadvise(src, 0, 0, 99));
    result("readahead", readahead(src, 0, 4096));
    struct cachestat_range range = {0, 0};
    struct cachestat stat;
    result("cachestat", syscall(451, src, &range, &stat, 0));
    result("cachestat bad flags", syscall(451, src, &range, &stat, 1));
    int directory = open(".", O_RDONLY | O_DIRECTORY);
    result("readahead directory", readahead(directory, 0, 4096));
    result("splice directory", splice(directory, 0, p[1], 0, 4, SPLICE_F_NONBLOCK));
    result("sendfile directory", sendfile(dst, directory, 0, 4));
    return 0;
}
