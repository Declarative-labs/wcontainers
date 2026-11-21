// Docker's masks: /dev/null bound over /proc files that tell of the host, an empty read-only
// tmpfs over /proc/scsi and /sys/firmware, and what /proc's listing shows of them.
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/sysmacros.h>
#include <unistd.h>

int main(void) {
    struct stat null;
    stat("/dev/null", &null);
    const char *files[] = {"/proc/interrupts", "/proc/kcore", "/proc/keys", "/proc/timer_list"};
    for (int i = 0; i < 4; i++) {
        struct stat st;
        int found = stat(files[i], &st);
        int fd = open(files[i], O_RDWR);
        char buffer[16];
        long got = fd >= 0 ? read(fd, buffer, sizeof buffer) : -1;
        long put = fd >= 0 ? write(fd, "x", 1) : -1;
        printf("%s stat %d mode %o rdev %u:%u is null %d read %ld write %ld\n", files[i], found,
               st.st_mode, major(st.st_rdev), minor(st.st_rdev),
               st.st_dev == null.st_dev && st.st_ino == null.st_ino, got, put);
        if (fd >= 0)
            close(fd);
    }
    struct stat scsi, firmware;
    stat("/proc/scsi", &scsi);
    stat("/sys/firmware", &firmware);
    printf("scsi mode %o uid %u links %lu, same as firmware %d\n", scsi.st_mode, scsi.st_uid,
           (unsigned long)scsi.st_nlink,
           scsi.st_dev == firmware.st_dev && scsi.st_ino == firmware.st_ino);
    const char *empty[] = {"/proc/scsi", "/sys/firmware"};
    for (int i = 0; i < 2; i++) {
        struct statfs fs;
        statfs(empty[i], &fs);
        printf("%s type %lx blocks %ld files %ld free %ld flags %lx\n", empty[i], (long)fs.f_type,
               (long)fs.f_blocks, (long)fs.f_files, (long)fs.f_ffree, (long)fs.f_flags);
        DIR *dir = opendir(empty[i]);
        int entries = 0;
        for (struct dirent *entry; dir && (entry = readdir(dir));)
            entries++;
        if (dir)
            closedir(dir);
        char path[64];
        snprintf(path, sizeof path, "%s/file", empty[i]);
        int made = open(path, O_CREAT | O_WRONLY, 0644);
        printf("  entries %d create %d %s\n", entries, made, made < 0 ? strerror(errno) : "");
        // Up and back again.
        snprintf(path, sizeof path, "%s/..", empty[i]);
        struct stat up;
        stat(path, &up);
        printf("  up is a directory %d\n", S_ISDIR(up.st_mode));
    }
    // /proc lists the entries under the masks as its own.
    DIR *proc = opendir("/proc");
    for (struct dirent *entry; (entry = readdir(proc));) {
        for (int i = 0; i < 4; i++)
            if (strcmp(entry->d_name, files[i] + 6) == 0)
                printf("listed %s type %d\n", entry->d_name, entry->d_type);
        if (strcmp(entry->d_name, "scsi") == 0)
            printf("listed scsi type %d\n", entry->d_type);
    }
    closedir(proc);
    return 0;
}
