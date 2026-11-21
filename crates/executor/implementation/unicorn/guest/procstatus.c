// Prints the lines of /proc/self/status that follow the descriptor table and the groups, and the
// /proc/self/maps lines of shared anonymous memory and shared /dev/zero mappings (inode left out), after
// changing each. With an argument "private", it shows instead whether a private file mapping sees a
// later write to the file; with "unbuilt", it opens entries of /proc Linux has and one it has not;
// with "limits", it lists /proc/sys/kernel/keys and /proc/sys/user with the values that do not
// depend on the machine's memory, tries to write one, and lists /proc/cgroups and the heading of
// /proc/swaps.

#define _GNU_SOURCE
#include <fcntl.h>
#include <dirent.h>
#include <errno.h>
#include <grp.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <unistd.h>

static void status(void) {
    char line[512];
    FILE *file = fopen("/proc/self/status", "r");
    while (file && fgets(line, sizeof line, file)) {
        if (strncmp(line, "FDSize:", 7) == 0 || strncmp(line, "Groups:", 7) == 0) {
            printf("[%.*s]\n", (int)strcspn(line, "\n"), line);
        }
    }
    if (file) {
        fclose(file);
    }
}

// The maps lines of the range, with the inode left out and /dev's device number (which differs between
// machines) named.
static void maps(void *start) {
    struct stat dev;
    char dev_device[16] = "";
    if (stat("/dev/zero", &dev) == 0) {
        snprintf(dev_device, sizeof dev_device, "%02x:%02x", major(dev.st_dev), minor(dev.st_dev));
    }
    char line[512], want[32];
    snprintf(want, sizeof want, "%lx-", (unsigned long)start);
    FILE *file = fopen("/proc/self/maps", "r");
    while (file && fgets(line, sizeof line, file)) {
        if (strncmp(line, want, strlen(want)) == 0) {
            char range[64], perms[8], offset[16], device[16], path[256] = "";
            unsigned long ino;
            sscanf(line, "%63s %7s %15s %15s %lu %255[^\n]", range, perms, offset, device, &ino, path);
            printf("%s %s %s %s %s\n", perms, offset, strcmp(device, dev_device) ? device : "/dev's", ino ? "ino" : "0",
                   path);
        }
    }
    if (file) {
        fclose(file);
    }
}

static int private_mapping(void) {
    int fd = open("/dev/shm/p", O_CREAT | O_RDWR, 0600);
    pwrite(fd, "abcdefgh", 8, 0);
    char *map = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, fd, 0);
    pwrite(fd, "Q", 1, 7);
    printf("private mapping sees a later write: %c\n", map[7]);
    return 0;
}

// Lists a directory of sysctls in getdents order with each file's value, or "set" for those that
// come from the machine's memory (the namespace limits and fanotify's marks).
static void sysctls(const char *directory) {
    DIR *dir = opendir(directory);
    struct dirent *entry;
    while ((entry = readdir(dir))) {
        if (entry->d_name[0] == '.')
            continue;
        char path[256], value[64] = "";
        snprintf(path, sizeof path, "%s/%s", directory, entry->d_name);
        FILE *f = fopen(path, "r");
        if (f && fgets(value, sizeof value, f) &&
            (strstr(entry->d_name, "_namespaces") || strcmp(entry->d_name, "max_fanotify_marks") == 0))
            strcpy(value, "set\n");
        if (f)
            fclose(f);
        printf("%s %s", path, value);
    }
    closedir(dir);
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "limits") == 0) {
        sysctls("/proc/sys/kernel/keys");
        sysctls("/proc/sys/user");
        int fd = open("/proc/sys/kernel/keys/gc_delay", O_WRONLY);
        printf("write gc_delay %d errno %d\n", fd, fd < 0 ? errno : 0);
        // The control groups built, without the machine's count of groups.
        FILE *cgroups = fopen("/proc/cgroups", "r");
        char line[128], name[64];
        int hierarchy, count, enabled;
        fgets(line, sizeof line, cgroups);
        fputs(line, stdout);
        while (fscanf(cgroups, "%63s %d %d %d", name, &hierarchy, &count, &enabled) == 4)
            printf("%s %d %d\n", name, hierarchy, enabled);
        fclose(cgroups);
        // The swap areas' heading (the reference's machine has one area, the made-up one none).
        FILE *swaps = fopen("/proc/swaps", "r");
        fgets(line, sizeof line, swaps);
        fputs(line, stdout);
        fclose(swaps);
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "private") == 0) {
        return private_mapping();
    }
    if (argc > 1 && strcmp(argv[1], "unbuilt") == 0) {
        const char *paths[] = {"/proc/self/mountstats", "/proc/sys/kernel/random/urandom_min_reseed_secs",
                               "/proc/nonexistent"};
        for (int index = 0; index < 3; index++) {
            printf("%s: %d\n", paths[index], open(paths[index], O_RDONLY) >= 0);
        }
        return 0;
    }
    status();
    for (int count = 0; count < 70; count++) {
        open("/dev/null", O_RDONLY);
    }
    status();
    dup2(0, 300);
    status();
    close(300);
    status();
    gid_t groups[] = {5, 1, 3000};
    setgroups(3, groups);
    status();
    setgroups(1, groups);
    status();
    setgroups(0, NULL);
    status();
    char *shared = mmap(NULL, 3 * 4096, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
    printf("shared anonymous\n");
    maps(shared);
    mprotect(shared + 4096, 4096, PROT_READ);
    printf("after mprotect of its middle page\n");
    maps(shared);
    maps(shared + 4096);
    maps(shared + 2 * 4096);
    mprotect(shared + 4096, 4096, PROT_READ | PROT_WRITE);
    printf("after mprotect back\n");
    maps(shared);
    int zero = open("/dev/zero", O_RDWR);
    char *zeros = mmap(NULL, 4096, PROT_READ, MAP_SHARED, zero, 8192);
    printf("shared /dev/zero at offset 8192\n");
    maps(zeros);
    char *private = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, zero, 0);
    printf("private /dev/zero\n");
    maps(private);
    // A process's directory shows its owner as it is now; its files as they were when looked up.
    int directory = open("/proc/self/", O_RDONLY);
    int file = open("/proc/self/status", O_RDONLY);
    setgid(2000);
    setuid(1000);
    struct stat st;
    fstat(directory, &st);
    printf("directory owner %d group %d\n", st.st_uid, st.st_gid);
    fstat(file, &st);
    printf("file owner %d group %d\n", st.st_uid, st.st_gid);
    stat("/proc/self/status", &st);
    printf("looked up again owner %d group %d\n", st.st_uid, st.st_gid);
    return 0;
}
