// The signal fields of /proc/self/status and /proc/self/stat after blocking, ignoring and catching
// signals.

#define _GNU_SOURCE
#include <signal.h>
#include <stdio.h>
#include <string.h>

static void handler(int signal) { (void)signal; }

static void show(const char *when) {
    char line[512];
    FILE *status = fopen("/proc/self/status", "r");
    while (fgets(line, sizeof line, status)) {
        if (strncmp(line, "SigBlk", 6) == 0 || strncmp(line, "SigIgn", 6) == 0 || strncmp(line, "SigCgt", 6) == 0) {
            printf("%s: %s", when, line);
        }
    }
    fclose(status);
    FILE *stat = fopen("/proc/self/stat", "r");
    fgets(line, sizeof line, stat);
    fclose(stat);
    // The fields after the name; 32 to 34 are blocked, ignored and caught.
    char *at = strrchr(line, ')') + 2;
    int field = 3;
    for (char *word = strtok(at, " "); word; word = strtok(0, " "), field++) {
        if (field >= 32 && field <= 34) {
            printf("%s: stat %d %s\n", when, field, word);
        }
    }
}

int main(void) {
    show("start");
    sigset_t set;
    sigemptyset(&set);
    sigaddset(&set, SIGUSR1);
    sigaddset(&set, SIGRTMIN + 3);
    sigaddset(&set, SIGKILL);
    sigprocmask(SIG_BLOCK, &set, 0);
    signal(SIGHUP, SIG_IGN);
    signal(SIGPIPE, SIG_IGN);
    signal(SIGUSR2, handler);
    signal(SIGTERM, handler);
    signal(SIGRTMAX, handler);
    show("changed");
    signal(SIGHUP, SIG_DFL);
    sigprocmask(SIG_UNBLOCK, &set, 0);
    show("restored");
    return 0;
}
