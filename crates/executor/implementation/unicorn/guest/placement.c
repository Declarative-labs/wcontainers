// Where mappings go relative to each other, to the stack's top and the vDSO, to holes left by
// unmapping and to hints, and which start on a PMD's boundary, with the layout not random: what
// does not depend on the address space's width.
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/personality.h>
#include <unistd.h>

#define PAGE 4096
#define PMD (2 << 20)
#define ANONYMOUS (MAP_PRIVATE | MAP_ANONYMOUS)

extern char end;

static uintptr_t stack_start, stack_end, special_low = UINTPTR_MAX, special_high, heap_start;

static void read_maps(void) {
    FILE *maps = fopen("/proc/self/maps", "r");
    char line[512];
    while (fgets(line, sizeof line, maps)) {
        uintptr_t start, stop;
        sscanf(line, "%lx-%lx", &start, &stop);
        if (strstr(line, "[stack]")) {
            stack_start = start;
            stack_end = stop;
        }
        if (strstr(line, "[vdso]") || strstr(line, "[vvar")) {
            if (start < special_low) special_low = start;
            if (stop > special_high) special_high = stop;
        }
        if (strstr(line, "[heap]")) heap_start = start;
    }
    fclose(maps);
}

static char *map(void *hint, size_t length, int flags) {
    return mmap(hint, length, PROT_READ | PROT_WRITE, flags, -1, 0);
}

static void boundary(const char *name, const char *at) {
    printf("%s past a PMD's boundary: %#lx\n", name, (uintptr_t)at & (PMD - 1));
}

int main(int argc, char **argv) {
    if (argc < 2) {
        personality(ADDR_NO_RANDOMIZE);
        execl(argv[0], argv[0], "placed", (char *)NULL);
        return 1;
    }
    sbrk(PAGE);
    read_maps();
    printf("stack's top above the specials' top: %#lx\n", stack_end - special_high);
    printf("stack's start below its top: %s\n", stack_end - stack_start <= 33 * PAGE ? "near" : "far");
    uintptr_t program_end = ((uintptr_t)&end + PAGE - 1) & ~(uintptr_t)(PAGE - 1);
    printf("heap from the program's end: %#lx\n", heap_start - program_end);
    char *a = map(NULL, 3 * PAGE, ANONYMOUS);
    char *b = map(NULL, 5 * PAGE, ANONYMOUS);
    char *c = map(NULL, PAGE, ANONYMOUS);
    printf("first mapping ends at the specials' start: %s\n",
           (uintptr_t)a + 3 * PAGE == special_low ? "yes" : "no");
    printf("a - b %#lx, b - c %#lx\n", (uintptr_t)(a - b), (uintptr_t)(b - c));
    munmap(b, 5 * PAGE);
    char *d = map(NULL, 2 * PAGE, ANONYMOUS);
    printf("in the hole: d - b %#lx\n", (uintptr_t)(d - b));
    char *e = map(NULL, 1 << 20, ANONYMOUS);
    printf("c - e %#lx\n", (uintptr_t)(c - e));
    char *hint = c - (64 << 20);
    char *f = map(hint, PAGE, ANONYMOUS);
    printf("hint below: %s\n", f == hint ? "taken" : "moved");
    char *g = map(a, PAGE, ANONYMOUS);
    printf("hint on a mapping: e - g %#lx\n", (uintptr_t)(e - g));
    char *big = map(NULL, 1 << 30, ANONYMOUS);
    printf("e - big %#lx\n", (uintptr_t)(e - big));
    boundary("1 GiB", big);
    char *odd = map(NULL, 3 << 20, ANONYMOUS);
    printf("big - odd %#lx\n", (uintptr_t)(big - odd));
    boundary("3 MiB", odd);
    char *even = map(NULL, 4 << 20, ANONYMOUS);
    printf("odd - even %#lx\n", (uintptr_t)(odd - even));
    boundary("4 MiB", even);
    char *one = map(NULL, PMD, ANONYMOUS);
    printf("even - one %#lx\n", (uintptr_t)(even - one));
    boundary("one PMD", one);
    char *shared = map(NULL, PMD, MAP_SHARED | MAP_ANONYMOUS);
    printf("one - shared %#lx\n", (uintptr_t)(one - shared));
    char *hinted = map(a, PMD, ANONYMOUS);
    printf("shared - hinted on a mapping %#lx\n", (uintptr_t)(shared - hinted));
    char *grow = map(NULL, 16 * PAGE, ANONYMOUS | MAP_GROWSDOWN);
    printf("hinted - grow %#lx\n", (uintptr_t)(hinted - grow));
    char *moved = mremap(d, 2 * PAGE, 64 * PAGE, MREMAP_MAYMOVE);
    printf("grow - moved %#lx\n", (uintptr_t)(grow - moved));
    char *moved_whole = mremap(e, 1 << 20, PMD, MREMAP_MAYMOVE);
    printf("moved - moved to a PMD %#lx\n", (uintptr_t)(moved - moved_whole));
    boundary("moved to a PMD", moved_whole);
    char *after = map(NULL, PAGE, ANONYMOUS);
    printf("moved to a PMD - after %#lx\n", (uintptr_t)(moved_whole - after));
    return 0;
}
