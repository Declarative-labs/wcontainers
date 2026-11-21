// prctl(PR_SET_MM): the map's size, a whole map put back (checked, then shown by cmdline, environ
// and auxv, and taken by brk), and the forms that need powers a default container lacks.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <unistd.h>

static char data_page[8192] __attribute__((aligned(4096)));
static char strings[] = "one\0two\0HOME=/x\0";
static unsigned long vector[64];

static void p(const char *what, long r) { printf("%s: %ld %s\n", what, r, r < 0 ? strerror(errno) : ""); }

static void show(const char *path) {
    char buffer[512];
    int fd = open(path, O_RDONLY);
    long n = read(fd, buffer, sizeof buffer);
    close(fd);
    printf("%s: %ld bytes:", path, n);
    for (long i = 0; i < n && i < 48; i++) printf(" %02x", (unsigned char)buffer[i]);
    printf("\n");
}

static struct prctl_mm_map layout(void) {
    char *heap = sbrk(0);
    struct prctl_mm_map map = {
        .start_code = (uint64_t)(uintptr_t)&layout,
        .end_code = (uint64_t)(uintptr_t)&layout + 0x1000,
        .start_data = (uint64_t)(uintptr_t)data_page,
        .end_data = (uint64_t)(uintptr_t)data_page + 4096,
        .start_brk = (uint64_t)(uintptr_t)heap,
        .brk = (uint64_t)(uintptr_t)heap,
        .start_stack = (uint64_t)(uintptr_t)&heap,
        .arg_start = (uint64_t)(uintptr_t)strings,
        .arg_end = (uint64_t)(uintptr_t)strings + 8,
        .env_start = (uint64_t)(uintptr_t)strings + 8,
        .env_end = (uint64_t)(uintptr_t)strings + 16,
        .exe_fd = (uint32_t)-1,
    };
    return map;
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    unsigned int size = 0;
    p("map size", prctl(PR_SET_MM, PR_SET_MM_MAP_SIZE, &size, 0, 0));
    printf("size %u\n", size);
    p("map size bad address", prctl(PR_SET_MM, PR_SET_MM_MAP_SIZE, 8, 0, 0));
    struct prctl_mm_map map = layout();
    p("map wrong size", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map - 8, 0));
    p("map bad address", prctl(PR_SET_MM, PR_SET_MM_MAP, 8, sizeof map, 0));
    p("map fifth argument", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 1));
    map.start_code = 4096;
    p("map below the lowest address", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    map = layout();
    map.end_code = map.start_code;
    p("map code out of order", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    map = layout();
    map.arg_end = map.arg_start - 1;
    p("map arguments out of order", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    map = layout();
    map.auxv_size = 16;
    p("map auxv size without vector", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    map.auxv = vector;
    map.auxv_size = 408;
    p("map auxv too large", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    map.auxv = (void *)8;
    map.auxv_size = 16;
    p("map auxv bad address", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    map = layout();
    map.exe_fd = 0;
    p("map program file", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    struct rlimit saved, small = {4096, RLIM_INFINITY};
    getrlimit(RLIMIT_DATA, &saved);
    setrlimit(RLIMIT_DATA, &small);
    map = layout();
    map.brk = map.start_brk + 8192;
    p("map over the data limit", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    setrlimit(RLIMIT_DATA, &saved);
    map = layout();
    vector[0] = 6;
    vector[1] = 4096;
    vector[2] = 0;
    vector[3] = 0;
    vector[4] = 7;
    map.auxv = vector;
    map.auxv_size = 6 * sizeof(unsigned long);
    p("map", prctl(PR_SET_MM, PR_SET_MM_MAP, &map, sizeof map, 0));
    show("/proc/self/cmdline");
    show("/proc/self/environ");
    show("/proc/self/auxv");
    char *before = sbrk(0);
    p("sbrk a page", sbrk(4096) == before ? 0 : -1);
    p("start code", prctl(PR_SET_MM, PR_SET_MM_START_CODE, map.start_code, 0, 0));
    p("start code fourth argument", prctl(PR_SET_MM, PR_SET_MM_START_CODE, map.start_code, 1, 0));
    p("auxv", prctl(PR_SET_MM, PR_SET_MM_AUXV, vector, 16, 0));
    p("program file", prctl(PR_SET_MM, PR_SET_MM_EXE_FILE, 0, 0, 0));
    return 0;
}
