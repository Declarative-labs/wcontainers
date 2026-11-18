// /proc/self/mountinfo and /proc/self/mounts: each line checked against the mounted file system
// itself, and the mounts every default container has, shown without their numbers.

#define _GNU_SOURCE
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/sysmacros.h>
#include <unistd.h>

static const char *shown[] = {"/", "/proc", "/dev", "/dev/pts", "/sys", "/dev/mqueue", "/dev/shm", "/proc/sys"};

struct mount {
    int id, parent;
    unsigned major, minor;
    char root[256], point[256], options[256], kind[64], source[256], super[512];
};

static int has_option(const char *options, const char *name) {
    size_t length = strlen(name);
    for (const char *at = options; (at = strstr(at, name)) != NULL; at += length) {
        if ((at == options || at[-1] == ',') && (at[length] == ',' || at[length] == 0)) {
            return 1;
        }
    }
    return 0;
}

int main(void) {
    static struct mount mounts[128];
    int count = 0;
    FILE *info = fopen("/proc/self/mountinfo", "r");
    char line[2048];
    while (fgets(line, sizeof line, info) && count < 128) {
        struct mount *m = &mounts[count];
        char *dash = strstr(line, " - ");
        if (!dash || sscanf(line, "%d %d %u:%u %255s %255s %255s", &m->id, &m->parent, &m->major, &m->minor,
                            m->root, m->point, m->options) != 7 ||
            sscanf(dash + 3, "%63s %255s %511s", m->kind, m->source, m->super) != 3) {
            printf("unparsed: %s", line);
            continue;
        }
        count++;
    }
    fclose(info);
    printf("mountinfo has lines %d\n", count > 5);

    // Every line agrees with the file system mounted at its point, numbers are unique, and each
    // parent is listed but the first's.
    int ids_unique = 1, parents_listed = 1, devices_match = 1, flags_match = 1;
    for (int i = 0; i < count; i++) {
        struct mount *m = &mounts[i];
        int parent_found = i == 0;
        for (int j = 0; j < count; j++) {
            if (j != i && mounts[j].id == m->id) {
                ids_unique = 0;
            }
            if (mounts[j].id == m->parent) {
                parent_found = 1;
            }
        }
        parents_listed &= parent_found;
        struct stat st;
        struct statvfs vfs;
        // The standard mounts' devices (the reference's bind mount of the test's directory shows
        // another device to stat than the one it lists).
        int standard = 0;
        for (size_t s = 0; s < sizeof shown / sizeof shown[0]; s++) {
            standard |= strcmp(m->point, shown[s]) == 0;
        }
        if (standard && stat(m->point, &st) == 0 &&
            (major(st.st_dev) != m->major || minor(st.st_dev) != m->minor)) {
            printf("device differs at %s\n", m->point);
            devices_match = 0;
        }
        if (statvfs(m->point, &vfs) == 0) {
            int same = !!(vfs.f_flag & ST_RDONLY) == has_option(m->options, "ro") &&
                       !!(vfs.f_flag & ST_NOSUID) == has_option(m->options, "nosuid") &&
                       !!(vfs.f_flag & ST_NODEV) == has_option(m->options, "nodev") &&
                       !!(vfs.f_flag & ST_NOEXEC) == has_option(m->options, "noexec") &&
                       !!(vfs.f_flag & ST_NOATIME) == has_option(m->options, "noatime") &&
                       !!(vfs.f_flag & ST_RELATIME) == has_option(m->options, "relatime");
            if (!same) {
                printf("flags differ at %s: %s %lx\n", m->point, m->options, vfs.f_flag);
                flags_match = 0;
            }
        }
    }
    printf("ids unique %d parents listed %d devices match %d flags match %d\n", ids_unique, parents_listed,
           devices_match, flags_match);
    printf("first is the root %d\n", count > 0 && strcmp(mounts[0].point, "/") == 0);

    // The mounts every default container has, without numbers; the root's type is the image's.
    for (size_t s = 0; s < sizeof shown / sizeof shown[0]; s++) {
        for (int i = 0; i < count; i++) {
            struct mount *m = &mounts[i];
            if (strcmp(m->point, shown[s]) != 0) {
                continue;
            }
            int parent = -1;
            for (int j = 0; j < count; j++) {
                if (mounts[j].id == m->parent) {
                    parent = j;
                }
            }
            if (s == 0) {
                printf("%s %s %s\n", m->root, m->point, m->options);
            } else {
                printf("%s %s %s on %s - %s %s %s\n", m->root, m->point, m->options,
                       parent >= 0 ? mounts[parent].point : "?", m->kind, m->source, m->super);
            }
        }
    }

    // /proc/self/mounts lists the same mounts in the same order, with the same options.
    FILE *table = fopen("/proc/self/mounts", "r");
    int index = 0, same = 1;
    while (fgets(line, sizeof line, table)) {
        char source[256], point[256], kind[64], options[768];
        if (sscanf(line, "%255s %255s %63s %767s", source, point, kind, options) != 4 || index >= count) {
            same = 0;
            break;
        }
        struct mount *m = &mounts[index++];
        char expected[1024];
        // The mount's options, then the file system's after its leading rw or ro.
        const char *rest = strchr(m->super, ',');
        snprintf(expected, sizeof expected, "%s%s", m->options, rest ? rest : "");
        if (strcmp(source, m->source) || strcmp(point, m->point) || strcmp(kind, m->kind) ||
            strcmp(options, expected)) {
            printf("mounts differs: %s", line);
            same = 0;
        }
    }
    fclose(table);
    printf("mounts agrees %d\n", same && index == count);

    // Both poll as readable and not changed.
    int fd = open("/proc/self/mountinfo", O_RDONLY);
    struct pollfd p = {fd, POLLIN | POLLPRI | POLLERR};
    printf("poll %d revents %x\n", poll(&p, 1, 0), p.revents);
    return 0;
}
