// File calls in the working directory, which is on the file backend: making, reading, writing,
// listing, renaming, linking and removing files, their attributes and the file system's. What it
// prints leaves out what differs between two backends of the same kind (inode numbers, times).
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <unistd.h>

#define RENAME_NOREPLACE 1
#define RENAME_EXCHANGE 2

static int renameat2(int from, const char *old, int to, const char *new, unsigned flags) {
    return syscall(SYS_renameat2, from, old, to, new, flags);
}

static void result(const char *what, long value) {
    if (value < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
    } else {
        printf("%s: %ld\n", what, value);
    }
}

static void show(const char *path) {
    struct stat st;
    if (lstat(path, &st) != 0) {
        printf("stat %s: %s\n", path, strerror(errno));
        return;
    }
    printf("stat %s: mode %o links %ld uid %d gid %d size %ld blocks>0 %d\n", path, st.st_mode,
           (long)st.st_nlink, st.st_uid, st.st_gid, (long)st.st_size, st.st_blocks > 0);
}

static int by_name(const struct dirent **a, const struct dirent **b) {
    return strcmp((*a)->d_name, (*b)->d_name);
}

static void list(const char *path) {
    struct dirent **names;
    int count = scandir(path, &names, 0, by_name);
    if (count < 0) {
        printf("list %s: %s\n", path, strerror(errno));
        return;
    }
    printf("list %s:", path);
    for (int i = 0; i < count; i++) {
        printf(" %s(%d)", names[i]->d_name, names[i]->d_type);
        free(names[i]);
    }
    printf("\n");
    free(names);
}

/// Copies this program onto the backend and runs it from there, as a program and through a script.
static void run_from_backend(void) {
    int in = open("/proc/self/exe", O_RDONLY);
    int out = open("prog", O_CREAT | O_WRONLY | O_TRUNC, 0755);
    char block[65536];
    ssize_t n;
    while ((n = read(in, block, sizeof block)) > 0) write(out, block, n);
    close(in);
    close(out);
    int script = open("script", O_CREAT | O_WRONLY | O_TRUNC, 0755);
    const char *text = "#!./prog from-script\n";
    write(script, text, strlen(text));
    close(script);
    for (int i = 0; i < 2; i++) {
        pid_t child = fork();
        if (child == 0) {
            char *argv[] = {i ? "./script" : "./prog", "exec", 0};
            execv(argv[0], argv);
            printf("exec: %s\n", strerror(errno));
            _exit(1);
        }
        int status;
        waitpid(child, &status, 0);
        printf("ran from the backend: status %d\n", WEXITSTATUS(status));
    }
}

int main(int argc, char **argv) {
    setvbuf(stdout, 0, _IONBF, 0);
    if (argc > 1) {
        printf("running from the backend:");
        for (int i = 1; i < argc; i++) printf(" %s", argv[i]);
        printf("\n");
        return 3;
    }
    umask(022);
    char buffer[8192];

    int fd = open("a", O_CREAT | O_RDWR | O_EXCL, 0666);
    result("create a", fd >= 0 ? 0 : -1);
    result("create a again, exclusive", open("a", O_CREAT | O_RDWR | O_EXCL, 0666));
    result("write", write(fd, "hello, backend\n", 15));
    result("lseek end", lseek(fd, 0, SEEK_END));
    result("lseek set", lseek(fd, 7, SEEK_SET));
    memset(buffer, 0, sizeof buffer);
    result("read", read(fd, buffer, sizeof buffer));
    printf("read text: %s", buffer);
    result("pwrite past the end", pwrite(fd, "tail", 4, 100));
    show("a");
    result("pread a hole", pread(fd, buffer, 10, 50));
    printf("hole is zero: %d\n", buffer[0] == 0 && buffer[9] == 0);
    result("ftruncate", ftruncate(fd, 5));
    result("fsync", fsync(fd));
    result("fdatasync", fdatasync(fd));
    struct stat st;
    result("fstat", fstat(fd, &st));
    printf("fstat size %ld\n", (long)st.st_size);
    result("close", close(fd));
    // A file with no name, which the backend makes if it can.
    int unnamed = open(".", O_TMPFILE | O_RDWR, 0600);
    result("open with O_TMPFILE", unnamed >= 0 ? 0 : -1);
    if (unnamed >= 0) {
        close(unnamed);
    }

    // Many bytes, in more than one message each way.
    fd = open("big", O_CREAT | O_WRONLY | O_TRUNC, 0640);
    static char big[600000];
    for (size_t i = 0; i < sizeof big; i++) big[i] = (char)(i * 7);
    result("write big", write(fd, big, sizeof big));
    close(fd);
    fd = open("big", O_RDONLY);
    static char back[600000];
    result("read big", read(fd, back, sizeof back));
    printf("big the same: %d\n", memcmp(big, back, sizeof big) == 0);
    close(fd);
    show("big");

    fd = open("a", O_WRONLY | O_APPEND);
    result("append", write(fd, "+more", 5));
    close(fd);
    fd = open("a", O_RDONLY);
    memset(buffer, 0, sizeof buffer);
    result("read appended", read(fd, buffer, sizeof buffer));
    printf("text: %s\n", buffer);
    close(fd);
    fd = open("a", O_WRONLY | O_TRUNC);
    show("a");
    close(fd);

    result("mkdir d", mkdir("d", 0777));
    result("mkdir d again", mkdir("d", 0777));
    result("mkdir d/e", mkdir("d/e", 0700));
    result("mknod fifo", mknod("d/fifo", S_IFIFO | 0644, 0));
    result("symlink", symlink("../a", "d/link"));
    result("readlink", readlink("d/link", buffer, sizeof buffer));
    result("link", link("a", "d/hard"));
    show("a");
    show("d");
    show("d/link");
    show("d/fifo");
    result("stat through the link", stat("d/link", &st));
    printf("followed mode %o\n", st.st_mode);
    list(".");
    list("d");
    result("rename a to d/b", rename("a", "d/b"));
    result("rename to a missing directory", rename("d/b", "missing/b"));
    result("renameat2 noreplace", renameat2(AT_FDCWD, "d/b", AT_FDCWD, "d/hard", RENAME_NOREPLACE));
    result("renameat2 exchange", renameat2(AT_FDCWD, "d/b", AT_FDCWD, "big", RENAME_EXCHANGE));
    show("d/b");
    show("big");
    list("d");

    result("chmod", chmod("d/hard", 04755));
    show("d/hard");
    result("chown", chown("d/hard", 1000, 1000));
    show("d/hard");
    struct timespec times[2] = {{1000000, 5}, {2000000, 7}};
    result("utimensat", utimensat(AT_FDCWD, "d/hard", times, 0));
    result("stat times", stat("d/hard", &st));
    printf("times %ld.%ld %ld.%ld\n", (long)st.st_atim.tv_sec, st.st_atim.tv_nsec,
           (long)st.st_mtim.tv_sec, st.st_mtim.tv_nsec);
    result("truncate", truncate("d/hard", 3));
    show("d/hard");

    result("chdir d", chdir("d"));
    char cwd[256];
    printf("cwd ends with /d: %d\n", getcwd(cwd, sizeof cwd) && strlen(cwd) >= 2 && strcmp(cwd + strlen(cwd) - 2, "/d") == 0);
    show("..");
    result("access hard", access("hard", R_OK | W_OK));
    result("chdir back", chdir(".."));

    result("unlink d/hard", unlink("d/hard"));
    result("unlink d/hard again", unlink("d/hard"));
    result("rmdir d, not empty", rmdir("d"));
    result("rmdir a file", rmdir("big"));
    result("unlink a directory", unlink("d/e"));
    result("rmdir d/e", rmdir("d/e"));
    result("open a missing file", open("nothing", O_RDONLY));
    result("open a directory to write", open("d", O_WRONLY));
    result("open a file as a directory", open("big", O_RDONLY | O_DIRECTORY));

    struct statfs fs;
    result("statfs", statfs(".", &fs));
    printf("statfs type %lx namelen %ld\n", (long)fs.f_type, (long)fs.f_namelen);

    // A child writes, the parent reads what it wrote.
    pid_t child = fork();
    if (child == 0) {
        int out = open("from-child", O_CREAT | O_WRONLY, 0600);
        write(out, "child\n", 6);
        close(out);
        _exit(0);
    }
    waitpid(child, 0, 0);
    fd = open("from-child", O_RDONLY);
    memset(buffer, 0, sizeof buffer);
    result("read the child's file", read(fd, buffer, sizeof buffer));
    printf("child wrote: %s", buffer);
    close(fd);
    show("from-child");

    // As another user.
    if (fork() == 0) {
        setgid(1000);
        setuid(1000);
        result("other user creates in a root directory", open("theirs", O_CREAT | O_WRONLY, 0600));
        result("other user reads a 0600 file", open("from-child", O_RDONLY));
        result("other user mkdir in d", mkdir("d/mine", 0755));
        _exit(0);
    }
    wait(0);
    list(".");
    run_from_backend();
    return 0;
}
