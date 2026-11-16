// A restartable sequence left when its thread is taken off the CPU: a thread spins inside a long
// critical section while another thread wants the one CPU, and lands at the abort address. Run on
// one CPU in the reference.

#define _GNU_SOURCE
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <unistd.h>

#define RSEQ_SIG 0xd428bc00

struct rseq_cs {
    uint32_t version;
    uint32_t flags;
    uint64_t start_ip;
    uint64_t post_commit_offset;
    uint64_t abort_ip;
} __attribute__((aligned(32)));

struct rseq_area {
    uint32_t cpu_id_start;
    uint32_t cpu_id;
    uint64_t rseq_cs;
    uint32_t flags;
    uint32_t node_id;
    uint32_t mm_cid;
    char end[];
} __attribute__((aligned(32)));

// spin_section(&area.rseq_cs, &descriptor, count): arms the section, then counts down inside it;
// 1 if the count ran out, 2 if the thread was sent to the abort address.
long spin_section(uint64_t *armed, struct rseq_cs *cs, long count);
extern char spin_start[], spin_end[], spin_abort[];

__asm__(".text\n"
        ".global spin_section\n"
        "spin_section:\n"
        "    str x1, [x0]\n"
        ".global spin_start\n"
        "spin_start:\n"
        "    subs x2, x2, #1\n"
        "    b.ne spin_start\n"
        "    mov x0, #1\n"
        ".global spin_end\n"
        "spin_end:\n"
        "    ret\n"
        "    .inst 0xd428bc00\n"
        ".global spin_abort\n"
        "spin_abort:\n"
        "    mov x0, #2\n"
        "    ret\n");

static volatile int stop;

static void *compete(void *arg) {
    (void)arg;
    while (!stop)
        __asm__ volatile("" ::: "memory");
    return NULL;
}

static __thread struct rseq_area area;

int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    long registered = syscall(SYS_rseq, &area, 32, 0, RSEQ_SIG);
    printf("rseq: %ld\n", registered);
    struct rseq_cs cs = {
        .start_ip = (uint64_t)spin_start,
        .post_commit_offset = (uint64_t)(spin_end - spin_start),
        .abort_ip = (uint64_t)spin_abort,
    };
    long alone = spin_section(&area.rseq_cs, &cs, 1000);
    printf("alone: %ld, section cleared %d\n", alone, area.rseq_cs == (uint64_t)&cs);
    pthread_t other;
    pthread_create(&other, NULL, compete, NULL);
    int aborted = 0;
    for (int round = 0; round < 20 && !aborted; round++)
        aborted = spin_section(&area.rseq_cs, &cs, 200000000) == 2;
    stop = 1;
    pthread_join(other, NULL);
    printf("left at the abort address while another thread waited: %d, cleared %d\n", aborted,
           area.rseq_cs == 0);
    return 0;
}
