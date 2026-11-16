// Makes the system call numbered by its first argument (decimal, may be negative, all 64 bits go in
// x8), with all six arguments zero, prints what it returned in decimal and exits with status 0. No C
// library.

#include "bare.h"

void start(long *stack) {
    char **argv = (char **)(stack + 1);
    const char *digit = stack[0] > 1 ? argv[1] : "";
    int negative = *digit == '-';
    unsigned long number = 0;
    for (digit += negative; *digit; digit++) {
        number = number * 10 + (*digit - '0');
    }
    print_number(call(negative ? -number : number, 0, 0, 0));
    exit_group(0);
}
