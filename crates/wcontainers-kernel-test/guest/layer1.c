// Runs system call sequences for layer 1 and reports what each call returned and wrote.
//
// Reads sequences from standard input, one at a time, and runs each in a child process so no state
// carries over. A sequence is: arena bytes (u32 length, bytes), then calls (u32 count, then for each the
// number and six arguments as u64, then CLEARS ranges of what the call writes that two machines cannot
// share, each an address, a length and whether the length is the call's value at most, all u64; see
// `clear_machine`). For each call the child reports the return value (i64), whether the
// call's result depends on what the runners cannot share (u32: bit 0, see `unshared`; bit 1, the call
// put a random device's bytes somewhere, which the runner then cleared, see `clear_random`), and the
// arena's changed
// byte ranges (u32 count, then u32 offset, u32 length, bytes) into a pipe the parent
// drains. After each sequence the parent writes everything the child reported (u32 length, bytes), then
// the child's wait status (u32), and puts back what the sequence may have changed. After its last
// call the child reports the play area: for each page whether process_vm_readv could read it (u8),
// then its bytes (zeros when not). A child that dies
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
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <sys/uio.h>
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
// Replaced by the last identifier shmget gave in the sequence (`SEGMENT` in layer1.rs).
#define SEGMENT 0x5e1f5e1f00000002ULL
#define NO_SEGMENT 4000
// Replaced by the last identifier msgget gave (`QUEUE` in layer1.rs).
#define QUEUE 0x5e1f5e1f00000003ULL
// Replaced by the last identifier semget gave (`SEMAPHORES` in layer1.rs).
#define SEMAPHORES 0x5e1f5e1f00000004ULL
#define PLAY 0x71000000UL
#define PLAY_PAGES 16
// The ranges sent with each call (`CLEARS` in layer1.rs).
#define CLEARS 5
#define CALL_WORDS (7 + 3 * CLEARS)

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

// Whether a descriptor is on /dev/random or /dev/urandom, whose bytes differ between the runners. By
// the bare call: musl's fstat may also call fcntl, which a sequence's filter would see.
// The device of the mqueue file system, found at the start.
static dev_t mqueue_device;

// A random device, or a message queue, whose file reads as its notification's process number,
// which differs between the runners.
static int random_device(uint64_t fd) {
    struct stat st;
    if (syscall(SYS_fstat, (int)fd, &st) != 0) {
        return 0;
    }
    return (S_ISCHR(st.st_mode) && major(st.st_rdev) == 1 && (minor(st.st_rdev) == 8 || minor(st.st_rdev) == 9)) ||
           (S_ISREG(st.st_mode) && st.st_dev == mqueue_device);
}

#define MAX_RW_COUNT 0x7ffff000UL
static unsigned char zeros[65536];
// The child's process number, asked for once: the sequence's filters must not see the runner's own calls.
static pid_t self_pid;

// Zeros a range up to the first page the process cannot write, as a copy to it would stop.
static void clear_range(uint64_t at, uint64_t length) {
    while (length) {
        uint64_t part = length < sizeof zeros ? length : sizeof zeros;
        struct iovec local = {zeros, part}, remote = {(void *)at, part};
        long put = syscall(SYS_process_vm_writev, self_pid, &local, 1, &remote, 1, 0);
        if (put <= 0) {
            return;
        }
        at += put;
        length -= put;
        if ((uint64_t)put < part) {
            return;
        }
    }
}

// After a call that read a random device, zeros what it could have written (the whole range it was
// given, up to the first page it cannot write; for sendfile, the bytes it put in a regular file), so
// the bytes go no further. Returns 2 if so, and 1 more if a read moved less than it was asked for:
// how far a copy that meets a fault gets differs (a known gap), so its count is not compared.
// `out_before` is a regular sendfile output's position before the call, or -1.
static uint32_t clear_random(const uint64_t *call, int64_t result, int64_t out_before) {
    const uint64_t *a = call + 1;
    if (result < 0) {
        return 0;
    }
    switch (call[0]) {
    case SYS_read:
    case SYS_pread64: {
        if (!random_device(a[0])) {
            return 0;
        }
        uint64_t wanted = a[2] < MAX_RW_COUNT ? a[2] : MAX_RW_COUNT;
        clear_range(a[1], wanted);
        return 2;
    }
    case SYS_readv:
    case SYS_preadv:
    case SYS_preadv2: {
        if (!random_device(a[0])) {
            return 0;
        }
        const struct iovec *parts = (const struct iovec *)a[1];
        uint64_t total = 0;
        for (uint64_t index = 0; index < a[2] && total < MAX_RW_COUNT; index++) {
            uint64_t length = parts[index].iov_len;
            if (length > MAX_RW_COUNT - total) {
                length = MAX_RW_COUNT - total;
            }
            clear_range((uint64_t)parts[index].iov_base, length);
            total += length;
        }
        return 2;
    }
    case SYS_sendfile:
        if (out_before < 0) {
            return 0;
        }
        for (int64_t done = 0; done < result;) {
            int64_t part = result - done < (int64_t)sizeof zeros ? result - done : (int64_t)sizeof zeros;
            long put = pwrite((int)a[0], zeros, part, out_before + done);
            if (put <= 0) {
                break;
            }
            done += put;
        }
        return 2;
    }
    return 0;
}

// After a call is reported, zeros what it wrote that two machines cannot share (`machine_bytes` in
// layer1.rs), so no later call reads it: after a failed call only in the arena, and a range measured by
// the call's value only after a success.
static void clear_machine(const uint64_t *clears, int64_t result) {
    for (int index = 0; index < CLEARS; index++) {
        uint64_t at = clears[index * 3], length = clears[index * 3 + 1], by_value = clears[index * 3 + 2];
        int in_arena = at >= (uint64_t)ARENA && at < (uint64_t)ARENA + ARENA_SIZE;
        if (length == 0 || (result < 0 && (by_value || !in_arena))) {
            continue;
        }
        if (by_value && (uint64_t)result < length) {
            length = (uint64_t)result;
        }
        clear_range(at, length);
    }
}

// For a sendfile from a random device to a regular file, the output's position before the call.
static int64_t random_output(const uint64_t *call) {
    struct stat st;
    if (call[0] != SYS_sendfile || !random_device(call[2]) || syscall(SYS_fstat, (int)call[1], &st) != 0 ||
        !S_ISREG(st.st_mode)) {
        return -1;
    }
    return lseek((int)call[1], 0, SEEK_CUR);
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

// A child made by clone ends at once with this status (`CHILD_STATUS` in layer1.rs), before it
// touches the stack it may share with its parent (vfork).
#define CHILD_STATUS 7

static long raw(long number, const uint64_t *a) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a[0];
    register long x1 __asm__("x1") = a[1];
    register long x2 __asm__("x2") = a[2];
    register long x3 __asm__("x3") = a[3];
    register long x4 __asm__("x4") = a[4];
    register long x5 __asm__("x5") = a[5];
    // A clone answering 0 may also be a filter's ERRNO of 0 in the caller: getpid tells them apart
    // (sequences with clone let getpid through their filters).
    register long caller __asm__("x9") = self_pid;
    __asm__ volatile("svc 0\n\t"
                     "cmp x8, %[clone]\n\t"
                     "b.ne 1f\n\t"
                     "cbnz x0, 1f\n\t"
                     "mov x8, %[getpid]\n\t"
                     "svc 0\n\t"
                     "cmp x0, x9\n\t"
                     "b.eq 2f\n\t"
                     "mov x0, %[status]\n\t"
                     "mov x8, %[exit]\n\t"
                     "svc 0\n"
                     "2:\n\t"
                     "mov x0, #0\n"
                     "1:"
                     : "+r"(x0), "+r"(x8)
                     : "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x5), "r"(caller),
                       [clone] "i"(SYS_clone), [getpid] "i"(SYS_getpid),
                       [status] "i"(CHILD_STATUS), [exit] "i"(SYS_exit_group)
                     : "memory", "cc");
    return x0;
}

// The changed ranges of the arena since `before`, written to `out` (fd 1 of the parent). Both passes
// read one copy of the arena: the kernel may rewrite a registered rseq area's CPU number whenever the
// child returns to user space, so the arena itself can change between them.
static void report_changes(int out) {
    static unsigned char now[ARENA_SIZE];
    memcpy(now, ARENA, ARENA_SIZE);
    uint32_t count = 0;
    for (int pass = 0; pass < 2; pass++) {
        if (pass == 1) {
            write_all(out, &count, 4);
        }
        int at = 0;
        while (at < ARENA_SIZE) {
            if (now[at] == before[at]) {
                at++;
                continue;
            }
            int start = at;
            while (at < ARENA_SIZE && now[at] != before[at]) {
                at++;
            }
            if (pass == 0) {
                count++;
            } else {
                uint32_t header[2] = {(uint32_t)start, (uint32_t)(at - start)};
                write_all(out, header, 8);
                write_all(out, now + start, at - start);
            }
        }
    }
    memcpy(before, now, ARENA_SIZE);
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

// Remakes the devices a sequence can name. The container's first /dev/null stays held from outside
// the container once removed (measured: its inode is never given back), so it is remade once before
// the first sequence too, and each sequence starts with one hidden inode in /dev, as ours does.
// Whether a sequence opens a pseudo-terminal (`opens_terminal` in layer1.rs).
static int opens_terminal(uint64_t (*sequence)[CALL_WORDS], uint32_t calls) {
    for (uint32_t index = 0; index < calls; index++) {
        uint64_t path = sequence[index][2];
        if ((sequence[index][0] == SYS_openat || sequence[index][0] == SYS_openat2) &&
            path >= (uint64_t)ARENA && path < (uint64_t)ARENA + ARENA_SIZE - 16) {
            const char *name = (const char *)path;
            if (strcmp(name, "/dev/ptmx") == 0 || (strncmp(name, "/dev/pts/", 9) == 0 && name[9])) {
                return 1;
            }
        }
    }
    return 0;
}

// Polls every descriptor, twice: Linux moves what one side of a pseudo-terminal wrote into the other
// side's line discipline later, from a work queue, and a poll runs that work (`n_tty_poll`); echoes
// it makes need the second round. Ours moves it at once.
static void settle_terminals(void) {
    static struct pollfd fds[64];
    struct timespec zero = {0, 0};
    for (int round = 0; round < 2; round++) {
        for (int start = 3; start < 1024; start += 64) {
            for (int index = 0; index < 64; index++) {
                fds[index].fd = start + index;
                fds[index].events = POLLIN;
            }
            if (syscall(SYS_ppoll, fds, 64, &zero, 0, 8) < 0) {
                // Fewer descriptors allowed than 64: one at a time.
                for (int index = 0; index < 64; index++) {
                    syscall(SYS_ppoll, &fds[index], 1, &zero, 0, 8);
                }
            }
        }
    }
}

static void remake_devices(void) {
    static const struct { const char *path; int major, minor; } devices[] = {
        {"/dev/null", 1, 3}, {"/dev/zero", 1, 5}, {"/dev/full", 1, 7}, {"/dev/random", 1, 8},
        {"/dev/urandom", 1, 9}, {"/dev/tty", 5, 0}};
    for (int index = 0; index < 6; index++) {
        remove_tree(AT_FDCWD, devices[index].path);
        mknod(devices[index].path, S_IFCHR | 0666, makedev(devices[index].major, devices[index].minor));
        chmod(devices[index].path, 0666);
    }
    // The master's link into devpts, and the node it names.
    remove_tree(AT_FDCWD, "/dev/ptmx");
    symlink("pts/ptmx", "/dev/ptmx");
    chown("/dev/pts/ptmx", 0, 0);
    chmod("/dev/pts/ptmx", 0666);
}

int main(void) {
    remake_devices();
    struct stat mqueue_root;
    if (stat("/dev/mqueue", &mqueue_root) != 0) {
        return 1;
    }
    mqueue_device = mqueue_root.st_dev;
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
        static uint64_t sequence[4096][CALL_WORDS];
        if (calls > 4096 || read_all(0, sequence, calls * sizeof sequence[0]) != 0) {
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
            // Our caller is the container's first process, which Linux keeps from a SIGPIPE it has
            // no handler for; ignored here too, a write with no reader is EPIPE on both sides.
            signal(SIGPIPE, SIG_IGN);
            self_pid = getpid();
            uint64_t self = (uint64_t)self_pid;
            int terminal = opens_terminal(sequence, calls);
            uint64_t segment = NO_SEGMENT;
            uint64_t queue = NO_SEGMENT;
            uint64_t semaphores = NO_SEGMENT;
            for (uint32_t index = 0; index < calls; index++) {
                for (int argument = 1; argument < 7; argument++) {
                    if (sequence[index][argument] == SELF) {
                        sequence[index][argument] = self;
                    }
                    if (sequence[index][argument] == SEGMENT) {
                        sequence[index][argument] = segment;
                    }
                    if (sequence[index][argument] == QUEUE) {
                        sequence[index][argument] = queue;
                    }
                    if (sequence[index][argument] == SEMAPHORES) {
                        sequence[index][argument] = semaphores;
                    }
                }
                int64_t out_before = random_output(sequence[index]);
                int64_t result = raw(sequence[index][0], &sequence[index][1]);
                if (sequence[index][0] == SYS_shmget && result >= 0) {
                    segment = (uint64_t)result;
                }
                if (sequence[index][0] == SYS_msgget && result >= 0) {
                    queue = (uint64_t)result;
                }
                if (sequence[index][0] == SYS_semget && result >= 0) {
                    semaphores = (uint64_t)result;
                }
                // A child is waited for until it has ended, left to be reaped.
                if (sequence[index][0] == SYS_clone && result > 0) {
                    siginfo_t info;
                    waitid(P_PID, (id_t)result, &info, WEXITED | WNOWAIT | __WALL);
                }
                uint32_t unshared = clear_random(sequence[index], result, out_before);
                if (sequence[index][0] == SYS_getdents64 || sequence[index][0] == SYS_lseek) {
                    unshared |= outlives((int)sequence[index][1]) || lists_processes((int)sequence[index][1]);
                }
                if (sequence[index][0] == SYS_getcwd) {
                    unshared |= in_proc(AT_FDCWD);
                }
                write_all(1, &result, 8);
                write_all(1, &unshared, 4);
                report_changes(1);
                clear_machine(&sequence[index][7], result);
                if (terminal) {
                    settle_terminals();
                }
            }
            static unsigned char page[4096];
            for (int index = 0; index < PLAY_PAGES; index++) {
                struct iovec local = {page, sizeof page};
                struct iovec remote = {(void *)(PLAY + index * 4096UL), sizeof page};
                unsigned char readable = syscall(SYS_process_vm_readv, self_pid, &local, 1, &remote, 1, 0) == sizeof page;
                if (!readable) {
                    memset(page, 0, sizeof page);
                }
                write_all(1, &readable, 1);
                write_all(1, page, sizeof page);
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
        // The sequence's own children, handed to this first process when it ended, so that no
        // later sequence finds them (kill of every process would reach them).
        while (waitpid(-1, NULL, WNOHANG) > 0) {
        }
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
        // Every shared memory segment goes; the slots and sequence numbers carry over.
        static unsigned char segment_status[112];
        long highest = syscall(SYS_shmctl, 0, 3, segment_status);
        for (long slot = 0; slot <= highest; slot++) {
            long id = syscall(SYS_shmctl, slot, 15, segment_status);
            if (id >= 0) {
                // As its creator, who may remove it whoever owns it now.
                uint32_t creator;
                memcpy(&creator, segment_status + 12, 4);
                syscall(SYS_setresuid, -1, creator, -1);
                syscall(SYS_shmctl, id, 0, 0);
                syscall(SYS_setresuid, -1, 0, -1);
                // A change of user takes away dumpable, which later sequences inherit.
                prctl(PR_SET_DUMPABLE, 1);
            }
        }
        // So does every message queue.
        static unsigned char queue_status[120];
        highest = syscall(SYS_msgctl, 0, 3, queue_status);
        for (long slot = 0; slot <= highest; slot++) {
            long id = syscall(SYS_msgctl, slot, 13, queue_status);
            if (id >= 0) {
                uint32_t creator;
                memcpy(&creator, queue_status + 12, 4);
                syscall(SYS_setresuid, -1, creator, -1);
                syscall(SYS_msgctl, id, 0, 0);
                syscall(SYS_setresuid, -1, 0, -1);
                prctl(PR_SET_DUMPABLE, 1);
            }
        }
        // And every semaphore set.
        static unsigned char set_status[88];
        highest = syscall(SYS_semctl, 0, 0, 3, set_status);
        for (long slot = 0; slot <= highest; slot++) {
            long id = syscall(SYS_semctl, slot, 0, 20, set_status);
            if (id >= 0) {
                uint32_t creator;
                memcpy(&creator, set_status + 12, 4);
                syscall(SYS_setresuid, -1, creator, -1);
                syscall(SYS_semctl, id, 0, 0, 0);
                syscall(SYS_setresuid, -1, 0, -1);
                prctl(PR_SET_DUMPABLE, 1);
            }
        }
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
        // Every message queue goes, and the mqueue root gets back its owner and mode.
        DIR *queues = opendir("/dev/mqueue");
        for (struct dirent *entry; queues && (entry = readdir(queues));) {
            if (entry->d_name[0] != '.') {
                unlinkat(dirfd(queues), entry->d_name, 0);
            }
        }
        if (queues) {
            closedir(queues);
        }
        chown("/dev/mqueue", 0, 0);
        chmod("/dev/mqueue", 01777);
        remake_devices();
        // `..` from a process directory reaches /proc, which keeps a change of owner or mode.
        chown("/proc", 0, 0);
        chmod("/proc", 0555);
        // So does devpts's root.
        chown("/dev/pts", 0, 0);
        chmod("/dev/pts", 0755);
    }
}
