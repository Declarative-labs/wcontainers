// xsave and xrstor of x87, SSE and AVX state: the area xsave writes after every component was written, and
// what xrstor loads, initialises or leaves for components named or not in XSTATE_BV and in edx:eax. Built for
// a program told XSAVE and AVX.
#include <stdint.h>
#include <stdio.h>
#include <string.h>

enum {
    AREA = 1024,
    LEGACY_FIELDS = 32,
    REGISTERS = 32,
    REGISTERS_END = 416,
    HEADER = 512,
    AVX = 576,
    AVX_END = 832,
};

static uint8_t area[AREA] __attribute__((aligned(64)));
static uint8_t saved[AREA] __attribute__((aligned(64)));
static uint8_t pattern[16][32];

static void dump(const char *what, const uint8_t *p, int from, int to) {
    printf("%s %d:", what, from);
    for (int i = from; i < to; i++) {
        printf("%s%02x", i % 16 ? "" : " ", p[i]);
    }
    printf("\n");
}

// The state as a second xsave of every component sees it, without its header, which says what the processor
// saw in use.
static void show(const char *what) {
    dump(what, saved, 0, LEGACY_FIELDS);
    dump(what, saved, REGISTERS, REGISTERS_END);
    dump(what, saved, AVX, AVX_END);
}

static void restore_and_save(uint32_t components) {
    memset(saved, 0, sizeof saved);
    uint32_t eax = components, edx = 0;
    __asm__ volatile("xrstor64 (%2)\n\t"
                     "mov $7, %%eax\n\t"
                     "xsave64 (%3)"
                     : "+a"(eax), "+d"(edx)
                     : "r"(area), "r"(saved)
                     : "memory", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7", "xmm8",
                       "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15");
}

int main(void) {
    for (int r = 0; r < 16; r++) {
        for (int b = 0; b < 32; b++) {
            pattern[r][b] = (uint8_t)(r * 37 + b * 11 + 1);
        }
    }
    memset(area, 0x5a, sizeof area);
    memset(area + HEADER, 0, AVX - HEADER);
    uint16_t control = 0x27f;
    uint32_t mxcsr = 0x1fa0;
    __asm__ volatile("vmovdqu 0(%0), %%ymm0\n\t"
                     "vmovdqu 32(%0), %%ymm1\n\t"
                     "vmovdqu 64(%0), %%ymm2\n\t"
                     "vmovdqu 96(%0), %%ymm3\n\t"
                     "vmovdqu 128(%0), %%ymm4\n\t"
                     "vmovdqu 160(%0), %%ymm5\n\t"
                     "vmovdqu 192(%0), %%ymm6\n\t"
                     "vmovdqu 224(%0), %%ymm7\n\t"
                     "vmovdqu 256(%0), %%ymm8\n\t"
                     "vmovdqu 288(%0), %%ymm9\n\t"
                     "vmovdqu 320(%0), %%ymm10\n\t"
                     "vmovdqu 352(%0), %%ymm11\n\t"
                     "vmovdqu 384(%0), %%ymm12\n\t"
                     "vmovdqu 416(%0), %%ymm13\n\t"
                     "vmovdqu 448(%0), %%ymm14\n\t"
                     "vmovdqu 480(%0), %%ymm15\n\t"
                     "fldcw (%2)\n\t"
                     "ldmxcsr (%3)\n\t"
                     "mov $7, %%eax\n\t"
                     "xor %%edx, %%edx\n\t"
                     "xsave64 (%1)"
                     :
                     : "r"(pattern), "r"(area), "r"(&control), "r"(&mxcsr)
                     : "rax", "rdx", "memory", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6",
                       "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15");
    dump("xsave", area, 0, LEGACY_FIELDS);
    dump("xsave", area, REGISTERS, REGISTERS_END);
    dump("xsave", area, HEADER, HEADER + 16);
    dump("xsave", area, AVX, AVX_END);

    // SSE put in its initial configuration, MXCSR still loaded; x87 and AVX loaded.
    uint32_t round_toward_zero = 0x7f80;
    memcpy(area + 24, &round_toward_zero, sizeof round_toward_zero);
    area[HEADER] = 1 | 4;
    restore_and_save(7);
    show("sse initial");

    // x87 and AVX initial, SSE not named in edx:eax so left as it is.
    area[HEADER] = 2;
    restore_and_save(1 | 4);
    show("x87 and avx initial");

    // Everything loaded again, then only SSE named.
    area[HEADER] = 7;
    restore_and_save(7);
    show("all loaded");
    area[HEADER] = 0;
    restore_and_save(2);
    show("sse alone initial");
    return 0;
}
