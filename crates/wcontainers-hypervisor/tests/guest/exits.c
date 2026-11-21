// Integer, float and vector work, calls through pointers and recursion, each result written as hex, with no C
// library. The exits test runs it on the native CPU side and on the WebAssembly one and compares every exit.

#include <stdint.h>
#include <sys/syscall.h>

static long call3(long number, long a, long b, long c) {
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x8), "r"(x1), "r"(x2) : "memory");
    return x0;
}

static void put(uint64_t value) {
    char text[17];
    for (int i = 15; i >= 0; i--) {
        text[i] = "0123456789abcdef"[value & 15];
        value >>= 4;
    }
    text[16] = '\n';
    call3(SYS_write, 1, (long)text, sizeof text);
}

static uint64_t fibonacci(uint64_t n) { return n < 2 ? n : fibonacci(n - 1) + fibonacci(n - 2); }

static uint64_t twice(uint64_t x) { return x * 2; }
static uint64_t square(uint64_t x) { return x * x; }
static uint64_t (*const steps[])(uint64_t) = {twice, square};

static uint32_t numbers[1024];
static float values[1024];

void _start(void) {
    uint64_t state = 0x9e3779b97f4a7c15;
    for (int i = 0; i < 1024; i++) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        numbers[i] = (uint32_t)state;
        values[i] = (float)(state >> 40) / 3.0f;
    }

    uint64_t sum = 0;
    for (int i = 0; i < 1024; i++) {
        sum += numbers[i] * 3u + (numbers[i] >> 5);
    }
    put(sum);

    uint64_t quotients = 0;
    for (int i = 1; i < 1024; i++) {
        quotients += numbers[i] / (numbers[i - 1] | 1) + (int64_t)(int32_t)numbers[i] % 7;
    }
    put(quotients);

    float total = 0;
    for (int i = 0; i < 1024; i++) {
        total += values[i] * values[(i * 7) & 1023];
    }
    double root = 1;
    for (int i = 0; i < 40; i++) {
        root = (root + 2.0 / root) / 2;
    }
    union { float f; uint32_t u; } t = {total};
    union { double d; uint64_t u; } r = {root};
    put(t.u);
    put(r.u);

    uint64_t value = 3;
    for (int i = 0; i < 12; i++) {
        value = steps[numbers[i] & 1](value) % 1000003;
    }
    put(value);
    put(fibonacci(20));

    uint64_t status;
    __asm__ volatile("mrs %0, fpsr" : "=r"(status));
    put(status);

    call3(SYS_exit_group, (long)(sum & 0x7f), 0, 0);
    for (;;) {
    }
}
