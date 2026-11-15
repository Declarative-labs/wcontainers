// Runs write with bad and edge-case buffers, counts and descriptors, printing a label and the result of
// each. No C library.
//
// The end of the program's last segment is the first unmapped address after it, so buffers placed just
// before it run off into unmapped memory at a known point.

#include "bare.h"

extern char _end[];

// Makes sure the last writable pages exist and are the program's to fill.
static char area[3 * 4096] __attribute__((aligned(4096), used));

#define label(name) print("\n" name " ")

void start(long *stack) {
    (void)stack;
    unsigned long limit = ((unsigned long)_end + 4095) & ~4095ul;
    // The last two pages hold a known pattern.
    for (unsigned long at = limit - 8192; at < limit; at++) {
        *(volatile char *)at = 'A' + at % 26;
    }
    const long top = 1l << 48;

    label("inside");
    print_number(call(SYS_write, 1, limit - 100, 100));
    label("one-past-end");
    print_number(call(SYS_write, 1, limit - 100, 101));
    // A count that is not a whole number of pages makes Linux's pipe write depend on whether the reader
    // has drained the pipe yet, so faults part way through use whole pages.
    label("second-page-runs-off");
    print_number(call(SYS_write, 1, limit - 4196, 2 * 4096));
    label("huge-count");
    print_number(call(SYS_write, 1, limit - 4196, 1l << 40));
    label("count-minus-one");
    print_number(call(SYS_write, 1, limit - 100, -1));
    label("end-at-address-limit");
    print_number(call(SYS_write, 1, limit - 4196, top - (long)(limit - 4196)));
    label("end-past-address-limit");
    print_number(call(SYS_write, 1, limit - 4196, top - (long)(limit - 4196) + 1));
    label("zero-at-null");
    print_number(call(SYS_write, 1, 0, 0));
    label("zero-at-address-limit");
    print_number(call(SYS_write, 1, top, 0));
    label("zero-past-address-limit");
    print_number(call(SYS_write, 1, top + 1, 0));
    label("unmapped");
    print_number(call(SYS_write, 2, limit, 10));
    label("stdin");
    print_number(call(SYS_write, 0, (long)"x", 1));
    label("stdin-unmapped");
    print_number(call(SYS_write, 0, limit, 10));
    label("stdin-huge-count");
    print_number(call(SYS_write, 0, limit, 1l << 40));
    label("stdin-count-minus-one");
    print_number(call(SYS_write, 0, limit, -1));
    label("not-open");
    print_number(call(SYS_write, 9, (long)"x", 1));
    label("fd-high-bits");
    print_number(call(SYS_write, (1l << 32) | 1, (long)"x", 1));
    label("number-high-bits");
    print_number(call((1l << 32) | SYS_write, 1, (long)"hi", 2));
    exit_group(0);
}
