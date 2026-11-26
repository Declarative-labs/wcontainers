// The edges of the space placement reaches, with the layout not random: a hint below the lowest
// address a mapping may take, and the gap between the mappings and the stack once everything
// below is full.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/personality.h>
#include <unistd.h>

#define PAGE 4096
#define ANONYMOUS (MAP_PRIVATE | MAP_ANONYMOUS)
#define LARGEST_PIECE ((size_t)1 << 40)

// /proc/self/maps once the space is full: read without stdio, which may want memory there is no
// room for.
static char maps[1 << 20];

static size_t read_all(const char *path, char *buffer, size_t size) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) return 0;
    size_t length = 0;
    ssize_t got;
    while (length < size - 1 && (got = read(fd, buffer + length, size - 1 - length)) > 0) length += got;
    close(fd);
    buffer[length] = 0;
    return length;
}

static const char *outcome(void *at, uintptr_t expected) {
    if (at == MAP_FAILED) return strerror(errno);
    return (uintptr_t)at == expected ? "at the lowest address" : "elsewhere";
}

int main(int argc, char **argv) {
    if (argc < 2) {
        personality(ADDR_NO_RANDOMIZE);
        execl(argv[0], argv[0], "placed", (char *)NULL);
        return 1;
    }
    char text[64];
    uintptr_t lowest = 0;
    if (read_all("/proc/sys/vm/mmap_min_addr", text, sizeof text) == 0 ||
        sscanf(text, "%lu", &lowest) != 1) {
        printf("no mmap_min_addr\n");
        return 1;
    }
    printf("mmap_min_addr read, a whole number of pages: %s\n", lowest % PAGE == 0 ? "yes" : "no");

    // A hint below the lowest address is moved up to it; a fixed address there is refused.
    void *first = mmap((void *)PAGE, PAGE, PROT_READ, ANONYMOUS, -1, 0);
    printf("hint below the lowest address: %s\n", outcome(first, lowest));
    void *again = mmap((void *)PAGE, PAGE, PROT_READ, ANONYMOUS, -1, 0);
    printf("the same hint again: %s\n", outcome(again, lowest));
    void *taken = mmap((void *)lowest, PAGE, PROT_READ, ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
    printf("no replacing at the lowest address: %s\n", outcome(taken, lowest));
    void *below = mmap((void *)PAGE, PAGE, PROT_READ, ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
    printf("no replacing below it: %s\n", outcome(below, lowest));
    void *fixed = mmap((void *)PAGE, PAGE, PROT_READ, ANONYMOUS | MAP_FIXED, -1, 0);
    printf("fixed below it: %s\n", outcome(fixed, lowest));
    munmap(first, PAGE);
    munmap(again, PAGE);

    // Fill the space with pieces that take no memory, largest first, until none fits.
    char *start = mmap(NULL, PAGE, PROT_NONE, ANONYMOUS | MAP_NORESERVE, -1, 0);
    int refusal = 0;
    for (size_t piece = LARGEST_PIECE; piece >= PAGE; piece /= 2) {
        while (mmap(NULL, piece, PROT_NONE, ANONYMOUS | MAP_NORESERVE, -1, 0) != MAP_FAILED) {
        }
        refusal = errno;
    }
    printf("the last piece refused: %s\n", strerror(refusal));
    fflush(stdout);

    // The mappings between the first piece and the stack: Linux places them there only once
    // there is no room below.
    read_all("/proc/self/maps", maps, sizeof maps);
    char *found = strstr(maps, "[stack]");
    uintptr_t stack = 0;
    if (found) {
        while (found > maps && found[-1] != '\n') found--;
        sscanf(found, "%lx", &stack);
    }
    printf("the stack found: %s\n", stack ? "yes" : "no");
    int between = 0;
    for (char *line = maps; *line;) {
        char *end = strchr(line, '\n');
        if (end) *end = 0;
        uintptr_t from, to;
        if (sscanf(line, "%lx-%lx", &from, &to) == 2 && from > (uintptr_t)start && to <= stack &&
            !strchr(line, '[') && !strchr(line, '/'))
            between++;
        if (!end) break;
        line = end + 1;
    }
    printf("mappings between the first piece and the stack: %s\n", between ? "some" : "none");
    return 0;
}
