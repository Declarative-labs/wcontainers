// A mapped backend file changed from outside the container: once its attributes show the change,
// the pages its mappings and reads share are read again (FUSE_AUTO_INVAL_DATA), but for a private
// mapping's own copies.
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

static struct timespec modified(int fd) {
    struct stat st;
    fstat(fd, &st);
    return st.st_mtim;
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    int fd = open("changed", O_CREAT | O_RDWR | O_TRUNC, 0644);
    char page[2 * 4096];
    memset(page, 'a', 4096);
    memset(page + 4096, 'b', 4096);
    write(fd, page, sizeof page);
    char *shared = mmap(0, sizeof page, PROT_READ, MAP_SHARED, fd, 0);
    char *private = mmap(0, sizeof page, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    if (shared == MAP_FAILED || private == MAP_FAILED) {
        printf("mmap: %s\n", strerror(errno));
        return 1;
    }
    private[4096] = 'p';
    char byte = 0;
    pread(fd, &byte, 1, 0);
    printf("before: shared %c %c, private %c %c, read %c\n", shared[0], shared[4096], private[0],
           private[4096], byte);
    struct timespec before = modified(fd);
    printf("change it\n");
    for (int tries = 0;; tries++) {
        struct timespec now = modified(fd);
        if (now.tv_sec != before.tv_sec || now.tv_nsec != before.tv_nsec) break;
        if (tries == 3000) {
            printf("never changed\n");
            return 1;
        }
        usleep(10000);
    }
    pread(fd, &byte, 1, 4096);
    printf("after: shared %c %c, private %c %c, read %c\n", shared[0], shared[4096], private[0],
           private[4096], byte);
    printf("unmap: %d %d\n", munmap(shared, sizeof page), munmap(private, sizeof page));
    return 0;
}
