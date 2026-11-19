// getppid as many times as the first argument says, then one line of output: for counting the virtual
// machine's exits while a program makes system calls.

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv) {
    long calls = argc > 1 ? atol(argv[1]) : 0;
    long parents = 0;
    for (long i = 0; i < calls; i++) {
        parents += getppid();
    }
    printf("%ld calls, parent %ld\n", calls, calls ? parents / calls : -1);
    return 0;
}
