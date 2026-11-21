// Large transfers the kernel fills itself: reads of the random devices, getrandom, and sendfile from
// devices, files and what cannot be spliced, with counts of zero. Prints each call and what it gave.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/random.h>
#include <sys/sendfile.h>
#include <sys/uio.h>
#include <unistd.h>

#define R(expr)                                                                                    \
    do {                                                                                           \
        errno = 0;                                                                                 \
        long r_ = (long)(expr);                                                                    \
        printf("%-64s = %ld (%s)\n", #expr, r_, r_ < 0 ? strerror(errno) : "-");                 \
    } while (0)

#define BIG (64L << 20)

int main(int argc, char **argv) {
    char *buffer = mmap(0, BIG + 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    // The last two pages: one writable, then one that is not.
    char *edge = buffer + BIG;
    mprotect(edge + 4096, 4096, PROT_NONE);
    int urandom = open("/dev/urandom", O_RDWR);
    int random = open("/dev/random", O_RDONLY);
    int zero = open("/dev/zero", O_RDONLY);
    int null = open("/dev/null", O_RDWR);

    R(read(urandom, buffer, BIG));
    R(read(random, buffer, 70000));
    R(pread(urandom, buffer, 100000, 0));
    struct iovec parts[2] = {{buffer, 1 << 20}, {buffer + (1 << 20), 1 << 20}};
    R(readv(urandom, parts, 2));
    // With an argument, only copies the random devices make up to a fault inside the buffer.
    if (argc > 1) {
        R(read(urandom, edge + 4000, 8192));
        R(read(urandom, edge + 4064, 8192));
        R(getrandom(edge + 4000, 8192, 0));
        return 0;
    }
    R(read(urandom, edge, 8192));
    R(read(urandom, edge + 4096, 8192));
    R(write(urandom, edge, 8192));
    R(write(urandom, edge + 100, 8192));
    R(write(urandom, edge + 4096, 8192));
    R(write(urandom, edge, 0));
    R(getrandom(buffer, BIG, 0));
    R(getrandom(buffer, BIG, GRND_NONBLOCK));
    R(getrandom(edge + 4096, 8192, 0));

    int out = open("/tmp/out", O_RDWR | O_CREAT | O_TRUNC, 0644);
    R(sendfile(out, zero, NULL, 1 << 20));
    R(sendfile(out, urandom, NULL, 1 << 20));
    R(sendfile(null, zero, NULL, 1 << 20));
    R(sendfile(null, urandom, NULL, 3 << 20));
    R(lseek(out, 0, SEEK_CUR));
    R(lseek(zero, 0, SEEK_CUR));
    off_t at = 5;
    R(sendfile(out, zero, &at, 100000));
    printf("offset after %ld\n", (long)at);
    R(sendfile(out, urandom, &at, 100000));
    printf("offset after %ld\n", (long)at);

    // From a regular file, from a position inside a page, past a pipe's worth.
    int in = open("/tmp/out", O_RDONLY);
    int copy = open("/tmp/copy", O_RDWR | O_CREAT | O_TRUNC, 0644);
    R(lseek(in, 1000, SEEK_SET));
    R(sendfile(copy, in, NULL, 300000));
    R(lseek(in, 0, SEEK_CUR));
    R(lseek(copy, 0, SEEK_CUR));
    at = 3 << 20;
    R(sendfile(copy, in, &at, 100));
    printf("offset after %ld\n", (long)at);
    at = 100;
    R(sendfile(1, in, &at, 0));
    printf("offset after %ld\n", (long)at);

    // Nothing to move is done before anything asks whether the input can be spliced.
    int directory = open("/tmp", O_RDONLY | O_DIRECTORY);
    int stat = open("/proc/self/stat", O_RDONLY);
    int cmdline = open("/proc/self/cmdline", O_RDONLY);
    int full = open("/dev/full", O_WRONLY);
    R(sendfile(out, directory, NULL, 0));
    R(sendfile(1, directory, NULL, 0));
    R(sendfile(out, null, NULL, 0));
    R(sendfile(1, null, NULL, 0));
    R(sendfile(out, stat, NULL, 0));
    at = 0;
    R(sendfile(out, cmdline, &at, 0));
    R(sendfile(full, zero, NULL, 0));
    R(sendfile(full, in, NULL, 0));
    R(sendfile(out, directory, NULL, 1));
    R(sendfile(1, directory, NULL, 1));
    R(sendfile(out, null, NULL, 1));
    R(sendfile(out, stat, NULL, 1));
    R(sendfile(out, cmdline, &at, 1));
    R(sendfile(full, zero, NULL, 1));
    R(sendfile(full, in, NULL, 10));
    R(sendfile(stat, zero, NULL, 0));
    int append = open("/tmp/out", O_WRONLY | O_APPEND);
    R(sendfile(append, zero, NULL, 0));
    R(sendfile(append, zero, NULL, 1));
    return 0;
}
