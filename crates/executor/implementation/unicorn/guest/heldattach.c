// A shared memory segment that only a child has attached, which waits for ever. Once the
// child is gone (its pipe's write end closes with it), the parent's exit code is how many
// attachments the segment still has.
#include <sys/shm.h>
#include <unistd.h>

int main(void) {
    int segment = shmget(IPC_PRIVATE, 4096, IPC_CREAT | 0600);
    int ends[2];
    if (segment < 0 || pipe(ends) != 0)
        return 100;
    if (fork() == 0) {
        close(ends[0]);
        if (shmat(segment, NULL, 0) == (void *)-1)
            _exit(101);
        write(ends[1], "a", 1);
        for (;;)
            pause();
    }
    close(ends[1]);
    char byte;
    if (read(ends[0], &byte, 1) != 1)
        return 102;
    read(ends[0], &byte, 1);
    struct shmid_ds state;
    if (shmctl(segment, IPC_STAT, &state) != 0)
        return 103;
    return (int)state.shm_nattch;
}
