// A canonical terminal in EXTPROC mode reads its bytes as they come, past its line ends, and
// then counts what is left and lets its master write again.
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int master = posix_openpt(O_RDWR);
    grantpt(master);
    unlockpt(master);
    int slave = open(ptsname(master), O_RDWR);
    struct termios io;
    tcgetattr(master, &io);
    io.c_lflag = EXTPROC | ICANON;
    printf("set: %d\n", tcsetattr(master, TCSANOW, &io));
    printf("write: %zd\n", write(master, "ab\ncd", 5));
    char buffer[16];
    printf("read: %zd\n", read(slave, buffer, sizeof buffer));
    int queued = -1;
    printf("fionread: %d", ioctl(slave, FIONREAD, &queued));
    printf(" %d\n", queued);
    printf("write again: %zd\n", write(master, "ef\n", 3));
    printf("read again: %zd\n", read(slave, buffer, sizeof buffer));
    return 0;
}
