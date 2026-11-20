// Code that writes the page it runs from, with no announcement: a loop that writes the immediate of its own
// next instruction on every pass, and an instruction that writes into its own first byte, which runs as it
// was read and is something else the next time. From the build: PAGE_SIZE and PASSES (the loop's passes).
#include <stdio.h>
#include <string.h>

static unsigned char page[PAGE_SIZE] __attribute__((aligned(PAGE_SIZE)));

static const unsigned char loop_code[] = {
    0x88, 0x0d, 0x01, 0x00, 0x00, 0x00, // mov [rip+1], cl: the next instruction's immediate
    0xb8, 0x00, 0x00, 0x00, 0x00,       // mov eax, imm32
    0x01, 0xc2,                         // add edx, eax
    0xff, 0xc9,                         // dec ecx
    0x75, 0xef,                         // jnz to the start
    0xc3,                               // ret
};

static const unsigned char own_code[] = {
    0x89, 0xf8,                               // mov eax, edi
    0xc6, 0x05, 0xf9, 0xff, 0xff, 0xff, 0x90, // mov byte [rip-7], 0x90: its own first byte
    0xc3,                                     // ret
};

typedef long (*fn)(long);

int main(void) {
    memcpy(page, loop_code, sizeof loop_code);
    unsigned long sum = 0;
    unsigned eax;
    __asm__ volatile("call *%2" : "=a"(eax), "+d"(sum) : "r"(page), "c"(PASSES) : "rsi", "rdi", "r8", "r9", "r10", "r11", "memory", "cc");
    printf("eax %u sum %lu\n", eax, sum);

    unsigned char *own = page + PAGE_SIZE / 2;
    memcpy(own, own_code, sizeof own_code);
    fn f = (fn)(void *)own;
    long first = f(100);
    long second = f(100);
    printf("%ld %ld %02x\n", first, second, own[2]);
    return 0;
}
