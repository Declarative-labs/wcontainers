// Given a mask in hexadecimal, sets it as its own affinity and runs itself again without it, as a
// runtime does before the container's program. Then exits with what it sees of its affinity: the
// set of processors 0 to 3 in bits 0 to 3; bit 4 if a child has the same set; bit 5 if
// sched_getcpu names a processor of the set; bit 6 if setting processors 0 and 2 reads back as
// exactly those; bit 7 if a mask of no processor is refused with EINVAL. Exits with 255 if a call
// fails.
#define _GNU_SOURCE
#include <errno.h>
#include <sched.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

// The processors that the code's bits may name.
#define SHOWN 4

static int shown(const cpu_set_t *set) {
    int bits = 0;
    for (int processor = 0; processor < SHOWN; processor++)
        if (CPU_ISSET(processor, set)) bits |= 1 << processor;
    return bits;
}

int main(int argc, char **argv) {
    cpu_set_t set;
    if (argc > 1) {
        unsigned long mask = strtoul(argv[1], NULL, 16);
        CPU_ZERO(&set);
        for (int processor = 0; processor < (int)(8 * sizeof mask); processor++)
            if (mask >> processor & 1) CPU_SET(processor, &set);
        if (sched_setaffinity(0, sizeof set, &set) != 0) return 255;
        execl(argv[0], argv[0], (char *)NULL);
        return 255;
    }
    if (sched_getaffinity(0, sizeof set, &set) != 0) return 255;
    int code = shown(&set);
    int count = CPU_COUNT(&set);
    pid_t child = fork();
    if (child == 0) {
        cpu_set_t own;
        if (sched_getaffinity(0, sizeof own, &own) != 0) _exit(255);
        _exit(CPU_EQUAL(&own, &set) ? 0 : 1);
    }
    int status;
    if (child < 0 || waitpid(child, &status, 0) != child) return 255;
    if (WIFEXITED(status) && WEXITSTATUS(status) == 0) code |= 1 << 4;
    int processor = sched_getcpu();
    if (processor < 0) return 255;
    if (CPU_ISSET(processor, &set) && count == CPU_COUNT(&set)) code |= 1 << 5;
    cpu_set_t asked;
    CPU_ZERO(&asked);
    CPU_SET(0, &asked);
    CPU_SET(2, &asked);
    if (sched_setaffinity(0, sizeof asked, &asked) != 0) return 255;
    if (sched_getaffinity(0, sizeof set, &set) != 0) return 255;
    if (CPU_EQUAL(&set, &asked)) code |= 1 << 6;
    CPU_ZERO(&asked);
    if (sched_setaffinity(0, sizeof asked, &asked) == -1 && errno == EINVAL) code |= 1 << 7;
    return code;
}
