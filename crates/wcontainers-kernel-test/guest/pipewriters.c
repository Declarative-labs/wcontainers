// Several writers of atomic writes wait together on a full pipe while its reader is late, and
// all their bytes arrive whole.
#include <stdio.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int ends[2];
    pipe(ends);
    for (int w = 0; w < 3; w++)
        if (fork() == 0) {
            close(ends[0]);
            static char buffer[4090];
            for (int i = 0; i < 30; i++)
                if (write(ends[1], buffer, sizeof buffer) != sizeof buffer)
                    _exit(1);
            _exit(0);
        }
    close(ends[1]);
    struct timespec late = {0, 200000000};
    nanosleep(&late, NULL);
    static char into[4090];
    long total = 0, reads = 0, whole = 0;
    for (;;) {
        long got = read(ends[0], into, sizeof into);
        if (got <= 0)
            break;
        total += got;
        reads++;
        whole += got == sizeof into;
    }
    int status, clean = 0;
    while (wait(&status) > 0)
        clean += WIFEXITED(status) && WEXITSTATUS(status) == 0;
    printf("read %ld bytes in %ld reads, %ld whole, %d writers done\n", total, reads, whole, clean);
    return 0;
}
