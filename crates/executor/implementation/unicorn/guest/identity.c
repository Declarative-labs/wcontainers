// Every register of the identification space Linux answers for a program (op0 3, op1 0, CRn 0,
// CRm 0 to 7, each op2), read with mrs, and the two hardware capability words: one line each, the
// value in hexadecimal or "undefined" where the read is an undefined instruction.
//
// Then what the capability words and /proc/cpuinfo's Features must be for the registers this
// program read, derived here as Linux derives them from the same fields
// (<https://docs.kernel.org/arch/arm64/elf_hwcaps.html>): "derived AT_HWCAP", "derived AT_HWCAP2",
// the Features line the kernel gave ("cpuinfo Features"), whether every processor's line is the
// same ("cpuinfo same"), and the line the derived words name ("derived Features").
#include <setjmp.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/auxv.h>

static sigjmp_buf undefined;

static void on_undefined(int signal) {
    (void)signal;
    siglongjmp(undefined, 1);
}

#define READER(crm, op2)                                                  \
    static uint64_t read_##crm##_##op2(void) {                            \
        uint64_t value;                                                   \
        __asm__ volatile("mrs %0, S3_0_C0_C" #crm "_" #op2 : "=r"(value)); \
        return value;                                                     \
    }
#define ROW(crm) \
    READER(crm, 0) READER(crm, 1) READER(crm, 2) READER(crm, 3) \
    READER(crm, 4) READER(crm, 5) READER(crm, 6) READER(crm, 7)
ROW(0) ROW(1) ROW(2) ROW(3) ROW(4) ROW(5) ROW(6) ROW(7)

#define ENTRY(crm, op2) {crm, op2, read_##crm##_##op2}
#define ENTRIES(crm)                                              \
    ENTRY(crm, 0), ENTRY(crm, 1), ENTRY(crm, 2), ENTRY(crm, 3), \
    ENTRY(crm, 4), ENTRY(crm, 5), ENTRY(crm, 6), ENTRY(crm, 7)

static const struct {
    int crm, op2;
    uint64_t (*read)(void);
} registers[] = {ENTRIES(0), ENTRIES(1), ENTRIES(2), ENTRIES(3),
                 ENTRIES(4), ENTRIES(5), ENTRIES(6), ENTRIES(7)};

// What each read gave, by CRm * 8 + op2; 0 where it was undefined.
static uint64_t seen[64];

// The registers by (CRm, op2), as the Arm architecture names them.
enum {
    PFR0 = 4 * 8 + 0,
    PFR1 = 4 * 8 + 1,
    PFR2 = 4 * 8 + 2,
    ZFR0 = 4 * 8 + 4,
    SMFR0 = 4 * 8 + 5,
    FPFR0 = 4 * 8 + 7,
    ISAR0 = 6 * 8 + 0,
    ISAR1 = 6 * 8 + 1,
    ISAR2 = 6 * 8 + 2,
    ISAR3 = 6 * 8 + 3,
    MMFR0 = 7 * 8 + 0,
    MMFR1 = 7 * 8 + 1,
    MMFR2 = 7 * 8 + 2,
    MMFR3 = 7 * 8 + 3,
};

// The four-bit field of a register at bit `low`, unsigned.
static unsigned field(int place, int low) {
    return (unsigned)(seen[place] >> low) & 0xF;
}

// The same field read as signed, as Linux reads FP and AdvSIMD (0xF is -1: not implemented).
static int signed_field(int place, int low) {
    int value = (int)field(place, low);
    return value >= 8 ? value - 16 : value;
}

// One bit of a register (the SME fields one bit wide).
static unsigned bit(int place, int low) {
    return (unsigned)(seen[place] >> low) & 1;
}

// A capability: in word 1 (AT_HWCAP) or 2 (AT_HWCAP2) at bit `at`, present when the field at
// `low` of `place` is at least `least`.
struct capability {
    int word, at, place, low;
    unsigned least;
};

// Each capability Linux gives from one field reaching a level, from the table of
// <https://docs.kernel.org/arch/arm64/elf_hwcaps.html>, by its bit in uapi/asm/hwcap.h.
static const struct capability by_level[] = {
    {1, 3, ISAR0, 4, 1},    // aes: AES
    {1, 4, ISAR0, 4, 2},    // pmull: AES is PMULL
    {1, 5, ISAR0, 8, 1},    // sha1
    {1, 6, ISAR0, 12, 1},   // sha2
    {1, 7, ISAR0, 16, 1},   // crc32
    {1, 8, ISAR0, 20, 2},   // atomics: Atomic is LSE
    {1, 12, ISAR0, 28, 1},  // asimdrdm: RDM
    {1, 13, ISAR1, 12, 1},  // jscvt
    {1, 14, ISAR1, 16, 1},  // fcma
    {1, 15, ISAR1, 20, 1},  // lrcpc
    {1, 16, ISAR1, 0, 1},   // dcpop: DPB
    {1, 17, ISAR0, 32, 1},  // sha3
    {1, 18, ISAR0, 36, 1},  // sm3
    {1, 19, ISAR0, 40, 1},  // sm4
    {1, 20, ISAR0, 44, 1},  // asimddp: DP
    {1, 21, ISAR0, 12, 2},  // sha512: SHA2 is SHA512
    {1, 22, PFR0, 32, 1},   // sve
    {1, 23, ISAR0, 48, 1},  // asimdfhm: FHM
    {1, 24, PFR0, 48, 1},   // dit
    {1, 25, MMFR2, 32, 1},  // uscat: AT
    {1, 26, ISAR1, 20, 2},  // ilrcpc: LRCPC is LRCPC2
    {1, 27, ISAR0, 52, 1},  // flagm: TS
    {1, 28, PFR1, 4, 2},    // ssbs: SSBS is SSBS2
    {1, 29, ISAR1, 36, 1},  // sb
    {1, 32, PFR1, 44, 1},   // gcs
    {1, 33, ISAR2, 52, 2},  // cmpbr: CSSC is CMPBR
    {1, 34, ISAR3, 28, 1},  // fprcvt
    {2, 0, ISAR1, 0, 2},    // dcpodp: DPB is DPB2
    {2, 7, ISAR0, 52, 2},   // flagm2: TS is FLAGM2
    {2, 8, ISAR1, 32, 1},   // frint: FRINTTS
    {2, 13, ISAR1, 52, 1},  // i8mm
    {2, 14, ISAR1, 44, 1},  // bf16
    {2, 15, ISAR1, 48, 1},  // dgh
    {2, 16, ISAR0, 60, 1},  // rng: RNDR
    {2, 17, PFR1, 0, 1},    // bti: BT
    {2, 18, PFR1, 8, 2},    // mte: MTE is MTE2
    {2, 19, MMFR0, 60, 1},  // ecv
    {2, 20, MMFR1, 44, 1},  // afp
    {2, 21, ISAR2, 4, 1},   // rpres
    {2, 22, PFR1, 8, 3},    // mte3
    {2, 23, PFR1, 24, 1},   // sme
    {2, 31, ISAR2, 0, 2},   // wfxt: WFxT is 2
    {2, 32, ISAR1, 44, 2},  // ebf16: BF16 is EBF16
    {2, 34, ISAR2, 52, 1},  // cssc
    {2, 35, ISAR2, 48, 1},  // rprfm
    {2, 43, ISAR2, 16, 1},  // mops
    {2, 44, ISAR2, 20, 1},  // hbc: BC
    {2, 46, ISAR1, 20, 3},  // lrcpc3: LRCPC is LRCPC3
    {2, 47, ISAR0, 20, 3},  // lse128: Atomic is LSE128
    {2, 48, PFR2, 32, 1},   // fpmr
    {2, 49, ISAR2, 56, 1},  // lut
    {2, 50, ISAR3, 4, 1},   // faminmax
    {2, 63, MMFR3, 16, 1},  // poe: S1POE
};

// The capabilities of SVE's own register, which Linux gives only with SVE.
static const struct capability with_sve[] = {
    {1, 39, ZFR0, 4, 3},   // sveaes2: AES is AES2
    {1, 41, ZFR0, 0, 3},   // sve2p2: SVEver is SVE2p2
    {2, 1, ZFR0, 0, 1},    // sve2: SVEver
    {2, 2, ZFR0, 4, 1},    // sveaes: AES
    {2, 3, ZFR0, 4, 2},    // svepmull: AES is PMULL128
    {2, 4, ZFR0, 16, 1},   // svebitperm: BitPerm
    {2, 5, ZFR0, 32, 1},   // svesha3: SHA3
    {2, 6, ZFR0, 40, 1},   // svesm4: SM4
    {2, 9, ZFR0, 44, 1},   // svei8mm: I8MM
    {2, 10, ZFR0, 52, 1},  // svef32mm: F32MM
    {2, 11, ZFR0, 56, 1},  // svef64mm: F64MM
    {2, 12, ZFR0, 20, 1},  // svebf16: BF16
    {2, 33, ZFR0, 20, 2},  // sveebf16: BF16 is EBF16
    {2, 36, ZFR0, 0, 2},   // sve2p1: SVEver is SVE2p1
    {2, 45, ZFR0, 24, 1},  // sveb16b16: B16B16
};

// The capabilities of SME's own register, which Linux gives only with SME: the one-bit fields
// F32F32 (32), BI32I32 (33), B16F32 (34), F16F32 (35), F16F16 (42), B16B16 (43), F64F64 (48) and
// FA64 (63), and the wider I8I32 (36, all ones), I16I32 (44, 0x5), I16I64 (52, all ones) and
// SMEver (56).
static uint64_t sme_capabilities(void) {
    uint64_t word = 0;
    word |= (uint64_t)bit(SMFR0, 32) << 29;           // smef32f32
    word |= (uint64_t)bit(SMFR0, 33) << 40;           // smebi32i32
    word |= (uint64_t)bit(SMFR0, 34) << 28;           // smeb16f32
    word |= (uint64_t)bit(SMFR0, 35) << 27;           // smef16f32
    word |= (uint64_t)(field(SMFR0, 36) == 0xF) << 26;  // smei8i32
    word |= (uint64_t)bit(SMFR0, 42) << 42;           // smef16f16
    word |= (uint64_t)bit(SMFR0, 43) << 41;           // smeb16b16
    word |= (uint64_t)(field(SMFR0, 44) == 0x5) << 39;  // smei16i32
    word |= (uint64_t)bit(SMFR0, 48) << 25;           // smef64f64
    word |= (uint64_t)(field(SMFR0, 52) == 0xF) << 24;  // smei16i64
    word |= (uint64_t)(field(SMFR0, 56) >= 1) << 37;  // sme2
    word |= (uint64_t)(field(SMFR0, 56) >= 2) << 38;  // sme2p1
    word |= (uint64_t)bit(SMFR0, 63) << 30;           // smefa64
    return word;
}

// The bit of AT_HWCAP no identification register gives: the timer's event stream.
#define EVENT_STREAM 2

static void derive(uint64_t *first, uint64_t *second) {
    uint64_t words[3] = {0, 0, 0};
    // fp and fphp, asimd and asimdhp from the signed FP and AdvSIMD fields; cpuid always, as
    // Linux answers these reads.
    words[1] |= (uint64_t)(signed_field(PFR0, 16) >= 0) << 0;
    words[1] |= (uint64_t)(signed_field(PFR0, 20) >= 0) << 1;
    words[1] |= (uint64_t)(signed_field(PFR0, 16) >= 1) << 9;
    words[1] |= (uint64_t)(signed_field(PFR0, 20) >= 1) << 10;
    words[1] |= (uint64_t)1 << 11;
    // paca and pacg: address or generic authentication by any of its algorithms.
    unsigned address = field(ISAR1, 4) | field(ISAR1, 8) | field(ISAR2, 12);
    unsigned generic = field(ISAR1, 24) | field(ISAR1, 28) | field(ISAR2, 8);
    words[1] |= (uint64_t)(address != 0) << 30;
    words[1] |= (uint64_t)(generic != 0) << 31;
    for (unsigned i = 0; i < sizeof by_level / sizeof by_level[0]; i++) {
        const struct capability *c = &by_level[i];
        words[c->word] |= (uint64_t)(field(c->place, c->low) >= c->least) << c->at;
    }
    if (field(PFR0, 32) >= 1) {
        for (unsigned i = 0; i < sizeof with_sve / sizeof with_sve[0]; i++) {
            const struct capability *c = &with_sve[i];
            words[c->word] |= (uint64_t)(field(c->place, c->low) >= c->least) << c->at;
        }
    }
    if (field(PFR1, 24) >= 1) {
        words[2] |= sme_capabilities();
    }
    *first = words[1];
    *second = words[2];
}

// The names /proc/cpuinfo gives each capability, by bit, AT_HWCAP's then AT_HWCAP2's
// (Linux's `hwcap_str`, in the order of the bits).
static const char *const first_names[64] = {
    "fp", "asimd", "evtstrm", "aes", "pmull", "sha1", "sha2", "crc32", "atomics", "fphp",
    "asimdhp", "cpuid", "asimdrdm", "jscvt", "fcma", "lrcpc", "dcpop", "sha3", "sm3", "sm4",
    "asimddp", "sha512", "sve", "asimdfhm", "dit", "uscat", "ilrcpc", "flagm", "ssbs", "sb",
    "paca", "pacg", "gcs", "cmpbr", "fprcvt", "f8mm8", "f8mm4", "svef16mm", "sveeltperm",
    "sveaes2", "svebfscale", "sve2p2", "sme2p2", "smesbitperm", "smeaes", "smesfexpa",
    "smestmop", "smesmop4",
};
static const char *const second_names[64] = {
    "dcpodp", "sve2", "sveaes", "svepmull", "svebitperm", "svesha3", "svesm4", "flagm2",
    "frint", "svei8mm", "svef32mm", "svef64mm", "svebf16", "i8mm", "bf16", "dgh", "rng", "bti",
    "mte", "ecv", "afp", "rpres", "mte3", "sme", "smei16i64", "smef64f64", "smei8i32",
    "smef16f32", "smeb16f32", "smef32f32", "smefa64", "wfxt", "ebf16", "sveebf16", "cssc",
    "rprfm", "sve2p1", "sme2", "sme2p1", "smei16i32", "smebi32i32", "smeb16b16", "smef16f16",
    "mops", "hbc", "sveb16b16", "lrcpc3", "lse128", "fpmr", "lut", "faminmax", "f8cvt", "f8fma",
    "f8dp4", "f8dp2", "f8e4m3", "f8e5m2", "smelutv2", "smef8f16", "smef8f32", "smesf8fma",
    "smesf8dp4", "smesf8dp2", "poe",
};

static void print_names(uint64_t word, const char *const names[64], int *first) {
    for (int at = 0; at < 64; at++) {
        if (word >> at & 1) {
            printf("%s%s", *first ? "" : " ", names[at] ? names[at] : "unnamed");
            *first = 0;
        }
    }
}

// Prints the first processor's Features line of /proc/cpuinfo and whether every one is the same.
static void print_cpuinfo(void) {
    static char line[4096], features[4096];
    int count = 0, same = 1;
    FILE *file = fopen("/proc/cpuinfo", "r");
    if (!file) {
        printf("cpuinfo Features unreadable\ncpuinfo same no\n");
        return;
    }
    while (fgets(line, sizeof line, file)) {
        if (strncmp(line, "Features", 8) != 0) {
            continue;
        }
        char *value = strchr(line, ':');
        value = value ? value + 1 : line;
        while (*value == ' ' || *value == '\t') {
            value++;
        }
        value[strcspn(value, "\n")] = 0;
        // Linux ends the line with a space after the last name.
        size_t length = strlen(value);
        while (length > 0 && value[length - 1] == ' ') {
            value[--length] = 0;
        }
        if (count == 0) {
            strcpy(features, value);
        } else if (strcmp(features, value) != 0) {
            same = 0;
        }
        count++;
    }
    fclose(file);
    printf("cpuinfo Features %s\n", count ? features : "missing");
    printf("cpuinfo same %s\n", count && same ? "yes" : "no");
}

int main(void) {
    struct sigaction action = {0};
    action.sa_handler = on_undefined;
    action.sa_flags = SA_NODEFER;
    sigaction(SIGILL, &action, 0);
    for (unsigned i = 0; i < sizeof registers / sizeof registers[0]; i++) {
        printf("S3_0_C0_C%d_%d ", registers[i].crm, registers[i].op2);
        fflush(stdout);
        if (sigsetjmp(undefined, 1) == 0) {
            uint64_t value = registers[i].read();
            seen[registers[i].crm * 8 + registers[i].op2] = value;
            printf("%016llx\n", (unsigned long long)value);
        } else {
            printf("undefined\n");
        }
    }
    uint64_t given = getauxval(AT_HWCAP);
    printf("AT_HWCAP %016llx\n", (unsigned long long)given);
    printf("AT_HWCAP2 %016llx\n", (unsigned long long)getauxval(AT_HWCAP2));
    uint64_t first, second;
    derive(&first, &second);
    printf("derived AT_HWCAP %016llx\n", (unsigned long long)first);
    printf("derived AT_HWCAP2 %016llx\n", (unsigned long long)second);
    print_cpuinfo();
    // The names follow the derived words, with the event stream as the kernel gave it, which no
    // register tells.
    int none = 1;
    printf("derived Features ");
    print_names(first | (given & (uint64_t)1 << EVENT_STREAM), first_names, &none);
    print_names(second, second_names, &none);
    printf("\n");
    return 0;
}
