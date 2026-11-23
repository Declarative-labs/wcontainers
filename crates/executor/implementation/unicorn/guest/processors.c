// Exits with how many processors it is told are online, if sched_getaffinity, the cpu lines of
// /proc/stat and the processor lines of /proc/cpuinfo all say the same; else with 255.
#define _GNU_SOURCE
#include <sched.h>
#include <stdio.h>
#include <string.h>

// The lines of `path` that start with `prefix`, or -1 if it cannot be read.
static int lines(const char *path, const char *prefix) {
    FILE *file = fopen(path, "r");
    char line[256];
    int count = 0;
    if (!file) return -1;
    while (fgets(line, sizeof line, file))
        if (strncmp(line, prefix, strlen(prefix)) == 0) count++;
    fclose(file);
    return count;
}

int main(void) {
    cpu_set_t set;
    if (sched_getaffinity(0, sizeof set, &set) != 0) return 255;
    int affinity = CPU_COUNT(&set);
    // The first cpu line is every processor's sum.
    int stat = lines("/proc/stat", "cpu") - 1;
    int info = lines("/proc/cpuinfo", "processor");
    if (affinity != stat || stat != info) return 255;
    return affinity;
}
