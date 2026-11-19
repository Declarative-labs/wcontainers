// /sys: the read-only sysfs a default container mounts there, its root's details, its file
// system, and what may not be done on it. With "lookups", looks up names Linux has under it and
// prints nothing.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/statvfs.h>
#include <sys/time.h>
#include <sys/xattr.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

// A file's mode and what it holds, or why it cannot be opened.
static void show(const char *path) {
    struct stat st;
    if (stat(path, &st) < 0) {
        printf("%s: stat errno %d\n", path, errno);
        return;
    }
    char text[128] = "";
    int fd = open(path, O_RDONLY);
    int error = errno;
    if (fd >= 0) {
        long got = read(fd, text, sizeof text - 1);
        text[got > 0 ? got : 0] = 0;
        close(fd);
    }
    printf("%s: mode %o %s%s", path, st.st_mode, fd < 0 ? strerror(error) : text,
           fd < 0 ? "\n" : "");
}

// The huge pages the machine offers and holds: none is ever reserved in a default container.
static void hugepages(void) {
    const char *sizes[] = {"64kB", "2048kB", "32768kB", "1048576kB"};
    const char *files[] = {"free_hugepages", "nr_hugepages", "nr_overcommit_hugepages",
                           "resv_hugepages", "surplus_hugepages", "demote_size"};
    char path[128];
    show("/sys/kernel/mm/hugepages");
    for (unsigned i = 0; i < 4; i++) {
        for (unsigned j = 0; j < 6; j++) {
            snprintf(path, sizeof path, "/sys/kernel/mm/hugepages/hugepages-%s/%s", sizes[i], files[j]);
            show(path);
        }
    }
    p("write nr_hugepages", open("/sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages", O_WRONLY));
    // Same-page merging, built and never run.
    const char *ksm[] = {"advisor_max_cpu", "advisor_max_pages_to_scan", "advisor_min_pages_to_scan",
                         "advisor_mode", "advisor_target_scan_time", "full_scans", "general_profit",
                         "ksm_zero_pages", "max_page_sharing", "pages_scanned", "pages_shared",
                         "pages_sharing", "pages_skipped", "pages_to_scan", "pages_unshared",
                         "pages_volatile", "run", "sleep_millisecs", "smart_scan", "stable_node_chains",
                         "stable_node_chains_prune_millisecs", "stable_node_dups", "use_zero_pages"};
    for (unsigned i = 0; i < sizeof ksm / sizeof *ksm; i++) {
        snprintf(path, sizeof path, "/sys/kernel/mm/ksm/%s", ksm[i]);
        show(path);
    }
    p("write ksm run", open("/sys/kernel/mm/ksm/run", O_WRONLY));
    show("/proc/sys/vm/nr_hugepages");
    show("/proc/sys/vm/nr_overcommit_hugepages");
    show("/proc/sys/vm/hugetlb_shm_group");
    show("/proc/sys/vm/drop_caches");
    p("write drop_caches", open("/proc/sys/vm/drop_caches", O_WRONLY));
    char line[256];
    FILE *meminfo = fopen("/proc/meminfo", "r");
    while (fgets(line, sizeof line, meminfo)) {
        if (strncmp(line, "HugePages_", 10) == 0 || strncmp(line, "Hugepagesize", 12) == 0 ||
            strncmp(line, "Hugetlb:", 8) == 0) {
            fputs(line, stdout);
        }
    }
    fclose(meminfo);
}

// A file's mode only, for files whose values say what each machine does.
static void mode_of(const char *path) {
    struct stat st;
    int r = stat(path, &st);
    printf("%s: %s %o\n", path, r < 0 ? "stat errno" : "mode", r < 0 ? errno : st.st_mode);
}

static int by_name(const void *a, const void *b) {
    return strcmp(*(char *const *)a, *(char *const *)b);
}

// The modes of a directory's entries, sorted by name.
static void modes_in(const char *path) {
    char *names[32];
    unsigned count = 0;
    DIR *dir = opendir(path);
    struct dirent *entry;
    while (dir && (entry = readdir(dir)) && count < 32) {
        if (entry->d_name[0] != '.') {
            names[count++] = strdup(entry->d_name);
        }
    }
    if (dir) {
        closedir(dir);
    }
    qsort(names, count, sizeof *names, by_name);
    for (unsigned i = 0; i < count; i++) {
        char file[300];
        snprintf(file, sizeof file, "%s/%s", path, names[i]);
        mode_of(file);
    }
}

// Transparent huge pages: the settings and their choices; whether any are made, and the counts,
// are each machine's own.
static void transparent(void) {
    const char *base = "/sys/kernel/mm/transparent_hugepage";
    const char *same[] = {"defrag", "shmem_enabled", "shrink_underused", "use_zero_page",
                          "hpage_pmd_size", "khugepaged/alloc_sleep_millisecs", "khugepaged/defrag",
                          "khugepaged/max_ptes_none", "khugepaged/max_ptes_shared",
                          "khugepaged/max_ptes_swap", "khugepaged/pages_to_scan",
                          "khugepaged/scan_sleep_millisecs"};
    const char *own[] = {"enabled", "khugepaged/full_scans", "khugepaged/pages_collapsed"};
    char path[256];
    for (unsigned i = 0; i < sizeof same / sizeof *same; i++) {
        snprintf(path, sizeof path, "%s/%s", base, same[i]);
        show(path);
    }
    for (unsigned i = 0; i < sizeof own / sizeof *own; i++) {
        snprintf(path, sizeof path, "%s/%s", base, own[i]);
        mode_of(path);
    }
    p("write enabled", open("/sys/kernel/mm/transparent_hugepage/enabled", O_WRONLY));
    // Every entry, sorted, and in each size the files and counts.
    char *names[64];
    unsigned count = 0;
    DIR *dir = opendir(base);
    struct dirent *entry;
    while ((entry = readdir(dir)) && count < 64) {
        names[count++] = strdup(entry->d_name);
    }
    closedir(dir);
    qsort(names, count, sizeof *names, by_name);
    for (unsigned i = 0; i < count; i++) {
        printf("entry %s\n", names[i]);
        if (strncmp(names[i], "hugepages-", 10) != 0) {
            continue;
        }
        snprintf(path, sizeof path, "%s/%s", base, names[i]);
        modes_in(path);
        snprintf(path, sizeof path, "%s/%s/stats", base, names[i]);
        modes_in(path);
    }
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "hugepages") == 0) {
        hugepages();
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "transparent") == 0) {
        transparent();
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "lookups") == 0) {
        struct stat st;
        stat("/sys/kernel/mm/lru_gen/enabled", &st);
        stat("/sys/fs/cgroup/cpu.max", &st);
        stat("/sys/devices/system/cpu/online", &st);
        stat("/sys/no-such-entry", &st);
        return 0;
    }
    struct stat st;
    p("stat", stat("/sys", &st));
    printf("mode %o uid %u gid %u ino %lu size %ld blocks %ld blksize %ld\n", st.st_mode, st.st_uid,
           st.st_gid, (unsigned long)st.st_ino, (long)st.st_size, (long)st.st_blocks,
           (long)st.st_blksize);
    struct stat root;
    stat("/", &root);
    printf("own device %d\n", st.st_dev != root.st_dev);

    struct statfs fs;
    p("statfs", statfs("/sys", &fs));
    printf("type %lx bsize %ld blocks %lu bfree %lu files %lu ffree %lu namelen %ld frsize %ld flags %lx\n",
           (unsigned long)fs.f_type, (long)fs.f_bsize, (unsigned long)fs.f_blocks,
           (unsigned long)fs.f_bfree, (unsigned long)fs.f_files, (unsigned long)fs.f_ffree,
           (long)fs.f_namelen, (long)fs.f_frsize, (unsigned long)fs.f_flags);

    // Read-only: nothing is made, changed or written there.
    p("mkdir", mkdir("/sys/made", 0755));
    p("creat", open("/sys/made", O_CREAT | O_WRONLY, 0644));
    p("mknod", mknod("/sys/fifo", S_IFIFO | 0644, 0));
    p("symlink", symlink("x", "/sys/link"));
    p("chmod", chmod("/sys", 0755));
    p("chown", chown("/sys", 1, 1));
    p("utimes", utimes("/sys", NULL));
    p("access w", access("/sys", W_OK));
    p("access r", access("/sys", R_OK | X_OK));
    p("rmdir", rmdir("/sys"));
    p("open dir write", open("/sys", O_RDWR));
    p("setxattr", setxattr("/sys", "user.x", "1", 1, 0));

    // Names it lacks in every machine.
    p("missing", stat("/sys/no-such-entry", &st));
    char longname[300];
    memset(longname, 'a', sizeof longname);
    memcpy(longname, "/sys/", 5);
    longname[sizeof longname - 1] = 0;
    p("long name", stat(longname, &st));
    p("missing under", stat("/sys/no-such-entry/x", &st));

    int fd = open("/sys", O_RDONLY | O_DIRECTORY);
    p("open", fd);
    struct statvfs vfs;
    p("fstatvfs", fstatvfs(fd, &vfs));
    printf("vfs flags %lx\n", vfs.f_flag);
    p("fchdir", fchdir(fd));
    char cwd[64];
    printf("cwd %s\n", getcwd(cwd, sizeof cwd));
    p("parent", stat("..", &st));
    printf("parent is root %d\n", st.st_ino == root.st_ino && st.st_dev == root.st_dev);
    return 0;
}
