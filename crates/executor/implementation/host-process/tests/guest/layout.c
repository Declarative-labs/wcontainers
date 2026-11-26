// Where a process's stack, break, heap and mappings lie and how much memory it is told it has: the same on every
// executor the kernel is given the same ceiling and memory size on.

#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/sysinfo.h>
#include <unistd.h>

int main(int argc, char **argv) {
    struct sysinfo info;
    if (sysinfo(&info) != 0) {
        printf("sysinfo failed\n");
        return 1;
    }
    printf("memory %lu in units of %u, %ld pages\n", info.totalram, info.mem_unit, sysconf(_SC_PHYS_PAGES));
    printf("stack %p, arguments %p\n", (void *)&info, (void *)argv);
    printf("break %p\n", sbrk(0));
    printf("small allocation %p\n", malloc(64));
    printf("large allocation %p\n", malloc(1 << 20));
    void *mapped = mmap(0, 1 << 24, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    printf("mapping %p\n", mapped);
    FILE *maps = fopen("/proc/self/maps", "r");
    if (!maps) {
        printf("no /proc/self/maps\n");
        return 1;
    }
    char line[512];
    while (fgets(line, sizeof line, maps)) {
        fputs(line, stdout);
    }
    fclose(maps);
    return 0;
}
