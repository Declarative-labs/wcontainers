// /proc/<pid>/mem: reads and writes at the position with FOLL_FORCE (pages with no access, read-only
// private and shared mappings, a file's end, unmapped memory, the program's own code), its seeks,
// vectored and positioned calls, another process's memory and who may open it.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/uio.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE 4096

static volatile char value[64] = "the value";

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

__attribute__((noinline)) int answer(void) {
    return 1;
}

static long position(int fd) {
    return lseek(fd, 0, SEEK_CUR);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int fd = open("/proc/self/mem", O_RDWR);
    result("open", fd < 0 ? -1 : 0);
    char buffer[2 * PAGE];

    // Seeks: any position, SEEK_END refused.
    result("seek set", lseek(fd, (long)value, SEEK_SET) == (long)value);
    result("read", read(fd, buffer, 9));
    printf("bytes %.9s, moved %ld\n", buffer, position(fd) - (long)value);
    result("seek cur back", lseek(fd, -9, SEEK_CUR) == (long)value);
    result("seek end", lseek(fd, 0, SEEK_END));
    result("seek data", lseek(fd, 0, SEEK_DATA));
    result("seek high", lseek(fd, 0x7000000000000000L, SEEK_SET) == 0x7000000000000000L);
    result("read high", read(fd, buffer, 8));
    result("position after", position(fd) == 0x7000000000000000L);
    result("pread negative", pread(fd, buffer, 8, -8192));
    result("pread wraps", pread(fd, buffer, 8192, -4096));

    // Writes through the file show in memory.
    result("pwrite", pwrite(fd, "THE", 3, (long)value));
    printf("value now %s\n", value);

    // A page with no access is read and written.
    char *none = mmap(NULL, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    strcpy(none, "hidden");
    mprotect(none, 2 * PAGE, PROT_NONE);
    result("read none", pread(fd, buffer, 6, (long)none));
    printf("none %.6s\n", buffer);
    result("write none", pwrite(fd, "shown", 5, (long)none + PAGE));
    mprotect(none, 2 * PAGE, PROT_READ);
    printf("none second page %.5s\n", none + PAGE);

    // A read-only private file mapping takes its own copy; the file is unchanged.
    int file = open("/tmp/procmem", O_RDWR | O_CREAT | O_TRUNC, 0600);
    memset(buffer, 'f', PAGE + 100);
    write(file, buffer, PAGE + 100);
    char *private = mmap(NULL, 3 * PAGE, PROT_READ, MAP_PRIVATE, file, 0);
    result("write private", pwrite(fd, "COPY", 4, (long)private + 10));
    char check[8] = "";
    pread(file, check, 4, 10);
    printf("private %.4s file %.4s\n", private + 10, check);
    result("write private across pages", pwrite(fd, buffer, 200, (long)private + PAGE - 100));
    result("read past the file end", pread(fd, buffer, 2 * PAGE, (long)private + PAGE));
    result("read wholly past the end", pread(fd, buffer, 10, (long)private + 2 * PAGE));
    result("write past the end", pwrite(fd, "x", 1, (long)private + 2 * PAGE + 5));

    // A read-only shared mapping is not written; a writable one writes the file.
    char *shared = mmap(NULL, PAGE, PROT_READ, MAP_SHARED, file, 0);
    result("write read-only shared", pwrite(fd, "S", 1, (long)shared));
    char *writable = mmap(NULL, PAGE, PROT_READ | PROT_WRITE, MAP_SHARED, file, 0);
    result("write shared", pwrite(fd, "SHARED", 6, (long)writable + 20));
    pread(file, check, 6, 20);
    printf("file now %.6s\n", check);
    int readonly = open("/tmp/procmem", O_RDONLY);
    char *readonly_shared = mmap(NULL, PAGE, PROT_READ, MAP_SHARED, readonly, 0);
    result("write shared of read-only file", pwrite(fd, "R", 1, (long)readonly_shared));
    result("read shared of read-only file", pread(fd, buffer, 4, (long)readonly_shared + 20));

    // Unmapped memory, and a range that runs into it.
    char *ends = mmap(NULL, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    munmap(ends + PAGE, PAGE);
    result("read unmapped", pread(fd, buffer, 10, (long)ends + PAGE));
    result("read into unmapped", pread(fd, buffer, 2 * PAGE, (long)ends + 100));
    lseek(fd, (long)ends + PAGE - 10, SEEK_SET);
    result("read into unmapped moves", read(fd, buffer, 100));
    result("moved to", position(fd) - (long)ends);
    result("write into unmapped", write(fd, buffer, 100));
    result("moved to", position(fd) - (long)ends);

    // Vectored: segment by segment, stopping short.
    lseek(fd, (long)ends + PAGE - 30, SEEK_SET);
    struct iovec parts[2] = {{buffer, 20}, {buffer + 100, 20}};
    result("readv", readv(fd, parts, 2));
    result("moved to", position(fd) - (long)ends);
    // A bad buffer on the caller's side.
    result("read to a bad buffer", pread(fd, (void *)8, 10, (long)value));
    result("write from a bad buffer", pwrite(fd, (void *)8, 10, (long)value));
    lseek(fd, (long)value, SEEK_SET);
    result("read to a bad buffer moves", read(fd, (void *)8, 10));
    result("moved", position(fd) - (long)value);

    // The program's own code.
    int (*volatile call)(void) = answer;
    printf("answer %d\n", call());
    unsigned int instruction;
    pread(fd, &instruction, 4, (long)answer);
    unsigned int two = (instruction & ~(0xffffu << 5)) | (2u << 5);
    result("patch code", pwrite(fd, &two, 4, (long)answer));
    printf("answer now %d\n", call());

    result("mmap", mmap(NULL, PAGE, PROT_READ, MAP_PRIVATE, fd, 0) == MAP_FAILED ? -1 : 0);
    int ends_of_pipe[2];
    pipe(ends_of_pipe);
    loff_t from = (long)value;
    result("splice to a pipe", splice(fd, &from, ends_of_pipe[1], NULL, 9, 0));

    // Another process: a child reads and writes its parent's memory, then as another user may not
    // open it.
    pid_t parent = getpid();
    pid_t child = fork();
    if (child == 0) {
        char path[64];
        snprintf(path, sizeof path, "/proc/%d/mem", parent);
        int other = open(path, O_RDWR);
        result("child open", other < 0 ? -1 : 0);
        char seen[16] = "";
        result("child read", pread(other, seen, 9, (long)value));
        printf("child sees %.9s\n", seen);
        result("child write", pwrite(other, "kid", 3, (long)value));
        setgid(1000);
        setuid(1000);
        result("open as another user", open(path, O_RDONLY));
        result("read held after setuid", pread(other, seen, 3, (long)value));
        _exit(0);
    }
    waitpid(child, NULL, 0);
    printf("value after child %s\n", value);

    // A child that ended: its memory cannot be opened, and one opened before reads nothing.
    int pipes[2];
    pipe(pipes);
    child = fork();
    if (child == 0) {
        char byte;
        read(pipes[0], &byte, 1);
        _exit(0);
    }
    char path[64];
    snprintf(path, sizeof path, "/proc/%d/mem", child);
    int held = open(path, O_RDONLY);
    result("held read", pread(held, buffer, 9, (long)value));
    write(pipes[1], "x", 1);
    usleep(200000);
    result("open of an ended child", open(path, O_RDONLY));
    result("held read after it ended", pread(held, buffer, 9, (long)value));
    waitpid(child, NULL, 0);
    result("held read after it was reaped", pread(held, buffer, 9, (long)value));

    // A child that ran another program: what was opened before reads nothing.
    child = fork();
    if (child == 0) {
        char byte;
        read(pipes[0], &byte, 1);
        execl("/bin/sleep", "sleep", "1", NULL);
        _exit(1);
    }
    snprintf(path, sizeof path, "/proc/%d/mem", child);
    held = open(path, O_RDONLY);
    write(pipes[1], "x", 1);
    usleep(300000);
    result("held read after exec", pread(held, buffer, 9, (long)value));
    int fresh = open(path, O_RDONLY);
    result("fresh open after exec", fresh < 0 ? -1 : 0);
    waitpid(child, NULL, 0);
    unlink("/tmp/procmem");
    return 0;
}
