// The memory calls the stand-in kernel answers: brk growing and shrinking, private anonymous mmap placed by
// the kernel and fixed, munmap, and their errors. Only what does not depend on where memory lands is
// printed; the last store, to an unmapped page, faults. Output and exit status are checked against real
// Linux. The break never goes below where it started: on Linux the pages below belong to the reference.

typedef unsigned long u64;
typedef long i64;

#include "syscall.h"

#define PAGE 4096
#define PROT_NONE 0
#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_PRIVATE 0x02
#define MAP_FIXED 0x10
#define MAP_ANONYMOUS 0x20
#define MAP_FIXED_NOREPLACE 0x100000

static void result(i64 value) {
    char line[21];
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

static i64 map(u64 address, u64 length, i64 protection, i64 flags, i64 offset) {
    return syscall6(SYS_mmap, (i64)address, (i64)length, protection, flags, -1, offset);
}

// Whether every byte of a range is zero.
static i64 zero(const volatile char *at, u64 length) {
    for (u64 i = 0; i < length; i++) {
        if (at[i]) return 0;
    }
    return 1;
}

static void fill(volatile char *at, u64 length) {
    for (u64 i = 0; i < length; i++) at[i] = (char)(i | 1);
}

void _start(void) {
    const i64 private = MAP_PRIVATE | MAP_ANONYMOUS;
    const i64 rw = PROT_READ | PROT_WRITE;
    // brk: grows, is written, shrinks, grows again with zero pages.
    u64 start = (u64)syscall3(SYS_brk, 0, 0, 0);
    result(syscall3(SYS_brk, (i64)(start + 2 * PAGE + 100), 0, 0) == (i64)(start + 2 * PAGE + 100));
    fill((volatile char *)start, 2 * PAGE + 100);
    result(syscall3(SYS_brk, (i64)(start + 100), 0, 0) == (i64)(start + 100));
    result(syscall3(SYS_brk, (i64)(start + 2 * PAGE + 100), 0, 0) == (i64)(start + 2 * PAGE + 100));
    u64 kept = (start + 100 + PAGE - 1) & ~(u64)(PAGE - 1);
    result(zero((volatile char *)kept, start + 2 * PAGE + 100 - kept));
    result(syscall3(SYS_brk, (i64)start, 0, 0) == (i64)start);
    // mmap placed by the kernel: page aligned, zero, writable; then unmapped.
    i64 p = map(0, 3 * PAGE, rw, private, 0);
    result(p > 0 && p % PAGE == 0);
    result(zero((volatile char *)p, 3 * PAGE));
    fill((volatile char *)p, 3 * PAGE);
    // A fixed mapping over the middle page replaces it with zero.
    result(map(p + PAGE, 1, rw, private | MAP_FIXED, 0) == p + PAGE);
    result(zero((volatile char *)(p + PAGE), PAGE));
    result(((volatile char *)p)[0] == 1 && ((volatile char *)p)[2 * PAGE] == 1);
    // MAP_FIXED_NOREPLACE fails over a mapping and maps where nothing is.
    result(map(p, PAGE, rw, private | MAP_FIXED_NOREPLACE, 0));
    result(syscall3(SYS_munmap, p + 2 * PAGE, PAGE, 0));
    result(map(p + 2 * PAGE, PAGE, rw, private | MAP_FIXED_NOREPLACE, 0) == p + 2 * PAGE);
    // A page that allows nothing, and one unmapped, cannot be written out.
    result(map(p, PAGE, PROT_NONE, private | MAP_FIXED, 0) == p);
    result(syscall3(SYS_write, 1, p, 1));
    // Errors.
    result(map(0, 0, rw, private, 0));
    result(map(0, PAGE, rw, private, 1));
    result(map(p + 1, PAGE, rw, private | MAP_FIXED, 0));
    result(map(0, -1ul, rw, private, 0));
    result(syscall3(SYS_munmap, p + 1, PAGE, 0));
    result(syscall3(SYS_munmap, p, 0, 0));
    // Unmapping what is not mapped is no error.
    result(syscall3(SYS_munmap, p, 3 * PAGE, 0));
    result(syscall3(SYS_munmap, p, 3 * PAGE, 0));
    result(syscall3(SYS_write, 1, p + PAGE, 1));
    // A store to an unmapped page faults.
    *(volatile char *)(p + PAGE) = 1;
    syscall3(SYS_exit_group, 5, 0, 0);
    for (;;) {
    }
}
