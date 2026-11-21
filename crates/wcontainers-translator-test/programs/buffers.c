// write and writev with buffers the program may not read: address zero, a buffer running past the end of
// the blob, vectors on a forbidden page, and invalid vector counts and lengths. Each result is printed.
// Output and exit status are checked against real Linux.

typedef unsigned long u64;
typedef long i64;

#include "syscall.h"

struct vector {
    const void *base;
    u64 length;
};

// The first byte after the blob, from the linker script.
extern char __blob_end[];

static void result(i64 value) {
    char line[20];
    int at = 0;
    u64 magnitude = value < 0 ? -(u64)value : (u64)value;
    if (value < 0) line[at++] = '-';
    char digits[20];
    int count = 0;
    do {
        digits[count++] = '0' + magnitude % 10;
        magnitude /= 10;
    } while (magnitude);
    while (count) line[at++] = digits[--count];
    line[at++] = '\n';
    syscall3(SYS_write, 1, (i64)line, at);
}

static const char message[] = "abc";

void _start(void) {
    const char *after = (const char *)(((u64)__blob_end + 4095) & ~4095ul);
    // write
    result(syscall3(SYS_write, 1, 0, 5));
    result(syscall3(SYS_write, 1, 0, 0));
    result(syscall3(SYS_write, 1, (i64)(after - 3), 6));
    result(syscall3(SYS_write, 1, (i64)(after - 3), 3));
    result(syscall3(SYS_write, 1, (i64)after, 1));
    result(syscall3(SYS_write, 1, (i64)message, -1));
    result(syscall3(SYS_write, 7, (i64)message, 3));
    // writev
    struct vector one[2] = {{message, 3}, {0, 3}};
    result(syscall3(SYS_writev, 1, (i64)one, 2));
    struct vector two[2] = {{message, 3}, {after - 1, 2}};
    result(syscall3(SYS_writev, 1, (i64)two, 2));
    struct vector three[2] = {{message, 3}, {message, -1ul}};
    result(syscall3(SYS_writev, 1, (i64)three, 2));
    struct vector four[2] = {{message, 3}, {0, 0}};
    result(syscall3(SYS_writev, 1, (i64)four, 2));
    result(syscall3(SYS_writev, 1, 0, 1));
    result(syscall3(SYS_writev, 1, (i64)(after - 8), 1));
    result(syscall3(SYS_writev, 1, (i64)four, 1025));
    result(syscall3(SYS_writev, 1, (i64)four, 0));
    result(syscall3(SYS_writev, 1, (i64)four, 0x100000001));
    syscall3(SYS_exit_group, 5, 0, 0);
    for (;;) {
    }
}
