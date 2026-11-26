// What /proc/self/fd shows for a file whose opened name was taken away while other links stayed:
// unlinked, linked again under the same name, replaced by a rename, or the last name; on tmpfs
// and on the root's file system, for ordinary and O_PATH descriptors.
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static const char *directory;

static void show(const char *what, int fd) {
    char link[64], target[512];
    snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
    ssize_t n = readlink(link, target, sizeof target - 1);
    target[n < 0 ? 0 : n] = 0;
    // The directory is the same on both sides; only what follows it is shown.
    size_t length = strlen(directory);
    const char *shown = strncmp(target, directory, length) == 0 ? target + length : target;
    printf("%s: %s: %s\n", directory, what, shown);
}

static void path(char *out, const char *name) {
    sprintf(out, "%s/%s", directory, name);
}

static void run(const char *place) {
    char a[256], b[256], c[256], e[256], f[256], g[256], h[256];
    directory = place;
    mkdir(place, 0755);
    path(a, "a"), path(b, "b"), path(c, "c"), path(e, "e"), path(f, "f"), path(g, "g"),
        path(h, "h");
    close(open(a, O_CREAT | O_WRONLY, 0644));
    link(a, b);
    int opened_b = open(b, O_RDONLY);
    unlink(b);
    show("opened name unlinked, another link left", opened_b);

    link(a, c);
    int path_a = open(a, O_PATH);
    unlink(a);
    show("O_PATH, opened name unlinked, another link left", path_a);

    int opened_c = open(c, O_RDONLY);
    link(c, e);
    unlink(c);
    link(e, c);
    show("opened name unlinked and linked again", opened_c);
    int reopened_c = open(c, O_RDONLY);
    show("opened again after the new link", reopened_c);

    close(open(f, O_CREAT | O_WRONLY, 0644));
    rename(f, c);
    show("opened name replaced by a rename", reopened_c);

    int opened_e = open(e, O_RDONLY);
    unlink(e);
    show("last name unlinked", opened_e);

    close(open(g, O_CREAT | O_WRONLY, 0644));
    link(g, h);
    int opened_g = open(g, O_RDONLY);
    unlink(h);
    show("another name unlinked", opened_g);

    unlink(c), unlink(g);
    rmdir(place);
}

int main(void) {
    run("/dev/shm/namesgone");
    run("/tmp/namesgone");
    return 0;
}
