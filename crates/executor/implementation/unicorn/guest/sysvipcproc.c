// /proc/sysvipc's three files with a segment, a queue and a set of semaphores in use: each line's
// length, its fields, and the times as whether they are set. A segment's resident bytes are left
// out: the kernel does not track which pages were touched (counted "page residency not tracked").
#include <stdio.h>
#include <string.h>
#include <sys/ipc.h>
#include <sys/msg.h>
#include <sys/sem.h>
#include <sys/shm.h>
#include <unistd.h>

static void show(const char *path, const int *times, int count) {
    FILE *file = fopen(path, "r");
    if (!file) {
        perror(path);
        return;
    }
    char line[512];
    while (fgets(line, sizeof line, file)) {
        printf("%s %zu:", path, strlen(line));
        int field = 0;
        for (char *token = strtok(line, " \n"); token; token = strtok(NULL, " \n"), field++) {
            int time = 0;
            for (int i = 0; i < count; i++)
                time |= times[i] == field;
            if (field == 14 && strstr(path, "shm") && strcmp(token, "rss") != 0)
                printf(" resident");
            else if (time && strcmp(token, "0") != 0 && token[0] >= '0' && token[0] <= '9')
                printf(" T");
            else
                printf(" %s", token);
        }
        printf("\n");
    }
    fclose(file);
}

int main(void) {
    int shm = shmget(0x1234, 100, IPC_CREAT | 0640);
    char *at = shmat(shm, NULL, 0);
    at[0] = 1;
    int unnamed = shmget(IPC_PRIVATE, 8192, IPC_CREAT | 0600);
    shmctl(unnamed, IPC_RMID, NULL);
    int msg = msgget(0x1235, IPC_CREAT | 0600);
    struct {
        long type;
        char text[16];
    } message = {1, "hello"};
    msgsnd(msg, &message, 6, 0);
    msgsnd(msg, &message, 11, 0);
    msgrcv(msg, &message, sizeof message.text, 0, 0);
    int sem = semget(0x1236, 3, IPC_CREAT | 0604);
    struct sembuf op = {1, 2, 0};
    semop(sem, &op, 1);
    int shm_times[] = {11, 12, 13}, msg_times[] = {11, 12, 13}, sem_times[] = {8, 9};
    show("/proc/sysvipc/shm", shm_times, 3);
    show("/proc/sysvipc/msg", msg_times, 3);
    show("/proc/sysvipc/sem", sem_times, 2);
    shmdt(at);
    shmctl(shm, IPC_RMID, NULL);
    msgctl(msg, IPC_RMID, NULL);
    semctl(sem, 0, IPC_RMID);
    show("/proc/sysvipc/shm", shm_times, 3);
    return 0;
}
