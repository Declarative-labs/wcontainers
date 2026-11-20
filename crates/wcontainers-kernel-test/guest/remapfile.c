// remap_file_pages: its refusals, then windows of a tmpfs file, a /dev/shm file and shared anonymous
// memory remapped page by page, each checked by its bytes and by /proc/self/maps; then a System V
// segment remapped before and after its removal.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/shm.h>
#include <sys/syscall.h>
#include <unistd.h>

#define PAGE 4096
#define PAGES 8
#define WINDOW ((char *)0x48000000)

static long remap(void *start, size_t size, long prot, long pgoff, long flags) {
    return syscall(SYS_remap_file_pages, start, size, prot, pgoff, flags);
}

static void result(const char *what, long value) {
    printf("%s: %ld errno %d\n", what, value, value < 0 ? errno : 0);
}

// The lines of /proc/self/maps inside the window, addresses relative to it.
static void maps(const char *what) {
    char line[512];
    FILE *file = fopen("/proc/self/maps", "r");
    printf("%s maps:\n", what);
    while (fgets(line, sizeof line, file)) {
        unsigned long start, end, offset;
        char permissions[8], device[16];
        unsigned long inode;
        char path[300] = "";
        if (sscanf(line, "%lx-%lx %7s %lx %15s %lu %299[^\n]", &start, &end, permissions, &offset,
                   device, &inode, path) < 6) {
            continue;
        }
        if (start < (unsigned long)WINDOW || start >= (unsigned long)WINDOW + 4 * PAGES * PAGE) {
            continue;
        }
        char *name = strrchr(path, '/');
        printf("  %lx-%lx %s %lx %s\n", start - (unsigned long)WINDOW, end - (unsigned long)WINDOW,
               permissions, offset, name ? name : path);
    }
    fclose(file);
}

// The first byte of each page of the window.
static void contents(const char *what, const char *at) {
    printf("%s:", what);
    for (int page = 0; page < PAGES; page++) {
        printf(" %d", at[page * PAGE]);
    }
    printf("\n");
}

// Each page of the file holds its page number; the window is mapped shared, then remapped in
// reverse, pairs of pages at a time, and one page twice.
static void window(const char *what, int fd, char *at, long locked) {
    if (fd >= 0) {
        char page[PAGE];
        for (int number = 0; number < PAGES; number++) {
            memset(page, number, PAGE);
            pwrite(fd, page, PAGE, number * PAGE);
        }
    }
    int flags = (fd < 0 ? MAP_SHARED | MAP_ANONYMOUS : MAP_SHARED) | MAP_FIXED_NOREPLACE;
    char *data = mmap(at, PAGES * PAGE, PROT_READ | PROT_WRITE, flags, fd, 0);
    result(what, data == at ? 0 : -1);
    if (fd < 0) {
        for (int number = 0; number < PAGES; number++) {
            memset(data + number * PAGE, number, PAGE);
        }
    }
    if (locked) {
        result("mlock", mlock(data, PAGES * PAGE));
    }
    for (int page = 0; page < PAGES; page += 2) {
        result("remap pair", remap(data + page * PAGE, 2 * PAGE, 0, PAGES - page - 2, 0));
    }
    contents("after pairs", data);
    maps("after pairs");
    result("remap one", remap(data + 3 * PAGE + 17, PAGE + 5, 0, 0, MAP_NONBLOCK));
    contents("after one", data);
    data[3 * PAGE + 1] = 42;
    printf("page 0 byte 1 now %d\n", data[1]);
    result("remap whole back", remap(data, PAGES * PAGE, 0, 0, 0));
    contents("back", data);
    maps("back");
    result("remap past the file", remap(data, PAGE, 0, 100, 0));
    maps("past the file");
    result("munmap", munmap(data, PAGES * PAGE));
}

static void attached(const char *what, int id) {
    struct shmid_ds state;
    long got = shmctl(id, IPC_STAT, &state);
    printf("%s: stat %ld errno %d attached %lu\n", what, got, got < 0 ? errno : 0,
           got < 0 ? 0 : (unsigned long)state.shm_nattch);
}

// A System V segment remapped while it lives and after its removal while other mappings show it;
// then a segment removed and shown only by the window remapped.
static void segment(char *at) {
    int id = shmget(IPC_PRIVATE, 2 * PAGE, IPC_CREAT | 0600);
    char *data = shmat(id, at, 0);
    result("segment attached", data == at ? 0 : -1);
    memset(data + PAGE, 7, PAGE);
    result("segment remap", remap(data, PAGE, 0, 1, 0));
    printf("segment first byte %d\n", data[0]);
    attached("segment live", id);
    char *again = shmat(id, at + 4 * PAGE, 0);
    result("segment attached again", again == at + 4 * PAGE ? 0 : -1);
    result("segment removed", shmctl(id, IPC_RMID, NULL));
    result("segment remap shown elsewhere", remap(data, PAGE, 0, 1, 0));
    attached("segment removed shown elsewhere", id);
    maps("segment removed");
    result("segment detach other", shmdt(again));
    result("segment detach", shmdt(data));
    attached("segment detached", id);

    int alone = shmget(IPC_PRIVATE, PAGE, IPC_CREAT | 0600);
    char *window = shmat(alone, at, 0);
    result("lone segment attached", window == at ? 0 : -1);
    result("lone segment removed", shmctl(alone, IPC_RMID, NULL));
    result("lone segment remap", remap(window, PAGE, 0, 0, 0));
    maps("lone segment gone");
    attached("lone segment gone", alone);
    result("lone segment detach", shmdt(window));
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int fd = open("/tmp/remapfile", O_RDWR | O_CREAT | O_TRUNC, 0600);
    ftruncate(fd, PAGES * PAGE);
    char *shared = mmap(WINDOW, 2 * PAGE, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_FIXED_NOREPLACE, fd, 0);
    char *private = mmap(WINDOW + 4 * PAGE, PAGE, PROT_READ | PROT_WRITE,
                         MAP_PRIVATE | MAP_FIXED_NOREPLACE, fd, 0);
    char *other = mmap(WINDOW + 2 * PAGE, PAGE, PROT_READ, MAP_SHARED | MAP_FIXED_NOREPLACE, fd, 0);
    result("maps", shared == WINDOW && private == WINDOW + 4 * PAGE && other == WINDOW + 2 * PAGE ? 0 : -1);
    result("private", remap(private, PAGE, 0, 1, 0));
    result("unmapped", remap(WINDOW + 6 * PAGE, PAGE, 0, 1, 0));
    result("size 0", remap(shared, 0, 0, 1, 0));
    result("size under a page", remap(shared, 100, 0, 1, 0));
    result("prot", remap(shared, PAGE, PROT_READ, 1, 0));
    result("prot -1", remap(shared, PAGE, -1, 1, 0));
    result("wrapping size", remap(shared, -PAGE, 0, 1, 0));
    result("wrapping offset", remap(shared, PAGE, 0, -1L, 0));
    result("into other flags", remap(shared, 3 * PAGE, 0, 1, 0));
    result("past the last", remap(other, 2 * PAGE, 0, 1, 0));
    result("ok", remap(shared + PAGE, PAGE, 0, 3, 0));
    maps("small");
    result("into a hole", remap(shared, 6 * PAGE, 0, 1, 0));
    munmap(WINDOW, 6 * PAGE);

    window("tmp file", fd, WINDOW, 0);
    int shm = open("/dev/shm/remapfile", O_RDWR | O_CREAT | O_TRUNC, 0600);
    ftruncate(shm, PAGES * PAGE);
    window("shm file", shm, WINDOW + PAGES * PAGE, 1);
    window("shared anonymous", -1, WINDOW + 2 * PAGES * PAGE, 0);
    segment(WINDOW + 3 * PAGES * PAGE);
    unlink("/dev/shm/remapfile");
    unlink("/tmp/remapfile");
    return 0;
}
