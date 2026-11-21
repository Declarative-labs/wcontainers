// adjtimex and clock_adjtime: reading the clock's state, the refusals to set it, its mode checks and
// which clocks adjust.
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/timex.h>
#include <sys/syscall.h>
#include <unistd.h>
static void show(const char *what, long r, struct timex *t) {
    printf("%s: %ld %s modes %x offset %ld freq %ld maxerror %ld esterror %ld status %d constant %ld precision %ld tolerance %ld tick %ld ppsfreq %ld jitter %ld shift %d stabil %ld jitcnt %ld calcnt %ld errcnt %ld stbcnt %ld tai %d usec<1e6 %d\n",
        what, r, r < 0 ? strerror(errno) : "", t->modes, t->offset, t->freq, t->maxerror, t->esterror, t->status, t->constant, t->precision, t->tolerance, t->tick, t->ppsfreq, t->jitter, t->shift, t->stabil, t->jitcnt, t->calcnt, t->errcnt, t->stbcnt, t->tai, t->time.tv_usec < 1000000);
}
int main(void) {
    struct timex t;
    memset(&t, 0, sizeof t); show("adjtimex 0", syscall(SYS_adjtimex, &t), &t);
    memset(&t, 0, sizeof t); t.modes = 0xa001; show("ss read", syscall(SYS_adjtimex, &t), &t);
    memset(&t, 0x55, sizeof t); t.modes = 0x8001; show("singleshot", syscall(SYS_adjtimex, &t), &t);
    memset(&t, 0, sizeof t); t.modes = 0x8000; show("adjtime alone", syscall(SYS_adjtimex, &t), &t);
    memset(&t, 0, sizeof t); t.modes = 0x4000; t.tick = 1; show("tick bad", syscall(SYS_adjtimex, &t), &t);
    memset(&t, 0, sizeof t); t.modes = 0x0100; show("setoffset", syscall(SYS_adjtimex, &t), &t);
    memset(&t, 0, sizeof t); show("clock_adjtime rt", syscall(SYS_clock_adjtime, 0, &t), &t);
    memset(&t, 0, sizeof t); show("clock_adjtime mono", syscall(SYS_clock_adjtime, 1, &t), &t);
    memset(&t, 0, sizeof t); show("clock_adjtime tai", syscall(SYS_clock_adjtime, 11, &t), &t);
    for (int c = 2; c < 26; c++) { char b[32]; sprintf(b, "clock %d", c); memset(&t, 0, sizeof t); long r = syscall(SYS_clock_adjtime, c, &t); printf("%s: %ld %s\n", b, r, r < 0 ? strerror(errno) : ""); }
    memset(&t, 0, sizeof t); show("clock_adjtime 99", syscall(SYS_clock_adjtime, 99, &t), &t);
    memset(&t, 0, sizeof t); show("clock_adjtime cpu", syscall(SYS_clock_adjtime, -2, &t), &t);
    memset(&t, 0, sizeof t); show("clock_adjtime fd 3", syscall(SYS_clock_adjtime, (~3 << 3) | 3, &t), &t);
    show("clock_adjtime null", syscall(SYS_clock_adjtime, 0, 0), &t);
    show("adjtimex null", syscall(SYS_adjtimex, 0), &t);
    memset(&t, 0, sizeof t); t.modes = 0x0002; t.freq = 0x7fffffffffffffffL; show("freq huge", syscall(SYS_adjtimex, &t), &t);
    return 0;
}
