/* Writes code and runs it, announcing each change with __builtin___clear_cache (ic ivau). First in private
   memory: a function that gives 1, rewritten to give 2. Then in memory shared with a child: the parent runs a
   function that gives 3, the child rewrites it to give 4 and announces it, and the parent runs it again: the
   parent's translation must be dropped although the parent announced nothing. */

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

typedef int (*function)(void);

/* mov w0, #value; ret */
static void write_function(uint32_t *at, uint32_t value) {
    at[0] = 0x52800000u | (value << 5);
    at[1] = 0xd65f03c0u;
    __builtin___clear_cache((char *)at, (char *)(at + 2));
}

static uint32_t *code_memory(int flags) {
    void *at = mmap(0, 4096, PROT_READ | PROT_WRITE | PROT_EXEC, flags | MAP_ANONYMOUS, -1, 0);
    if (at == MAP_FAILED) {
        perror("mmap");
        _exit(1);
    }
    return at;
}

int main(void) {
    uint32_t *private_code = code_memory(MAP_PRIVATE);
    write_function(private_code, 1);
    int first = ((function)private_code)();
    write_function(private_code, 2);
    int second = ((function)private_code)();
    printf("private: %d %d\n", first, second);

    uint32_t *shared_code = code_memory(MAP_SHARED);
    write_function(shared_code, 3);
    int before = ((function)shared_code)();
    pid_t child = fork();
    if (child == 0) {
        write_function(shared_code, 4);
        _exit(0);
    }
    int status;
    waitpid(child, &status, 0);
    int after = ((function)shared_code)();
    printf("shared, changed by a child: %d %d\n", before, after);
    return 0;
}
