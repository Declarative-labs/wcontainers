/* Asks for memory as programs do and checks what it holds: a private mapping larger than the test machine's
   memory, read and written at a few pages; mappings of several sizes, each touched at its ends and unmapped;
   a mapping written at every page, unmapped and mapped again until more than the machine's memory has been
   written in all; contents kept while their pages allow nothing; and a shared mapping a child writes. With an
   argument it writes every page of a mapping of that many MiB instead. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

#define MIB ((size_t)1 << 20)

static char *map(size_t length, int flags) {
    char *p = mmap(0, length, PROT_READ | PROT_WRITE, flags | MAP_ANONYMOUS, -1, 0);
    return p == MAP_FAILED ? 0 : p;
}

static int write_every_page(size_t length, size_t page) {
    char *p = map(length, MAP_PRIVATE);
    if (!p) {
        printf("mmap failed\n");
        return 1;
    }
    for (size_t at = 0; at < length; at += page) p[at] = 1;
    printf("written at every page\n");
    return 0;
}

int main(int argc, char **argv) {
    size_t page = (size_t)sysconf(_SC_PAGESIZE);
    if (argc > 1) return write_every_page((size_t)atol(argv[1]) * MIB, page);

    size_t large = 1536 * MIB;
    char *p = map(large, MAP_PRIVATE);
    if (!p) {
        printf("large: mmap failed\n");
        return 1;
    }
    printf("large: read before written %d %d\n", p[large / 2], p[large - 1]);
    p[0] = 1;
    p[large - 1] = 2;
    printf("large: written %d %d\n", p[0], p[large - 1]);
    munmap(p, large);

    const size_t sizes[] = {64, 512, 700, 900};
    for (size_t i = 0; i < sizeof sizes / sizeof *sizes; i++) {
        size_t length = sizes[i] * MIB;
        p = map(length, MAP_PRIVATE);
        if (!p) {
            printf("%zu MiB: mmap failed\n", sizes[i]);
            continue;
        }
        p[0] = 1;
        p[length - 1] = 2;
        printf("%zu MiB: written %d %d\n", sizes[i], p[0], p[length - 1]);
        munmap(p, length);
    }

    size_t filled = 64 * MIB;
    unsigned long sum = 0;
    for (int round = 0; round < 24; round++) {
        p = map(filled, MAP_PRIVATE);
        if (!p) {
            printf("round %d: mmap failed\n", round);
            return 1;
        }
        for (size_t at = 0; at < filled; at += page) {
            sum += (unsigned char)p[at];
            p[at] = (char)(round + 1);
        }
        sum += (unsigned char)p[filled - page];
        munmap(p, filled);
    }
    printf("written at every page, 24 times: %lu\n", sum);

    p = map(4 * page, MAP_PRIVATE);
    strcpy(p + page, "kept");
    mprotect(p, 4 * page, PROT_NONE);
    mprotect(p, 4 * page, PROT_READ);
    printf("allowed nothing, then read: %s\n", p + page);
    munmap(p, 4 * page);

    size_t shared = 256 * MIB;
    p = map(shared, MAP_SHARED);
    pid_t child = fork();
    if (child == 0) {
        p[shared - 1] = 9;
        strcpy(p + shared / 2, "from the child");
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    printf("shared: %d %s %d\n", p[shared - 1], p + shared / 2, p[0]);
    munmap(p, shared);
    return 0;
}
