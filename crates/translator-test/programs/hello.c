// Hello world through the C library: formatted printing, buffered output flushed at exit, and an exit
// status. Output and exit status are checked against real Linux.

#include <stdio.h>

int main(int argc, char **argv) {
    printf("hello, world\n");
    printf("%d %s %x %c %ld\n", argc, argv[0], 0xbeefu, '!', -1234567890123L);
    return 3;
}
