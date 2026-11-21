// One kind of file call on the backend, 100 times, for counting the messages each sends. The
// setup (a directory d holding a 4 KiB file f and ten more names) is done for every kind, and the
// kind "none" does only the setup.
#include <dirent.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

int main(int argc, char **argv) {
    const char *kind = argc > 1 ? argv[1] : "none";
    char block[4096];
    memset(block, 'b', sizeof block);
    mkdir("d", 0755);
    int fd = open("d/f", O_CREAT | O_RDWR, 0644);
    write(fd, block, sizeof block);
    for (int i = 0; i < 10; i++) {
        char name[16];
        snprintf(name, sizeof name, "d/n%d", i);
        close(open(name, O_CREAT | O_WRONLY, 0644));
    }
    struct stat st;
    for (int i = 0; i < 100; i++) {
        if (!strcmp(kind, "stat")) {
            stat("d/f", &st);
        } else if (!strcmp(kind, "fstat")) {
            fstat(fd, &st);
        } else if (!strcmp(kind, "open-close")) {
            close(open("d/f", O_RDONLY));
        } else if (!strcmp(kind, "read")) {
            pread(fd, block, sizeof block, 0);
        } else if (!strcmp(kind, "write")) {
            pwrite(fd, block, sizeof block, 0);
        } else if (!strcmp(kind, "list")) {
            DIR *dir = opendir("d");
            while (readdir(dir)) {
            }
            closedir(dir);
        } else if (!strcmp(kind, "create-unlink")) {
            close(open("d/new", O_CREAT | O_WRONLY, 0644));
            unlink("d/new");
        } else if (!strcmp(kind, "mkdir-rmdir")) {
            mkdir("d/sub", 0755);
            rmdir("d/sub");
        } else if (!strcmp(kind, "rename")) {
            rename(i % 2 ? "d/g" : "d/f", i % 2 ? "d/f" : "d/g");
        }
    }
    printf("%s done\n", kind);
    return 0;
}
