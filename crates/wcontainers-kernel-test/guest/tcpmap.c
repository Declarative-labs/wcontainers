// Mapping sockets: TCP's read-only mapping with no pages (`tcp_mmap`), every other kind refused.
#define _GNU_SOURCE
#include <errno.h>
#include <netinet/in.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <unistd.h>

static sigjmp_buf back;
static volatile int code;
static void on_bus(int signal, siginfo_t *info, void *context) {
    (void)context;
    code = signal * 100 + info->si_code;
    siglongjmp(back, 1);
}

static void result(const char *what, long r) {
    if (r < 0)
        printf("%s: -1 %s\n", what, strerror(errno));
    else
        printf("%s: %ld\n", what, r);
}

static void mapped(const char *what, void *at) {
    if (at == MAP_FAILED)
        printf("%s: failed %s\n", what, strerror(errno));
    else
        printf("%s: mapped\n", what);
}

static void touch(const char *what, volatile char *at) {
    code = 0;
    if (sigsetjmp(back, 1) == 0)
        (void)*at;
    printf("%s: signal %d\n", what, code);
}

static void maps_line(void *at) {
    FILE *maps = fopen("/proc/self/maps", "r");
    char line[512];
    char start[32];
    snprintf(start, sizeof start, "%lx-", (unsigned long)at);
    while (fgets(line, sizeof line, maps))
        if (strncmp(line, start, strlen(start)) == 0) {
            // The range, permissions, offset and the name with its inode number masked.
            char range[64], perms[8], offset[16], device[16], name[128] = "";
            unsigned long inode;
            sscanf(line, "%63s %7s %15s %15s %lu %127s", range, perms, offset, device, &inode, name);
            char *bracket = strchr(name, '[');
            if (bracket)
                strcpy(bracket, "[N]");
            printf("maps: %s %s inode %d %s\n", perms, offset, inode != 0, name);
        }
    fclose(maps);
}

int main(void) {
    struct sigaction action = {.sa_sigaction = on_bus, .sa_flags = SA_SIGINFO};
    sigaction(SIGBUS, &action, NULL);
    sigaction(SIGSEGV, &action, NULL);
    int kinds[][2] = {{AF_INET, SOCK_STREAM}, {AF_INET6, SOCK_STREAM}, {AF_INET, SOCK_DGRAM},
                      {AF_UNIX, SOCK_STREAM}, {AF_UNIX, SOCK_DGRAM}};
    for (int i = 0; i < 5; i++) {
        int s = socket(kinds[i][0], kinds[i][1], 0);
        printf("kind %d/%d\n", kinds[i][0], kinds[i][1]);
        void *at = mmap(NULL, 8192, PROT_READ, MAP_SHARED, s, 0);
        mapped("shared read", at);
        if (at != MAP_FAILED)
            munmap(at, 8192);
        mapped("shared write", mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, s, 0));
        mapped("private write", mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE, s, 0));
        mapped("exec", mmap(NULL, 4096, PROT_READ | PROT_EXEC, MAP_PRIVATE, s, 0));
        mapped("none", mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE, s, 4096));
        close(s);
    }
    int s = socket(AF_INET, SOCK_STREAM, 0);
    char *at = mmap(NULL, 8192, PROT_READ, MAP_SHARED, s, 4096);
    maps_line(at);
    touch("read", at);
    result("mprotect write", mprotect(at, 4096, PROT_READ | PROT_WRITE));
    result("mprotect exec", mprotect(at, 4096, PROT_READ | PROT_EXEC));
    result("mprotect none", mprotect(at, 4096, PROT_NONE));
    result("mprotect read", mprotect(at, 4096, PROT_READ));
    result("mlock", mlock(at, 8192));
    maps_line(at);
    result("munlock", munlock(at, 8192));
    result("msync", msync(at, 8192, MS_SYNC));
    result("madvise dontneed", madvise(at, 8192, MADV_DONTNEED));
    result("madvise willneed", madvise(at, 8192, MADV_WILLNEED));
    unsigned char vec[2] = {9, 9};
    result("mincore", mincore(at, 8192, vec));
    printf("mincore: %d %d\n", vec[0], vec[1]);
    char *moved = mremap(at, 8192, 16384, MREMAP_MAYMOVE);
    mapped("mremap", moved);
    if (moved != MAP_FAILED) {
        maps_line(moved);
        at = moved;
    }
    result("munmap", munmap(at, 16384));
    mapped("locked", mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_LOCKED, s, 0));
    mapped("populate", mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_POPULATE, s, 0));
    char *private = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, s, 0);
    maps_line(private);
    touch("private read", private);
    result("private mprotect write", mprotect(private, 4096, PROT_READ | PROT_WRITE));
    return 0;
}
