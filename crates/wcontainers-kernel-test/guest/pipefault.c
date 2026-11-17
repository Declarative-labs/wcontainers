// Pipe writes whose buffer faults: what the reader and poll see after, and the room the pipe keeps.

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static char big[1 << 17];

static void round_(int faults, int then_byte) {
    int e[2];
    pipe2(e, O_NONBLOCK);
    for (int i = 0; i < faults; i++) write(e[1], NULL, 0x1000);
    if (then_byte) write(e[1], "x", 1);
    for (int i = 0; i < faults; i++) write(e[1], NULL, 0x1000);
    struct pollfd f[2] = {{e[0], POLLIN, 0}, {e[1], POLLOUT, 0}};
    poll(f, 2, 0);
    long w = write(e[1], big, sizeof big);
    printf("faults %d byte %d: read end %x write end %x then wrote %ld errno %d", faults, then_byte, f[0].revents, f[1].revents, w, w < 0 ? errno : 0);
    long r = read(e[0], big, 1);
    printf(" first read %ld\n", r);
    close(e[0]); close(e[1]);
}

int main(void) {
    memset(big, 'a', sizeof big);
    int e[2];
    pipe2(e, O_NONBLOCK);
    long r = write(e[1], NULL, 0x9000);
    struct pollfd f[2] = {{e[0], POLLIN, 0}, {e[1], POLLOUT, 0}};
    poll(f, 2, 0);
    printf("write null 0x9000 = %ld errno %d: read end %x write end %x\n", r, r < 0 ? errno : 0, f[0].revents, f[1].revents);
    close(e[0]);
    close(e[1]);
    round_(1, 0);
    round_(16, 0);
    round_(20, 0);
    round_(1, 1);
    round_(8, 1);
    round_(16, 1);
    return 0;
}
