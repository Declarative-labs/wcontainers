// Runs system call sequences for layer 1 and reports what each call returned and wrote.
//
// Reads sequences from standard input, one at a time, and runs each in a child process so no state
// carries over. A sequence is: arena bytes (u32 length, bytes), then calls (u32 count, then for each the
// number and six arguments as u64). For each call the child reports the return value (i64) and the
// arena's changed byte ranges (u32 count, then u32 offset, u32 length, bytes) into a pipe the parent
// drains. After each sequence the parent writes everything the child reported (u32 length, bytes), then
// the child's wait status (u32), and puts back what the sequence may have changed. A child that dies
// part way through a report leaves a short last record, which the reader drops; one that runs past the
// deadline is killed. All numbers are little endian.

#include <dirent.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <unistd.h>

#define ARENA ((unsigned char *)0x70000000UL)
#define ARENA_SIZE (16 * 4096)
// How long a sequence may take before its child is killed: only one that waits for ever comes near it.
#define DEADLINE_SECONDS 60

static unsigned char before[ARENA_SIZE];

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
            for (uint32_t index = 0; index < calls; index++) {
                int64_t result = raw(sequence[index][0], &sequence[index][1]);
                write_all(1, &result, 8);
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
        static const struct { const char *path; int minor; } devices[] = {
            {"/dev/null", 3}, {"/dev/zero", 5}, {"/dev/full", 7}};
        for (int index = 0; index < 3; index++) {
            remove_tree(AT_FDCWD, devices[index].path);
            mknod(devices[index].path, S_IFCHR | 0666, makedev(1, devices[index].minor));
            chmod(devices[index].path, 0666);
        }
    }
}
