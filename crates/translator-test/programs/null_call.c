// A call through a null function pointer, which Linux ends with SIGSEGV.
#include <stdio.h>

typedef long (*fn)(long);
static volatile fn pointer;

int main(void) {
    printf("before\n");
    fflush(stdout);
    return (int)pointer(3);
}
