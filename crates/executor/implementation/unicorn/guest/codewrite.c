// Runs code, has the kernel write other code over it (a read from a pipe), runs it again, and
// exits with what the second run returned: 2 when the new code ran, 1 when the old one did.
#include <stdint.h>
#include <sys/mman.h>
#include <unistd.h>

// mov w0, #value; ret
static void returning(uint32_t *code, uint32_t value) {
    code[0] = 0x52800000u | (value << 5);
    code[1] = 0xd65f03c0u;
}

int main(void) {
    uint32_t *code = mmap(0, 4096, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (code == MAP_FAILED) return 255;
    returning(code, 1);
    __builtin___clear_cache((char *)code, (char *)(code + 2));
    int (*run)(void) = (int (*)(void))code;
    if (run() != 1) return 254;
    uint32_t replacement[2];
    returning(replacement, 2);
    int ends[2];
    if (pipe(ends) != 0) return 253;
    if (write(ends[1], replacement, sizeof replacement) != sizeof replacement) return 252;
    if (read(ends[0], code, sizeof replacement) != sizeof replacement) return 251;
    __builtin___clear_cache((char *)code, (char *)(code + 2));
    return run();
}
