// Directory listings' `.` and `..`: whether they come, their inode numbers, and a position taken
// after them and gone back to, in directories of the image and new ones.
#define _GNU_SOURCE
#include <dirent.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>

// Counts and the inode of `..` are shown only for directories this program made (`counted`):
// the image's differ in what Docker adds, and overlayfs gives a merged directory's `..` the inode
// of the layer below.
static void list(const char *path, int counted) {
    char parent[256];
    snprintf(parent, sizeof parent, "%s/..", path);
    struct stat own, above;
    stat(path, &own);
    stat(parent, &above);
    DIR *directory = opendir(path);
    if (!directory) {
        printf("%s: not opened\n", path);
        return;
    }
    int dot = 0, dot_ino = 0, dotdot = 0, dotdot_ino = 0, entries = 0;
    struct dirent *entry;
    while ((entry = readdir(directory))) {
        entries++;
        if (strcmp(entry->d_name, ".") == 0) {
            dot++;
            dot_ino = entry->d_ino == own.st_ino;
        } else if (strcmp(entry->d_name, "..") == 0) {
            dotdot++;
            dotdot_ino = entry->d_ino == above.st_ino;
        }
    }
    // Two entries in, the place is kept and gone back to.
    rewinddir(directory);
    readdir(directory);
    readdir(directory);
    long place = telldir(directory);
    entry = readdir(directory);
    char next[256] = "(end)";
    if (entry) snprintf(next, sizeof next, "%s", entry->d_name);
    int rest = 0;
    while (readdir(directory)) rest++;
    seekdir(directory, place);
    entry = readdir(directory);
    int same = entry ? strcmp(entry->d_name, next) == 0 : strcmp(next, "(end)") == 0;
    closedir(directory);
    printf("%s: . %d (inode %d) .. %d, back again %d", path, dot, dot_ino, dotdot, same);
    if (counted)
        printf(", .. inode %d, %d entries, %d after the third", dotdot_ino, entries, rest);
    printf("\n");
}

int main(void) {
    list("/", 0);
    list("/etc", 0);
    list("/tmp", 0);
    mkdir("/tmp/dots", 0755);
    list("/tmp/dots", 1);
    mkdir("/tmp/dots/deeper", 0755);
    list("/tmp/dots/deeper", 1);
    FILE *file = fopen("/tmp/dots/one", "w");
    if (file) fclose(file);
    list("/tmp/dots", 1);
    list("/dev/shm", 0);
    return 0;
}
