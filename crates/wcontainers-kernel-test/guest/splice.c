// splice, tee and vmsplice: between pipes, from files into pipes and from pipes into files, with
// offsets, flags, refusals, which slots a later write may add to, waits, and vmsplice both ways.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/sendfile.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

static char buffer[70000];

// What a pipe holds, read without waiting.
static void drain(const char *what, int fd) {
    int flags = fcntl(fd, F_GETFL);
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    long got = read(fd, buffer, sizeof buffer);
    printf("%s: %ld errno %d", what, got, got < 0 ? errno : 0);
    if (got > 0) {
        printf(" first %02x last %02x", (unsigned char)buffer[0], (unsigned char)buffer[got - 1]);
    }
    printf("\n");
    fcntl(fd, F_SETFL, flags);
}

static void make(int fds[2], int pages) {
    pipe(fds);
    if (pages) {
        fcntl(fds[1], F_SETPIPE_SZ, pages * 4096);
    }
}

static void fill(int fd, int count, char byte) {
    memset(buffer, byte, count);
    write(fd, buffer, count);
}

static void between_pipes(void) {
    int a[2], b[2];
    make(a, 0);
    make(b, 2);
    result("nothing asked", splice(a[0], NULL, b[1], NULL, 0, 0));
    result("unknown flag", splice(a[0], NULL, b[1], NULL, 1, 16));
    result("bad descriptor", splice(99, NULL, b[1], NULL, 1, 0));
    result("from a write end", splice(a[1], NULL, b[1], NULL, 1, 0));
    result("into a read end", splice(a[0], NULL, b[0], NULL, 1, 0));
    loff_t offset = 0;
    result("offset on a pipe", splice(a[0], &offset, b[1], NULL, 1, 0));
    result("same pipe", splice(a[0], NULL, a[1], NULL, 1, 0));
    result("empty nonblocking", splice(a[0], NULL, b[1], NULL, 100, SPLICE_F_NONBLOCK));
    // A whole slot moves as it is, and a later small write may still add to it.
    fill(a[1], 100, 'a');
    result("whole slot", splice(a[0], NULL, b[1], NULL, 1000, 0));
    fill(b[1], 100, 'b');
    fcntl(b[1], F_SETFL, O_NONBLOCK);
    memset(buffer, 'c', 8192);
    result("after a moved slot", write(b[1], buffer, 8192));
    drain("moved slot read", b[0]);
    // Part of a slot is a copy no write adds to.
    fill(a[1], 100, 'd');
    result("part of a slot", splice(a[0], NULL, b[1], NULL, 50, 0));
    fill(b[1], 100, 'e');
    result("after a copied part", write(b[1], buffer, 8192));
    drain("copied part read", b[0]);
    drain("rest of the input", a[0]);
    // Several slots, up to the output's room.
    for (int i = 0; i < 4; i++) {
        fill(a[1], 4096, '0' + i);
    }
    result("into room for two", splice(a[0], NULL, b[1], NULL, 65536, 0));
    result("full nonblocking", splice(a[0], NULL, b[1], NULL, 65536, SPLICE_F_NONBLOCK));
    drain("two slots", b[0]);
    drain("two left", a[0]);
    close(a[1]);
    result("no writers", splice(a[0], NULL, b[1], NULL, 100, 0));
    close(a[0]);
    make(a, 0);
    fill(a[1], 10, 'f');
    close(b[0]);
    signal(SIGPIPE, SIG_IGN);
    result("no readers", splice(a[0], NULL, b[1], NULL, 100, 0));
    close(a[0]);
    close(a[1]);
    close(b[1]);
}

static void files_and_pipes(void) {
    int file = open("/tmp/splice", O_RDWR | O_CREAT | O_TRUNC, 0600);
    for (int i = 0; i < 10000; i++) {
        buffer[i] = (char)i;
    }
    write(file, buffer, 10000);
    int p[2];
    make(p, 2);
    loff_t offset = 100;
    result("file at an offset into room for two", splice(file, &offset, p[1], NULL, 10000, 0));
    printf("offset now %ld position %ld\n", (long)offset, (long)lseek(file, 0, SEEK_CUR));
    drain("file pages", p[0]);
    lseek(file, 5000, SEEK_SET);
    result("file at its position", splice(file, NULL, p[1], NULL, 3000, 0));
    printf("position now %ld\n", (long)lseek(file, 0, SEEK_CUR));
    // A page of a file is a slot no write adds to.
    result("small write after", write(p[1], "xyz", 3));
    fcntl(p[1], F_SETFL, O_NONBLOCK);
    result("and a page", write(p[1], buffer, 4096));
    drain("file slot read", p[0]);
    fcntl(p[1], F_SETFL, 0);
    offset = -1;
    result("negative offset", splice(file, &offset, p[1], NULL, 10, 0));
    result("offset from nowhere", splice(file, (loff_t *)16, p[1], NULL, 10, 0));
    int zero = open("/dev/zero", O_RDONLY);
    result("zero", splice(zero, NULL, p[1], NULL, 5000, 0));
    printf("zero position %ld\n", (long)lseek(zero, 0, SEEK_CUR));
    drain("zeros", p[0]);
    int null = open("/dev/null", O_RDWR);
    result("from null", splice(null, NULL, p[1], NULL, 10, 0));
    int directory = open("/tmp", O_RDONLY);
    result("from a directory", splice(directory, NULL, p[1], NULL, 10, 0));
    int version = open("/proc/version", O_RDONLY);
    long got = splice(version, NULL, p[1], NULL, 5, 0);
    result("from /proc/version", got);
    drain("version bytes", p[0]);
    int status = open("/proc/self/status", O_RDONLY);
    result("from /proc/self/status", splice(status, NULL, p[1], NULL, 5, 0) > 0 ? 0 : -1);
    drain("status bytes", p[0]);
    int stat = open("/proc/self/stat", O_RDONLY);
    result("from /proc/self/stat", splice(stat, NULL, p[1], NULL, 5, 0) > 0 ? 0 : -1);
    drain("stat bytes", p[0]);
    // Pipe into files.
    fill(p[1], 3000, 'g');
    offset = 20000;
    result("pipe into a file at an offset", splice(p[0], NULL, file, &offset, 1000, 0));
    printf("offset now %ld position %ld size %ld\n", (long)offset, (long)lseek(file, 0, SEEK_CUR),
           (long)lseek(file, 0, SEEK_END));
    lseek(file, 0, SEEK_SET);
    result("pipe into a file at its position", splice(p[0], NULL, file, NULL, 500, 0));
    printf("position now %ld\n", (long)lseek(file, 0, SEEK_CUR));
    result("into null", splice(p[0], NULL, null, NULL, 700, 0));
    printf("null position %ld\n", (long)lseek(null, 0, SEEK_CUR));
    int full = open("/dev/full", O_WRONLY);
    result("into full", splice(p[0], NULL, full, NULL, 10, 0));
    int appending = open("/tmp/splice", O_WRONLY | O_APPEND);
    result("into an appending file", splice(p[0], NULL, appending, NULL, 10, 0));
    int proc = open("/proc/self/comm", O_WRONLY);
    result("into /proc/self/comm", splice(p[0], NULL, proc, NULL, 10, 0));
    offset = 0;
    int pipe_fd = p[0];
    result("offset for a pipe output", splice(file, NULL, p[1], &offset, 10, 0));
    result("rest into a file", splice(pipe_fd, NULL, file, NULL, 100000, 0));
    result("empty into a file nonblocking", splice(p[0], NULL, file, NULL, 10, SPLICE_F_NONBLOCK));
    result("file to file", splice(file, NULL, file, NULL, 10, 0));
    close(p[1]);
    result("no writers into a file", splice(p[0], NULL, file, NULL, 10, 0));
    close(p[0]);
    close(file);
    close(zero);
    close(null);
    close(directory);
    close(version);
    close(status);
    close(stat);
    close(full);
    close(appending);
    close(proc);
}

static void teeing(void) {
    int a[2], b[2];
    make(a, 0);
    make(b, 2);
    fill(a[1], 5000, 'h');
    result("tee", tee(a[0], b[1], 100000, 0));
    result("tee again", tee(a[0], b[1], 100000, SPLICE_F_NONBLOCK));
    result("tee part", tee(a[0], b[1], 10, SPLICE_F_NONBLOCK));
    drain("teed", b[0]);
    drain("input kept", a[0]);
    result("tee empty nonblocking", tee(a[0], b[1], 10, SPLICE_F_NONBLOCK));
    result("tee same pipe", tee(a[0], a[1], 10, 0));
    int file = open("/tmp/splice", O_RDWR);
    result("tee from a file", tee(file, b[1], 10, 0));
    result("tee unknown flag", tee(a[0], b[1], 10, 32));
    result("tee nothing", tee(a[0], b[1], 0, 0));
    result("tee wrong ends", tee(a[1], b[0], 10, 0));
    close(a[1]);
    result("tee no writers", tee(a[0], b[1], 10, 0));
    close(file);
    close(a[0]);
    close(b[0]);
    close(b[1]);
}

static void user_pages(void) {
    int p[2];
    make(p, 4);
    char *pages = mmap(NULL, 3 * 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    memset(pages, 'i', 3 * 4096);
    // Across a page boundary: two slots.
    struct iovec across = {pages + 4000, 200};
    result("vmsplice across pages", vmsplice(p[1], &across, 1, 0));
    struct iovec two[2] = {{pages, 10}, {pages + 4096, 4096}};
    result("vmsplice two parts", vmsplice(p[1], two, 2, 0));
    struct iovec more = {pages, 3 * 4096};
    result("vmsplice into the last slot", vmsplice(p[1], &more, 1, SPLICE_F_NONBLOCK));
    result("vmsplice full", vmsplice(p[1], &more, 1, SPLICE_F_NONBLOCK));
    char out[9000];
    memset(out, 0, sizeof out);
    struct iovec back[2] = {{out, 100}, {out + 100, 8000}};
    result("vmsplice to the caller", vmsplice(p[0], back, 2, 0));
    printf("got %c %c %c\n", out[0], out[150], out[8099]);
    drain("left", p[0]);
    result("vmsplice empty nonblocking", vmsplice(p[0], back, 2, SPLICE_F_NONBLOCK));
    struct iovec nowhere = {NULL, 10};
    result("vmsplice from nowhere", vmsplice(p[1], &nowhere, 1, 0));
    mprotect(pages + 4096, 4096, PROT_NONE);
    struct iovec partly = {pages + 4000, 1000};
    result("vmsplice up to a hole", vmsplice(p[1], &partly, 1, 0));
    drain("before the hole", p[0]);
    result("vmsplice nothing", vmsplice(p[1], two, 0, 0));
    result("vmsplice unknown flag", vmsplice(p[1], two, 1, 64));
    int file = open("/tmp/splice", O_RDWR);
    result("vmsplice into a file", vmsplice(file, two, 1, 0));
    // Into a buffer that faults part way: the slot stays.
    fill(p[1], 300, 'j');
    struct iovec faulting = {pages + 4096 - 100, 300};
    result("vmsplice to a hole", vmsplice(p[0], &faulting, 1, 0));
    printf("stored before the hole %c\n", pages[4095]);
    drain("slot kept", p[0]);
    close(p[1]);
    result("vmsplice no writers", vmsplice(p[0], back, 2, 0));
    close(p[0]);
    close(file);
}

// A splice that waits for its input, and one that waits for room.
static void waiting(void) {
    int a[2], b[2];
    make(a, 0);
    make(b, 1);
    fflush(stdout);
    pid_t child = fork();
    if (child == 0) {
        struct timespec pause = {0, 50000000};
        nanosleep(&pause, NULL);
        fill(a[1], 10, 'k');
        nanosleep(&pause, NULL);
        // Its exit status says what it read: which side prints first after a wake is the
        // scheduler's choice, and the reference has several CPUs.
        _exit((int)read(b[0], buffer, sizeof buffer));
    }
    result("waited for input", splice(a[0], NULL, b[1], NULL, 100, 0));
    fill(a[1], 4096, 'l');
    result("waited for room", splice(a[0], NULL, b[1], NULL, 4096, 0));
    int status;
    waitpid(child, &status, 0);
    printf("child read %d\n", WEXITSTATUS(status));
    drain("after the wait", b[0]);
    // sendfile into a full pipe waits for room as well.
    int file = open("/tmp/splice", O_RDONLY);
    fill(b[1], 4096, 'm');
    fflush(stdout);
    child = fork();
    if (child == 0) {
        struct timespec pause = {0, 50000000};
        nanosleep(&pause, NULL);
        _exit((int)read(b[0], buffer, 4096) / 16);
    }
    result("sendfile waited for room", sendfile(b[1], file, NULL, 100));
    waitpid(child, &status, 0);
    printf("child read %d\n", WEXITSTATUS(status));
    drain("after sendfile", b[0]);
}

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    between_pipes();
    files_and_pipes();
    teeing();
    user_pages();
    waiting();
    return 0;
}
