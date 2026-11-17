// Mappings of files on the backend: a private one sees the file's bytes and keeps its own
// writes; a shared one writes through to the file and shows what others write.
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    int fd = open("mapped", O_CREAT | O_RDWR | O_TRUNC, 0644);
    char page[8192];
    memset(page, 'x', sizeof page);
    write(fd, page, sizeof page);
    char *private = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    printf("private mapping: %d\n", private != MAP_FAILED);
    if (private == MAP_FAILED) return 1;
    printf("private reads the file: %c\n", private[4096]);
    private[0] = 'p';
    char back[1];
    pread(fd, back, 1, 0);
    printf("private write stays private: %c\n", back[0]);
    char *shared = mmap(0, 8192, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    printf("shared mapping: %d\n", shared != MAP_FAILED);
    if (shared == MAP_FAILED) return 1;
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
