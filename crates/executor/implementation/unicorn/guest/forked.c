// A process and its child, each of one thread, both waiting for ever: the kernel interface's
// suite takes a kernel at that point.
#include <unistd.h>

int main(void) {
    fork();
    for (;;) pause();
}
