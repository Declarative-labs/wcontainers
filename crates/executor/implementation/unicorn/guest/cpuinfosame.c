// /proc/cpuinfo on a machine of several processors: one block for each processor online, and the
// same Features line in each. Exits 0 if so, 1 if the blocks are not one per processor, 2 if a
// Features line differs from the first, 3 if the file cannot be read.
#include <stdio.h>
#include <string.h>
#include <unistd.h>

int main(void) {
    FILE *file = fopen("/proc/cpuinfo", "r");
    if (!file)
        return 3;
    char line[4096], first[4096] = {0};
    long blocks = 0, features = 0;
    int differs = 0;
    while (fgets(line, sizeof line, file)) {
        if (strncmp(line, "processor\t", 10) == 0)
            blocks++;
        if (strncmp(line, "Features\t", 9) == 0) {
            if (features++ == 0)
                strcpy(first, line);
            else if (strcmp(first, line) != 0)
                differs = 1;
        }
    }
    fclose(file);
    long online = sysconf(_SC_NPROCESSORS_ONLN);
    if (blocks != online || features != online)
        return 1;
    return differs ? 2 : 0;
}
