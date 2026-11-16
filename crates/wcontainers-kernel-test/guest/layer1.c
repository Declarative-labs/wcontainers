// Runs system call sequences for layer 1 and reports what each call returned and wrote.
//
// Reads sequences from standard input, one at a time, and runs each in a child process so no state
// carries over. A sequence is: arena bytes (u32 length, bytes), then calls (u32 count, then for each the
// number and six arguments as u64). For each call the child reports the return value (i64), whether the
// call's result depends on what the runners cannot share (u32, see `unshared`), and the arena's changed
// byte ranges (u32 count, then u32 offset, u32 length, bytes) into a pipe the parent
// drains. After each sequence the parent writes everything the child reported (u32 length, bytes), then
// the child's wait status (u32), and puts back what the sequence may have changed. A child that dies
// part way through a report leaves a short last record, which the reader drops; one that runs past the
// deadline is killed. The child first makes itself a session leader, as a container's first process is,
// and an argument equal to SELF is replaced by its process number.
// All numbers are little endian.

#define _GNU_SOURCE
#include <dirent.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <sys/xattr.h>
#include <unistd.h>

#ifndef AT_HANDLE_FID
#define AT_HANDLE_FID 0x200
#endif

#define ARENA ((unsigned char *)0x70000000UL)
#define ARENA_SIZE (16 * 4096)
// How long a sequence may take before its child is killed: only one that waits for ever comes near it.
#define DEADLINE_SECONDS 60
#define SELF 0x5e1f5e1f00000001ULL

static unsigned char before[ARENA_SIZE];

// The directories that outlive every sequence and whose offsets therefore count every entry earlier
// sequences made there: /dev/shm (each sequence's work directory) and /dev (the devices are remade).
// Each is known by its file handle and mount.
struct handle {
    struct file_handle header;
    unsigned char bytes[128];
};
static struct handle outliving[2];
static int outliving_mount[2];
// The mount of /proc, where a working directory's path names the caller's process number.
static int proc_mount;

// Whether a directory lies on /proc's own mount, asked for an identifier any file system gives.
static uint32_t in_proc(int fd) {
    struct handle handle;
    int mount;
    handle.header.handle_bytes = sizeof handle.bytes;
    return syscall(SYS_name_to_handle_at, fd, "", &handle.header, &mount, AT_EMPTY_PATH | AT_HANDLE_FID) == 0 &&
           mount == proc_mount;
}

static int handle_of(int fd, struct handle *handle, int *mount) {
    handle->header.handle_bytes = sizeof handle->bytes;
    return syscall(SYS_name_to_handle_at, fd, "", &handle->header, mount, AT_EMPTY_PATH);
}

// Whether a descriptor is on a directory that outlives the sequence.
static uint32_t outlives(int fd) {
    struct handle handle;
    int mount;
    if (handle_of(fd, &handle, &mount) != 0) {
        return 0;
    }
    for (int index = 0; index < 2; index++) {
        if (mount == outliving_mount[index] && handle.header.handle_type == outliving[index].header.handle_type &&
            handle.header.handle_bytes == outliving[index].header.handle_bytes &&
            memcmp(handle.bytes, outliving[index].bytes, handle.header.handle_bytes) == 0) {
            return 1;
        }
    }
    return 0;
}

// Whether a descriptor is on /proc itself or a process directory there, whose listings hold the
// machine's processes and the chosen subset of Linux's entries (a declared difference).
static uint32_t lists_processes(int fd) {
    char link[64], path[256];
    snprintf(link, sizeof link, "/proc/self/fd/%d", fd);
    ssize_t length = readlink(link, path, sizeof path - 1);
    if (length < 0) {
        return 0;
    }
    path[length] = 0;
    if (strcmp(path, "/proc") == 0) {
        return 1;
    }
    if (strncmp(path, "/proc/", 6) != 0 || path[6] == 0) {
        return 0;
    }
    for (char *at = path + 6; *at; at++) {
        if (*at < '0' || *at > '9') {
            return 0;
        }
    }
    return 1;
}

static int read_all(int fd, void *into, size_t length) {
    unsigned char *at = into;
    while (length) {
        ssize_t got = read(fd, at, length);
        if (got <= 0) {
            return -1;
        }
        at += got;
        length -= got;
    }
    return 0;
}

static void write_all(int fd, const void *from, size_t length) {
    const unsigned char *at = from;
    while (length) {
        ssize_t put = write(fd, at, length);
        if (put <= 0) {
            _exit(2);
        }
        at += put;
        length -= put;
    }
}

static long raw(long number, const uint64_t *a) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a[0];
    register long x1 __asm__("x1") = a[1];
    register long x2 __asm__("x2") = a[2];
    register long x3 __asm__("x3") = a[3];
    register long x4 __asm__("x4") = a[4];
    register long x5 __asm__("x5") = a[5];
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x5) : "memory");
    return x0;
}

// The changed ranges of the arena since `before`, written to `out` (fd 1 of the parent).
static void report_changes(int out) {
    uint32_t count = 0;
    for (int pass = 0; pass < 2; pass++) {
        if (pass == 1) {
            write_all(out, &count, 4);
        }
        int at = 0;
        while (at < ARENA_SIZE) {
            if (ARENA[at] == before[at]) {
                at++;
                continue;
            }
            int start = at;
            while (at < ARENA_SIZE && ARENA[at] != before[at]) {
                at++;
            }
            if (pass == 0) {
                count++;
            } else {
                uint32_t header[2] = {(uint32_t)start, (uint32_t)(at - start)};
                write_all(out, header, 8);
                write_all(out, ARENA + start, at - start);
            }
        }
    }
    memcpy(before, ARENA, ARENA_SIZE);
}

static void remove_tree(int directory_fd, const char *name) {
    if (unlinkat(directory_fd, name, 0) == 0) {
        return;
    }
    int fd = openat(directory_fd, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW);
    if (fd >= 0) {
        fchmod(fd, 0700);
        DIR *listing = fdopendir(fd);
        struct dirent *entry;
        while (listing && (entry = readdir(listing))) {
            if (strcmp(entry->d_name, ".") && strcmp(entry->d_name, "..")) {
                remove_tree(fd, entry->d_name);
            }
        }
        if (listing) {
            closedir(listing);
        }
    }
    unlinkat(directory_fd, name, AT_REMOVEDIR);
}

int main(void) {
    static unsigned char arena_init[ARENA_SIZE];
    static const char *const outliving_paths[] = {"/dev/shm", "/dev"};
    for (int index = 0; index < 2; index++) {
        int fd = open(outliving_paths[index], O_RDONLY | O_DIRECTORY);
        if (fd < 0 || handle_of(fd, &outliving[index], &outliving_mount[index]) != 0) {
            return 1;
        }
        close(fd);
    }
    struct handle proc_handle;
    proc_handle.header.handle_bytes = sizeof proc_handle.bytes;
    if (syscall(SYS_name_to_handle_at, AT_FDCWD, "/proc", &proc_handle.header, &proc_mount, AT_HANDLE_FID) != 0) {
        return 1;
    }
    for (;;) {
        uint32_t length;
        if (read_all(0, &length, 4) != 0 || length > ARENA_SIZE || read_all(0, arena_init, length) != 0) {
            return 0;
        }
        uint32_t calls;
        if (read_all(0, &calls, 4) != 0) {
            return 1;
        }
        static uint64_t sequence[4096][7];
        if (calls > 4096 || read_all(0, sequence, calls * 56) != 0) {
            return 1;
        }
        int report[2];
        if (pipe(report) != 0) {
            return 1;
        }
        pid_t child = fork();
        if (child == 0) {
            dup2(report[1], 1);
            close(report[0]);
            close(report[1]);
            if (mmap(ARENA, ARENA_SIZE, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0) != ARENA) {
                _exit(3);
            }
            memcpy(ARENA, arena_init, length);
            memcpy(before, ARENA, ARENA_SIZE);
            setsid();
            uint64_t self = (uint64_t)getpid();
            for (uint32_t index = 0; index < calls; index++) {
                for (int argument = 1; argument < 7; argument++) {
                    if (sequence[index][argument] == SELF) {
                        sequence[index][argument] = self;
                    }
                }
                int64_t result = raw(sequence[index][0], &sequence[index][1]);
                uint32_t unshared = 0;
                if (sequence[index][0] == SYS_getdents64 || sequence[index][0] == SYS_lseek) {
                    unshared = outlives((int)sequence[index][1]) || lists_processes((int)sequence[index][1]);
                }
                if (sequence[index][0] == SYS_getcwd) {
                    unshared = in_proc(AT_FDCWD);
                }
                write_all(1, &result, 8);
                write_all(1, &unshared, 4);
                report_changes(1);
            }
            _exit(0);
        }
        close(report[1]);
        static unsigned char *reported;
        static size_t capacity;
        size_t used = 0;
        struct timespec now, deadline;
        clock_gettime(CLOCK_MONOTONIC, &deadline);
        deadline.tv_sec += DEADLINE_SECONDS;
        int killed = 0;
        for (;;) {
            if (capacity - used < 65536) {
                capacity = capacity * 2 + 65536;
                reported = realloc(reported, capacity);
                if (!reported) {
                    return 1;
                }
            }
            clock_gettime(CLOCK_MONOTONIC, &now);
            long left = (deadline.tv_sec - now.tv_sec) * 1000 + (deadline.tv_nsec - now.tv_nsec) / 1000000;
            struct pollfd ready = {report[0], POLLIN, 0};
            if (!killed && (left <= 0 || poll(&ready, 1, left) == 0)) {
                kill(child, SIGKILL);
                killed = 1;
            }
            ssize_t got = read(report[0], reported + used, capacity - used);
            if (got <= 0) {
                break;
            }
            used += got;
        }
        close(report[0]);
        int status;
        waitpid(child, &status, 0);
        uint32_t header = (uint32_t)used;
        write_all(1, &header, 4);
        write_all(1, reported, used);
        uint32_t end = (uint32_t)status;
        write_all(1, &end, 4);
        // Sequences may change anything they can name, so everything they can name outside their work
        // directory is put back: /dev/shm emptied with its own mode, and the devices remade.
        DIR *shm = opendir("/dev/shm");
        struct dirent *entry;
        while (shm && (entry = readdir(shm))) {
            if (strcmp(entry->d_name, ".") && strcmp(entry->d_name, "..")) {
                remove_tree(dirfd(shm), entry->d_name);
            }
        }
        if (shm) {
            closedir(shm);
        }
        chown("/dev/shm", 0, 0);
        chmod("/dev/shm", 01777);
        static char names[65536];
        long names_length = listxattr("/dev/shm", names, sizeof names);
        for (long at = 0; at < names_length; at += strlen(names + at) + 1) {
            removexattr("/dev/shm", names + at);
        }
        // A descriptor on /dev/shm and ".." reach /dev, and from there the root: names from the pool
        // made there go, and the two directories get back their owner, mode and attributes.
        static const char *const above[] = {"/dev", "/"};
        static const char *const pool_names[] = {"a", "b", "c", "l"};
        for (int index = 0; index < 2; index++) {
            int directory = open(above[index], O_RDONLY | O_DIRECTORY);
            for (int name = 0; name < 4; name++) {
                remove_tree(directory, pool_names[name]);
            }
            close(directory);
            chown(above[index], 0, 0);
            chmod(above[index], 0755);
            names_length = listxattr(above[index], names, sizeof names);
            for (long at = 0; at < names_length; at += strlen(names + at) + 1) {
                removexattr(above[index], names + at);
            }
        }
        static const struct { const char *path; int minor; } devices[] = {
            {"/dev/null", 3}, {"/dev/zero", 5}, {"/dev/full", 7}};
        for (int index = 0; index < 3; index++) {
            remove_tree(AT_FDCWD, devices[index].path);
            mknod(devices[index].path, S_IFCHR | 0666, makedev(1, devices[index].minor));
            chmod(devices[index].path, 0666);
        }
        // `..` from a process directory reaches /proc, which keeps a change of owner or mode.
        chown("/proc", 0, 0);
        chmod("/proc", 0555);
    }
}
