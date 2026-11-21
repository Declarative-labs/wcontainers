// fs.inotify.max_user_instances: a user's instances past the limit are refused with EMFILE, and
// one goes again once another closes. How many are made is not shown: the reference's root shares
// its count with the host's other root processes.
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/inotify.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    static int fds[9000];
    int made = 0;
    while (made < 9000) {
        int fd = inotify_init1(IN_CLOEXEC);
        if (fd < 0) {
            printf("refused within the limit %d: %s\n", made > 4000 && made <= 8192, strerror(errno));
            break;
        }
        fds[made++] = fd;
    }
    close(fds[0]);
    int again = inotify_init1(0);
    printf("one more after a close: %d\n", again >= 0);
    return 0;
}
