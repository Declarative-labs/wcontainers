// Directories on the root: a failed rmdir of a directory that is still used, then work inside it,
// by several processes at once, as ftest02 does.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static void p(const char *what, int r) { printf("%s: %d %s\n", what, r, r < 0 ? strerror(errno) : ""); }

static void round(const char *dir, int quiet) {
    int bad = 0;
    for (int i = 0; i < 20; i++) {
        mkdir(dir, 0755);
        if (chdir(dir)) bad++;
        close(open("f1", O_CREAT | O_WRONLY, 0644));
        close(open("f2", O_CREAT | O_WRONLY, 0644));
        if (chdir("..")) bad++;
        if (rmdir(dir) == 0 || errno != ENOTEMPTY) bad++;
        if (chdir(dir)) bad++;
        if (unlink("f1")) { if (!quiet) p("unlink f1", -1); bad++; }
        if (unlink("f2")) { if (!quiet) p("unlink f2", -1); bad++; }
        if (chdir("..")) bad++;
        if (i & 1) {
            if (rmdir(dir)) bad++;
        }
    }
    if (!quiet) printf("%s: %d failures\n", dir, bad);
    if (bad) _exit(bad);
}

// Files made and removed by whole paths, each walk looking the shared directories up again.
static int whole_paths(int k) {
    int bad = 0;
    char name[64];
    for (int i = 0; i < 40; i++) {
        snprintf(name, sizeof name, "/tmp/rootdirs/shared/c%d-%d", k, i % 7);
        int fd = open(name, O_RDWR | O_CREAT | O_TRUNC, 0666);
        if (fd < 0) { bad++; printf("walker %d open %s: %s\n", k, name, strerror(errno)); }
        if (write(fd, "x", 1) != 1) { bad++; printf("walker %d write: %s\n", k, strerror(errno)); }
        close(fd);
        snprintf(name, sizeof name, "/tmp/rootdirs/shared/c%d-%d", k, (i + 3) % 7);
        if (rmdir(name) == 0) { bad++; printf("walker %d rmdir of a file worked\n", k); }
        else if (errno != ENOTDIR && errno != ENOENT) { bad++; printf("walker %d rmdir %s: %s\n", k, name, strerror(errno)); }
        if (unlink(name) && errno != ENOENT) { bad++; printf("walker %d unlink %s: %s\n", k, name, strerror(errno)); }
    }
    return bad;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    mkdir("/tmp/rootdirs", 0755);
    if (chdir("/tmp/rootdirs")) return 1;
    round("one", 0);
    // rmdir of a file, then its unlink, by absolute name too.
    close(open("file", O_CREAT | O_WRONLY, 0644));
    p("rmdir a file", rmdir("file"));
    p("unlink it", unlink("file"));
    close(open("/tmp/rootdirs/file2", O_CREAT | O_WRONLY, 0644));
    p("rmdir a file by its whole path", rmdir("/tmp/rootdirs/file2"));
    p("unlink it by its whole path", unlink("/tmp/rootdirs/file2"));
    mkdir("/tmp/rootdirs/shared", 0755);
    pid_t walkers[5];
    for (int k = 0; k < 5; k++) {
        if ((walkers[k] = fork()) == 0) _exit(whole_paths(k));
    }
    for (int k = 0; k < 5; k++) {
        int status;
        waitpid(walkers[k], &status, 0);
        printf("walker %d: %d\n", k, status);
    }
    pid_t children[3];
    for (int k = 0; k < 3; k++) {
        if ((children[k] = fork()) == 0) {
            char name[8];
            snprintf(name, sizeof name, "d%d", k);
            round(name, 1);
            _exit(0);
        }
    }
    for (int k = 0; k < 3; k++) {
        int status;
        waitpid(children[k], &status, 0);
        printf("child %d: %d\n", k, status);
    }
    return 0;
}
