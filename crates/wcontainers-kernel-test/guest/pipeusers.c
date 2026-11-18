// Per-user pipe pages: past pipe-user-pages-soft new pipes are small and none may grow, shrinking
// gives pages back, and each user has a count of its own. Runs as users no other process on the
// machine has, since the reference counts a user's pipes across the whole machine.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

#define PIPES 1100

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

static void show(const char *path) {
    char text[64] = "";
    FILE *file = fopen(path, "r");
    if (file == NULL || fgets(text, sizeof text, file) == NULL) {
        printf("%s: none\n", path);
    } else {
        printf("%s: %s", path, text);
    }
    if (file) {
        fclose(file);
    }
}

static int fds[PIPES][2];

static void run(uid_t uid) {
    setgid(uid);
    setuid(uid);
    int made = 0;
    for (; made < PIPES; made++) {
        if (pipe(fds[made]) < 0) {
            result("pipe", -1);
            break;
        }
    }
    for (int i = 1020; i < 1030 && i < made; i++) {
        printf("pipe %d size %d\n", i, fcntl(fds[i][1], F_GETPIPE_SZ));
    }
    result("grow a small one", fcntl(fds[1029][1], F_SETPIPE_SZ, 65536));
    result("grow a big one", fcntl(fds[10][1], F_SETPIPE_SZ, 131072));
    result("same size", fcntl(fds[10][1], F_SETPIPE_SZ, 65536));
    result("shrink a small one", fcntl(fds[1029][1], F_SETPIPE_SZ, 4096));
    // Shrinking 20 pipes to one page gives back 300 pages.
    for (int i = 0; i < 20; i++) {
        fcntl(fds[i][1], F_SETPIPE_SZ, 4096);
    }
    result("grow a small one now", fcntl(fds[1029][1], F_SETPIPE_SZ, 65536));
    int extra[2];
    pipe(extra);
    printf("new pipe size %d\n", fcntl(extra[1], F_GETPIPE_SZ));
    // A full small pipe cannot shrink below what it holds.
    char bytes[8192] = {0};
    write(fds[1028][1], bytes, sizeof bytes);
    result("shrink a full one", fcntl(fds[1028][1], F_SETPIPE_SZ, 4096));
    // Closing pipes gives their pages back.
    for (int i = 100; i < 400; i++) {
        close(fds[i][0]);
        close(fds[i][1]);
    }
    pipe(extra);
    printf("after closing size %d\n", fcntl(extra[1], F_GETPIPE_SZ));
    // A child shares its parent's pipes, and a new user starts afresh.
    pid_t child = fork();
    if (child == 0) {
        pipe(extra);
        printf("child's pipe size %d\n", fcntl(extra[1], F_GETPIPE_SZ));
        setuid(uid + 1);
        pipe(extra);
        printf("other user's pipe size %d\n", fcntl(extra[1], F_GETPIPE_SZ));
        _exit(0);
    }
    waitpid(child, NULL, 0);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    show("/proc/sys/fs/pipe-user-pages-soft");
    show("/proc/sys/fs/pipe-user-pages-hard");
    show("/proc/sys/kernel/sched_autogroup_enabled");
    result("write soft", open("/proc/sys/fs/pipe-user-pages-soft", O_WRONLY));
    pid_t child = fork();
    if (child == 0) {
        run(54321);
        _exit(0);
    }
    waitpid(child, NULL, 0);
    return 0;
}
