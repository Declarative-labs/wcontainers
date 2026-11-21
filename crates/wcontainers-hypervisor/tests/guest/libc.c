// The C library's start, formatted output of integers, strings and floats, parsing, sorting through a function
// pointer and string functions: a static musl program for the exits test.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int compare(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return (x > y) - (x < y);
}

int main(int argc, char **argv) {
    double values[64];
    unsigned state = 12345;
    for (int i = 0; i < 64; i++) {
        state = state * 1103515245 + 12345;
        values[i] = (double)(state >> 8) / 7.0 - 1000000.0;
    }
    qsort(values, 64, sizeof values[0], compare);
    printf("%d %s %.6f %g %e\n", argc, argv[0], values[0], values[31], values[63]);

    char text[256];
    int length = snprintf(text, sizeof text, "%x %o %ld %-8s|%+.3Lf", 48879, 511, -1234567890123L, "left",
                          (long double)values[17] / 3);
    printf("%d [%s] %zu\n", length, text, strlen(text));

    char *end;
    double parsed = strtod("  -12345.678e-3xyz", &end);
    long number = strtol("0x7fffABCD", NULL, 16);
    printf("%.9f [%s] %ld %s\n", parsed, end, number, strstr(text, "left") ? "found" : "missing");

    char copy[200];
    memset(copy, 'a', sizeof copy);
    memcpy(copy + 3, text, (size_t)length + 1);
    printf("%d %s\n", memcmp(copy, text, 10), strchr(copy, '|'));
    return (int)(number & 0x3f);
}
