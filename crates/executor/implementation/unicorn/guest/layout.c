// Where execve places a program's parts: the program starts itself many times, each start reports
// the address of its stack, a new mapping, its heap, its vDSO and its own code, and the program
// prints which of them moved between starts. Then the randomization settings it can read, and what
// asking for a layout that is not random answers.

#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/mman.h>
#include <sys/personality.h>
#include <sys/wait.h>
#include <unistd.h>

#define STARTS 32
#define PARTS 5

static const char *names[PARTS] = {"stack", "mapping", "heap", "vdso", "code"};

static void report(int out) {
    int local = 0;
    uintptr_t parts[PARTS];
    parts[0] = (uintptr_t)&local;
    parts[1] = (uintptr_t)mmap(0, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    parts[2] = (uintptr_t)sbrk(0);
    parts[3] = (uintptr_t)getauxval(AT_SYSINFO_EHDR);
    parts[4] = (uintptr_t)&report;
    write(out, parts, sizeof parts);
}

static void show_file(const char *path) {
    char text[64] = {0};
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        printf("%s: errno %d\n", path, errno);
        return;
    }
    ssize_t got = read(fd, text, sizeof text - 1);
    close(fd);
    printf("%s: %s", path, got > 0 ? "read\n" : "empty\n");
    if (strcmp(path, "/proc/sys/kernel/randomize_va_space") == 0) printf("  value %s", text);
}

int main(int argc, char **argv) {
    if (argc > 1) {
        report(atoi(argv[1]));
        return 0;
    }
    uintptr_t seen[STARTS][PARTS];
    for (int start = 0; start < STARTS; start++) {
        int pipe_ends[2];
        pipe(pipe_ends);
        pid_t child = fork();
        if (child == 0) {
            close(pipe_ends[0]);
            char out[16];
            snprintf(out, sizeof out, "%d", pipe_ends[1]);
            char *arguments[] = {"layout", out, 0};
            execv("/proc/self/exe", arguments);
            _exit(1);
        }
        close(pipe_ends[1]);
        read(pipe_ends[0], seen[start], sizeof seen[start]);
        close(pipe_ends[0]);
        waitpid(child, 0, 0);
    }
    for (int part = 0; part < PARTS; part++) {
        uintptr_t differ = 0;
        for (int start = 1; start < STARTS; start++) differ |= seen[start][part] ^ seen[0][part];
        int page_bits = 0;
        for (uintptr_t bits = differ >> 12; bits; bits >>= 1) page_bits += bits & 1;
        printf("%s: %s\n", names[part], page_bits ? "moves by pages" : "fixed");
        if (strcmp(names[part], "stack") == 0)
            printf("  within its page: %s\n", (differ & 0xff0) ? "moves" : "fixed");
    }
    show_file("/proc/sys/kernel/randomize_va_space");
    show_file("/proc/sys/vm/mmap_rnd_bits");
    errno = 0;
    int old = personality(0xffffffff);
    int set = personality(old | ADDR_NO_RANDOMIZE);
    printf("personality(ADDR_NO_RANDOMIZE): %d errno %d\n", set == -1 ? -1 : 0, errno);
    return 0;
}
