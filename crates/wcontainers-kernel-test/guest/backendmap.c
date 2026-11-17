// Mappings of files on the backend: a private one sees the file's bytes and keeps its own
// writes; a shared one writes through to the file and shows what others write, where the backend
// lets files be mapped shared.
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    int fd = open("mapped", O_CREAT | O_RDWR | O_TRUNC, 0644);
    char page[3 * 4096];
    for (int i = 0; i < (int)sizeof page; i++) page[i] = 'a' + i / 4096;
    write(fd, page, 2 * 4096 + 100);
    char *private = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    printf("private mapping: %d\n", private != MAP_FAILED);
    if (private == MAP_FAILED) return 1;
    printf("private reads the file: %c %c\n", private[0], private[4096]);
    private[0] = 'p';
    char back[1];
    pread(fd, back, 1, 0);
    printf("private write stays private: %c\n", back[0]);
    if (fork() == 0) {
        printf("child sees the parent's copy: %c\n", private[0]);
        private[1] = 'q';
        _exit(0);
    }
    wait(0);
    printf("child's private write not seen: %c\n", private[1]);
    char *tail = mmap(0, 8192, PROT_READ, MAP_PRIVATE, fd, 8192);
    printf("mapping at an offset: %d\n", tail != MAP_FAILED);
    if (tail != MAP_FAILED) {
        printf("offset mapping reads: %c, past the end: %d\n", tail[0], tail[200]);
        munmap(tail, 8192);
    }
    printf("mprotect read only: %d\n", mprotect(private, 8192, PROT_READ));
    printf("unmap private: %d\n", munmap(private, 8192));
    int reader = open("mapped", O_RDONLY);
    char *readonly = mmap(0, 4096, PROT_READ, MAP_SHARED, reader, 0);
    printf("shared read-only mapping: %s\n", readonly == MAP_FAILED ? strerror(errno) : "made");
    char *shared = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    printf("shared mapping: %s\n", shared == MAP_FAILED ? strerror(errno) : "made");
    if (shared == MAP_FAILED) return 0;
    shared[1] = 's';
    msync(shared, 8192, MS_SYNC);
    pread(fd, back, 1, 1);
    printf("shared write reaches the file: %c\n", back[0]);
    if (fork() == 0) {
        shared[2] = 'c';
        _exit(0);
    }
    wait(0);
    printf("child's write seen: %c\n", shared[2]);
    munmap(shared, 8192);
    pread(fd, back, 1, 2);
    printf("file after unmap: %c\n", back[0]);
    return 0;
}
