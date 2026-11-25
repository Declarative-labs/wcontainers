// Code the program writes while it runs, into memory that held no code in the file, then calls, as a JIT does;
// then the same place written again with other code. x86 needs no announcement of either.
#include <stdio.h>
#include <string.h>

static unsigned char area_add[64] __attribute__((aligned(64)));
static unsigned char area_mul[64] __attribute__((aligned(64)));

typedef long (*fn)(long, long);

static const unsigned char add_code[] = {0x48, 0x8d, 0x04, 0x37, 0xc3};                   // lea rax, [rdi+rsi]; ret
static const unsigned char mul_code[] = {0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0xc3}; // mov rax, rdi; imul rax, rsi; ret
static const unsigned char sub_code[] = {0x48, 0x89, 0xf8, 0x48, 0x29, 0xf0, 0xc3};       // mov rax, rdi; sub rax, rsi; ret

int main(void) {
    memcpy(area_add, add_code, sizeof add_code);
    memcpy(area_mul, mul_code, sizeof mul_code);
    fn add = (fn)(void *)area_add;
    fn mul = (fn)(void *)area_mul;
    long total = 0;
    for (long i = 0; i < 1000; i++) total = add(total, mul(i, 3));
    // The same place written again with other code runs the new code.
    memcpy(area_add, sub_code, sizeof sub_code);
    __asm__ volatile("" : : : "memory");
    total = add(total, 500);
    printf("%ld\n", total);
    return 0;
}
