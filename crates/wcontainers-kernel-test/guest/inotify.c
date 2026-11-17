// inotify: watches on a directory and on a file, the events of making, opening, reading,
// writing, closing, changing, renaming and removing, one-shot watches, merged events, reading
// with too small a buffer, and the checks of each call.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/inotify.h>
#include <sys/ioctl.h>
#include <linux/fs.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static unsigned last_cookie;

// Reads and prints every event waiting.
static void events(int fd, const char *after) {
    char buf[4096] __attribute__((aligned(8)));
    printf("after %s:\n", after);
    long n = read(fd, buf, sizeof buf);
    if (n < 0) {
        printf(" none (%s)\n", strerror(errno));
        return;
    }
    for (char *p = buf; p < buf + n;) {
        struct inotify_event *e = (void *)p;
        printf(" wd %d mask %#x", e->wd, e->mask);
        if (e->cookie) {
            printf(" cookie %s", e->cookie == last_cookie ? "same" : "new");
            last_cookie = e->cookie;
        }
        if (e->len)
            printf(" name %s (length %u)", e->name, e->len);
        printf("\n");
        p += sizeof *e + e->len;
    }
}

// Events of an open file after its name is removed: watches with IN_EXCL_UNLINK hear none, on
// the directory or on the file; others still do.
static void unlinked_files(void) {
    int fd = inotify_init1(IN_NONBLOCK);
    mkdir("/tmp/x", 0755);
    int f = open("/tmp/x/a", O_CREAT | O_RDWR, 0644);
    int g = open("/tmp/x/b", O_CREAT | O_RDWR, 0644);
    link("/tmp/x/b", "/tmp/x/b2");
    printf("excluding dir %d\n", inotify_add_watch(fd, "/tmp/x", IN_ALL_EVENTS | IN_EXCL_UNLINK));
    printf("excluding file %d\n", inotify_add_watch(fd, "/tmp/x/a", IN_ALL_EVENTS | IN_EXCL_UNLINK));
    int other = inotify_init1(IN_NONBLOCK);
    printf("plain dir %d\n", inotify_add_watch(other, "/tmp/x", IN_ALL_EVENTS));
    unlink("/tmp/x/a");
    unlink("/tmp/x/b");
    events(fd, "unlinking, excluded");
    events(other, "unlinking, plain");
    write(f, "x", 1);
    write(g, "y", 1);
    close(f);
    close(g);
    events(fd, "writes after unlink, excluded");
    events(other, "writes after unlink, plain");
    // A file moved away while open is not unlinked.
    f = open("/tmp/x/c", O_CREAT | O_RDWR, 0644);
    rename("/tmp/x/c", "/tmp/x/d");
    events(fd, "moving c");
    events(other, "moving c, plain");
    write(f, "z", 1);
    close(f);
    events(fd, "writing moved c");
    events(other, "writing moved c, plain");
    unlink("/tmp/x/d");
    unlink("/tmp/x/b2");
    rmdir("/tmp/x");
    events(fd, "removing all");
    close(fd);
    close(other);
}

// An exchange of two names moves each, each with its own cookie.
static void exchanges(void) {
    int fd = inotify_init1(IN_NONBLOCK);
    mkdir("/tmp/y", 0755);
    mkdir("/tmp/z", 0755);
    close(open("/tmp/y/one", O_CREAT | O_WRONLY, 0644));
    mkdir("/tmp/z/two", 0755);
    inotify_add_watch(fd, "/tmp/y", IN_ALL_EVENTS);
    inotify_add_watch(fd, "/tmp/z", IN_ALL_EVENTS);
    inotify_add_watch(fd, "/tmp/y/one", IN_ALL_EVENTS);
    inotify_add_watch(fd, "/tmp/z/two", IN_ALL_EVENTS);
    result("exchange", syscall(SYS_renameat2, AT_FDCWD, "/tmp/y/one", AT_FDCWD, "/tmp/z/two", RENAME_EXCHANGE));
    events(fd, "exchange");
    // A rename over a file with no other name ends it.
    close(open("/tmp/y/three", O_CREAT | O_WRONLY, 0644));
    inotify_add_watch(fd, "/tmp/y/three", IN_ALL_EVENTS);
    result("replace", rename("/tmp/z/two", "/tmp/y/three"));
    events(fd, "replacing three");
    close(fd);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    result("bad flags", inotify_init1(1));
    int fd = inotify_init1(IN_NONBLOCK | IN_CLOEXEC);
    result("init", fd > 0);
    mkdir("/tmp/w", 0755);
    int wd = inotify_add_watch(fd, "/tmp/w", IN_ALL_EVENTS);
    result("watch", wd);
    result("again with create", inotify_add_watch(fd, "/tmp/w", IN_MODIFY | IN_MASK_CREATE));
    result("add and create", inotify_add_watch(fd, "/tmp/w", IN_MODIFY | IN_MASK_ADD | IN_MASK_CREATE));
    int flags_only = inotify_add_watch(fd, "/tmp", IN_ONESHOT);
    result("flags only", flags_only);
    result("remove it", inotify_rm_watch(fd, flags_only));
    result("no bits", inotify_add_watch(fd, "/tmp", 0x08000000));
    result("missing", inotify_add_watch(fd, "/tmp/nothing", IN_MODIFY));
    result("not an instance", inotify_add_watch(0, "/tmp/w", IN_MODIFY));
    result("bad descriptor", inotify_add_watch(99, "/tmp/w", IN_MODIFY));
    events(fd, "nothing");
    int f = open("/tmp/w/a", O_CREAT | O_WRONLY, 0644);
    write(f, "x", 1);
    write(f, "y", 1);
    close(f);
    events(fd, "making and writing");
    f = open("/tmp/w/a", O_RDONLY);
    char c;
    read(f, &c, 1);
    read(f, &c, 1);
    close(f);
    events(fd, "reading");
    chmod("/tmp/w/a", 0600);
    truncate("/tmp/w/a", 0);
    events(fd, "chmod and truncate");
    rename("/tmp/w/a", "/tmp/w/b");
    events(fd, "rename");
    unlink("/tmp/w/b");
    events(fd, "unlink");
    mkdir("/tmp/w/d", 0755);
    rmdir("/tmp/w/d");
    symlink("target", "/tmp/w/l");
    link("/tmp/w/l", "/tmp/w/l2");
    events(fd, "mkdir, rmdir, symlink and link");
    // A watch on a file of its own.
    f = open("/tmp/w/f", O_CREAT | O_RDWR, 0644);
    events(fd, "making f");
    int wf = inotify_add_watch(fd, "/tmp/w/f", IN_MODIFY | IN_ATTRIB | IN_DELETE_SELF);
    result("watch f", wf);
    result("only a directory", inotify_add_watch(fd, "/tmp/w/f", IN_MODIFY | IN_ONLYDIR));
    write(f, "z", 1);
    events(fd, "writing f");
    unlink("/tmp/w/f");
    events(fd, "unlinking f");
    close(f);
    events(fd, "closing f");
    // A one-shot watch.
    int ws = inotify_add_watch(fd, "/tmp/w/l2", IN_ATTRIB | IN_ONESHOT | IN_DONT_FOLLOW);
    result("one shot", ws);
    fchmodat(AT_FDCWD, "/tmp/w", 0700, 0);
    unlink("/tmp/w/l");
    unlink("/tmp/w/l2");
    events(fd, "one shot");
    // Waiting bytes, a buffer too small, readiness.
    f = open("/tmp/w/g", O_CREAT | O_WRONLY, 0644);
    int waiting = -1;
    result("fionread", ioctl(fd, FIONREAD, &waiting));
    printf("waiting %d\n", waiting);
    struct pollfd p = {.fd = fd, .events = POLLIN};
    result("poll", poll(&p, 1, 0));
    char small[20];
    result("small read", read(fd, small, sizeof small));
    events(fd, "making g");
    close(f);
    result("remove watch", inotify_rm_watch(fd, wd));
    result("remove again", inotify_rm_watch(fd, wd));
    events(fd, "removing the watch");
    result("write to instance", write(fd, "x", 1));
    unlink("/tmp/w/g");
    rmdir("/tmp/w");
    close(fd);
    unlinked_files();
    exchanges();
    return 0;
}
