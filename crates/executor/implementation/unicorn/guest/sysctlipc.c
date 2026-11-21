// The IPC namespace's limits as its sysctls set them: what /proc/sys and IPC_INFO show, and the
// calls each limit refuses (segment sizes and counts, message sizes and queue bytes, semaphore
// counts and operations, POSIX queue defaults, sizes and counts).

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <mqueue.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ipc.h>
#include <sys/msg.h>
#include <sys/sem.h>
#include <sys/shm.h>
#include <sys/syscall.h>
#include <unistd.h>

struct attr {
    long flags, maxmsg, msgsize, curmsgs, reserved[4];
};

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: %s\n", what, strerror(errno));
    else
        printf("%s: ok\n", what);
}

static long sysctl(const char *path) {
    char text[128] = {0};
    FILE *file = fopen(path, "r");
    if (!file || !fgets(text, sizeof text, file)) {
        printf("%s: %s\n", path, strerror(errno));
        return -1;
    }
    fclose(file);
    printf("%s: %s", path, text);
    return strtol(text, NULL, 10);
}

static void segments(void) {
    long max = sysctl("/proc/sys/kernel/shmmax");
    long count = sysctl("/proc/sys/kernel/shmmni");
    sysctl("/proc/sys/kernel/shmall");
    struct shminfo info;
    shmctl(0, IPC_INFO, (struct shmid_ds *)&info);
    printf("shm info: max %lu min %lu mni %lu seg %lu all %lu\n", info.shmmax, info.shmmin,
           info.shmmni, info.shmseg, info.shmall);
    show("shmget past shmmax", shmget(IPC_PRIVATE, max + 1, 0600));
    int ids[64], made = 0;
    for (int i = 0; i <= count && i < 64; i++) {
        ids[made] = shmget(IPC_PRIVATE, max, 0600);
        if (ids[made] < 0) {
            printf("segment %d: %s\n", i, strerror(errno));
            break;
        }
        made++;
    }
    printf("segments made: %d\n", made);
    for (int i = 0; i < made; i++)
        shmctl(ids[i], IPC_RMID, NULL);
}

static void messages(void) {
    long max = sysctl("/proc/sys/kernel/msgmax");
    long bytes = sysctl("/proc/sys/kernel/msgmnb");
    long count = sysctl("/proc/sys/kernel/msgmni");
    struct msginfo info;
    msgctl(0, IPC_INFO, (struct msqid_ds *)&info);
    printf("msg info: max %d mnb %d mni %d map %d tql %d\n", info.msgmax, info.msgmnb, info.msgmni,
           info.msgmap, info.msgtql);
    int queue = msgget(IPC_PRIVATE, 0600);
    struct msqid_ds state;
    msgctl(queue, IPC_STAT, &state);
    printf("queue bytes: %lu\n", (unsigned long)state.msg_qbytes);
    struct {
        long kind;
        char text[4096];
    } message = {1, {0}};
    show("msgsnd past msgmax", msgsnd(queue, &message, max + 1, IPC_NOWAIT));
    show("msgsnd of msgmax", msgsnd(queue, &message, max, IPC_NOWAIT));
    int sent = 1;
    while (msgsnd(queue, &message, max, IPC_NOWAIT) == 0 && sent < 64)
        sent++;
    printf("messages held: %d of %ld bytes in %ld (%s)\n", sent, max, bytes, strerror(errno));
    state.msg_qbytes = bytes + 1;
    show("raise queue bytes", msgctl(queue, IPC_SET, &state));
    msgctl(queue, IPC_RMID, NULL);
    int ids[64], made = 0;
    for (int i = 0; i <= count && i < 64; i++) {
        ids[made] = msgget(IPC_PRIVATE, 0600);
        if (ids[made] < 0) {
            printf("queue %d: %s\n", i, strerror(errno));
            break;
        }
        made++;
    }
    printf("queues made: %d\n", made);
    for (int i = 0; i < made; i++)
        msgctl(ids[i], IPC_RMID, NULL);
}

static void semaphores(void) {
    long set[4] = {0};
    FILE *file = fopen("/proc/sys/kernel/sem", "r");
    char text[128] = {0};
    if (file && fgets(text, sizeof text, file)) {
        printf("/proc/sys/kernel/sem: %s", text);
        sscanf(text, "%ld %ld %ld %ld", &set[0], &set[1], &set[2], &set[3]);
    }
    if (file)
        fclose(file);
    struct seminfo info;
    semctl(0, 0, IPC_INFO, &info);
    printf("sem info: msl %d mns %d opm %d mni %d map %d mnu %d ume %d\n", info.semmsl, info.semmns,
           info.semopm, info.semmni, info.semmap, info.semmnu, info.semume);
    show("semget past semmsl", semget(IPC_PRIVATE, set[0] + 1, 0600));
    int id = semget(IPC_PRIVATE, set[0], 0600);
    show("semget of semmsl", id);
    struct sembuf operations[64] = {0};
    long many = set[2] + 1 < 64 ? set[2] + 1 : 64;
    for (int i = 0; i < many; i++)
        operations[i] = (struct sembuf){.sem_num = 0, .sem_op = 1, .sem_flg = IPC_NOWAIT};
    show("semop past semopm", semop(id, operations, many));
    show("semop of semopm", semop(id, operations, many - 1));
    semctl(id, 0, IPC_RMID);
    int ids[64], made = 0;
    for (int i = 0; i <= set[3] && i < 64; i++) {
        ids[made] = semget(IPC_PRIVATE, 1, 0600);
        if (ids[made] < 0) {
            printf("set %d: %s\n", i, strerror(errno));
            break;
        }
        made++;
    }
    printf("sets made: %d\n", made);
    for (int i = 0; i < made; i++)
        semctl(ids[i], 0, IPC_RMID);
}

static void posix_queues(void) {
    long count = sysctl("/proc/sys/fs/mqueue/queues_max");
    long most = sysctl("/proc/sys/fs/mqueue/msg_max");
    long largest = sysctl("/proc/sys/fs/mqueue/msgsize_max");
    sysctl("/proc/sys/fs/mqueue/msg_default");
    sysctl("/proc/sys/fs/mqueue/msgsize_default");
    int queue = syscall(SYS_mq_open, "q-default", O_RDWR | O_CREAT, 0600, NULL);
    show("mq_open with defaults", queue);
    struct attr attr = {0};
    syscall(SYS_mq_getsetattr, queue, NULL, &attr);
    printf("default attributes: %ld messages of %ld bytes\n", attr.maxmsg, attr.msgsize);
    close(queue);
    syscall(SYS_mq_unlink, "q-default");
    struct attr asked = {.maxmsg = most + 1, .msgsize = 128};
    show("mq_open past msg_max", syscall(SYS_mq_open, "q-big", O_RDWR | O_CREAT, 0600, &asked));
    asked = (struct attr){.maxmsg = 1, .msgsize = largest + 1};
    show("mq_open past msgsize_max",
         syscall(SYS_mq_open, "q-big", O_RDWR | O_CREAT, 0600, &asked));
    asked = (struct attr){.maxmsg = most, .msgsize = largest};
    queue = syscall(SYS_mq_open, "q-big", O_RDWR | O_CREAT, 0600, &asked);
    show("mq_open of msg_max and msgsize_max", queue);
    close(queue);
    syscall(SYS_mq_unlink, "q-big");
    int made = 0;
    char name[32];
    for (int i = 0; i <= count && i < 64; i++) {
        snprintf(name, sizeof name, "q-%d", i);
        asked = (struct attr){.maxmsg = 1, .msgsize = 128};
        int one = syscall(SYS_mq_open, name, O_RDWR | O_CREAT, 0600, &asked);
        if (one < 0) {
            printf("posix queue %d: %s\n", i, strerror(errno));
            break;
        }
        close(one);
        made++;
    }
    printf("posix queues made: %d\n", made);
    for (int i = 0; i < made; i++) {
        snprintf(name, sizeof name, "q-%d", i);
        syscall(SYS_mq_unlink, name);
    }
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    segments();
    messages();
    semaphores();
    posix_queues();
    return 0;
}
