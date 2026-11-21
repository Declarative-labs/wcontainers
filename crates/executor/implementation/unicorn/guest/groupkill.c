// A process that sends SIGKILL to its own group ends with every other member of the group.
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int ends[2];
    pipe(ends);
    pid_t leader = fork();
    if (leader == 0) {
        setpgid(0, 0);
        for (int i = 0; i < 2; i++)
            if (fork() == 0) {
                close(ends[0]);
                for (;;)
                    pause();
            }
        close(ends[0]);
        close(ends[1]);
        kill(-getpgrp(), SIGKILL);
        _exit(1);
    }
    close(ends[1]);
    int status;
    waitpid(leader, &status, 0);
    printf("leader killed by %d\n", WIFSIGNALED(status) ? WTERMSIG(status) : 0);
    char byte;
    printf("members gone: %d\n", read(ends[0], &byte, 1) == 0);
    return 0;
}
