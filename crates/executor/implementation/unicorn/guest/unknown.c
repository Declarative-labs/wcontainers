// fcntl commands and prctl options Linux does not know, or has only for other architectures, which
// it answers EINVAL; with an argument "unbuilt", ones Linux has that are not built here.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/prctl.h>
#include <unistd.h>

static void p(const char *what, long r) { printf("%s = %ld errno %d\n", what, r, r < 0 ? errno : 0); }

int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int fd = open("/dev/shm/f", O_CREAT | O_RDWR, 0600);
    if (argc > 1) {
        p("F_GETDELEG", fcntl(fd, 1039, 0L));
        p("PR_GET_TIMING", prctl(PR_GET_TIMING, 0, 0, 0, 0));
        return 0;
    }
    p("fcntl 20", fcntl(fd, 20, 0L));
    p("fcntl 1029", fcntl(fd, 1029, 0L));
    p("fcntl 1041", fcntl(fd, 1041, 0L));
    p("fcntl 99999", fcntl(fd, 99999, 0L));
    p("prctl 0", prctl(0, 0, 0, 0, 0));
    p("prctl 80", prctl(80, 0, 0, 0, 0));
    p("prctl 1000", prctl(1000, 0, 0, 0, 0));
    p("PR_GET_UNALIGN", prctl(PR_GET_UNALIGN, 0, 0, 0, 0));
    p("PR_GET_FPEMU", prctl(PR_GET_FPEMU, 0, 0, 0, 0));
    p("PR_GET_ENDIAN", prctl(PR_GET_ENDIAN, 0, 0, 0, 0));
    p("PR_GET_FP_MODE", prctl(PR_GET_FP_MODE, 0, 0, 0, 0));
    return 0;
}
