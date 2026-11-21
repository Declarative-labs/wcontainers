// System V message queues: msgget's keys and flags, msgsnd's and msgrcv's sizes, types, flags and
// waits, a queue's status as messages come and go, limits, permissions, removal while processes wait,
// receivers handed messages in the order they waited, and every msgctl command. Times are printed as
// unset or set, and process numbers as the caller's, a child's or none.

#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

// Linux's own layouts for 64-bit ARM (`struct msqid64_ds`, `struct msginfo`).
struct perm64 {
    int key;
    unsigned uid, gid, cuid, cgid, mode;
    unsigned short seq, pad;
    unsigned long unused1, unused2;
};
struct msqid64 {
    struct perm64 perm;
    long stime, rtime, ctime;
    unsigned long cbytes, qnum, qbytes;
    int lspid, lrpid;
    unsigned long unused4, unused5;
};
struct msginfo {
    int msgpool, msgmap, msgmax, msgmnb, msgmni, msgssz, msgtql;
    unsigned short msgseg;
};

enum { RMID = 0, SET = 1, STAT = 2, INFO = 3, MSTAT = 11, MINFO = 12, MSTAT_ANY = 13 };
enum { CREAT = 01000, EXCL = 02000, NOWAIT = 04000, NOERROR = 010000, EXCEPT = 020000, COPY = 040000 };

static pid_t self, child_pid;

struct message {
    long type;
    char text[9000];
};

static void show(const char *what, long result) {
    if (result < 0)
        printf("%s: errno %d\n", what, errno);
    else
        printf("%s: %ld\n", what, result);
    errno = 0;
}

static long get(long key, long flags) { return syscall(SYS_msgget, key, flags); }
static long snd(int id, long type, const char *text, size_t size, long flags) {
    static struct message m;
    m.type = type;
    memset(m.text, 0, sizeof m.text);
    memcpy(m.text, text, strlen(text));
    return syscall(SYS_msgsnd, id, &m, size, flags);
}
static long rcv(int id, size_t size, long type, long flags, struct message *m) {
    memset(m, 0, sizeof *m);
    return syscall(SYS_msgrcv, id, m, size, type, flags);
}
static long ctl(int id, int command, void *buffer) { return syscall(SYS_msgctl, id, command, buffer); }

static const char *who(int pid) { return pid == 0 ? "none" : pid == self ? "me" : pid == child_pid ? "child" : "other"; }

static void status(const char *what, int id) {
    struct msqid64 s;
    memset(&s, 0, sizeof s);
    long r = ctl(id, STAT, &s);
    if (r < 0) {
        show(what, r);
        return;
    }
    printf("%s: key %d uid %u gid %u cuid %u cgid %u mode %o cbytes %lu qnum %lu qbytes %lu lspid %s lrpid %s "
           "stime %s rtime %s ctime %s\n",
           what, s.perm.key, s.perm.uid, s.perm.gid, s.perm.cuid, s.perm.cgid, s.perm.mode, s.cbytes, s.qnum,
           s.qbytes, who(s.lspid), who(s.lrpid), s.stime ? "set" : "0", s.rtime ? "set" : "0",
           s.ctime ? "set" : "0");
}

static void received(const char *what, long r, struct message *m) {
    show(what, r);
    if (r >= 0)
        printf("  type %ld text %.*s\n", m->type, (int)r, m->text);
}

static void basics(void) {
    struct message m;
    int id = get(0x4d51, CREAT | 0640);
    show("make", id >= 0);
    show("same key", get(0x4d51, 0) == id);
    show("same key exclusive", get(0x4d51, CREAT | EXCL));
    show("no key", get(0x4d52, 0));
    show("private twice differ", get(0, 0600) != get(0, 0600));
    status("new", id);
    show("send", snd(id, 1, "one", 3, 0));
    show("send type 2", snd(id, 2, "two", 3, 0));
    show("send type 3", snd(id, 3, "three", 5, 0));
    show("send empty", snd(id, 5, "", 0, 0));
    show("send type 0", snd(id, 0, "x", 1, 0));
    show("send type -1", snd(id, -1, "x", 1, 0));
    show("send too big", snd(id, 1, "x", 8193, 0));
    show("send negative size", syscall(SYS_msgsnd, id, &m, -1L, 0));
    show("send bad buffer", syscall(SYS_msgsnd, id, (void *)8, 4, 0));
    show("send bad id", snd(id + 1, 1, "x", 1, 0));
    show("send negative id", snd(-1, 1, "x", 1, 0));
    status("four messages", id);
    received("receive type 2", rcv(id, 100, 2, 0, &m), &m);
    received("receive except 1", rcv(id, 100, 1, EXCEPT, &m), &m);
    received("receive at most 3", rcv(id, 100, -3, 0, &m), &m);
    received("receive short", rcv(id, 2, 0, NOWAIT, &m), &m);
    received("receive short no error", rcv(id, 1, 0, NOWAIT | NOERROR, &m), &m);
    received("receive empty message", rcv(id, 100, 0, NOWAIT, &m), &m);
    received("receive none", rcv(id, 100, 0, NOWAIT, &m), &m);
    received("receive negative size", syscall(SYS_msgrcv, id, &m, -1L, 0, NOWAIT), &m);
    snd(id, 7, "seven", 5, 0);
    received("receive bad buffer", syscall(SYS_msgrcv, id, (void *)8, 100, 0, NOWAIT), &m);
    received("after bad buffer", rcv(id, 100, 0, NOWAIT, &m), &m);
    snd(id, 7, "copy me", 7, 0);
    snd(id, 8, "and me", 6, 0);
    received("copy without nowait", rcv(id, 100, 0, COPY, &m), &m);
    received("copy 1", rcv(id, 100, 1, COPY | NOWAIT, &m), &m);
    received("copy 5", rcv(id, 100, 5, COPY | NOWAIT, &m), &m);
    received("copy except", rcv(id, 100, 1, COPY | NOWAIT | EXCEPT, &m), &m);
    status("after copies", id);
    received("drain", rcv(id, 100, 0, NOWAIT, &m), &m);
    received("drain", rcv(id, 100, 0, NOWAIT, &m), &m);
    show("remove", ctl(id, RMID, NULL));
    show("remove again", ctl(id, RMID, NULL));
    show("send to removed", snd(id, 1, "x", 1, NOWAIT));
}

static void limits(void) {
    struct message m;
    int id = get(0, 0600);
    struct msqid64 s;
    ctl(id, STAT, &s);
    s.qbytes = 20;
    show("set qbytes 20", ctl(id, SET, &s));
    show("send 10", snd(id, 1, "0123456789", 10, NOWAIT));
    show("send 10 more", snd(id, 1, "0123456789", 10, NOWAIT));
    show("send 1 more", snd(id, 1, "0", 1, NOWAIT));
    show("send empty more", snd(id, 1, "", 0, NOWAIT));
    rcv(id, 100, 0, 0, &m);
    rcv(id, 100, 0, 0, &m);
    // A count of messages is held to the byte limit too.
    for (int i = 0; i < 22; i++)
        if (snd(id, 1, "", 0, NOWAIT) < 0) {
            printf("empty messages fit: %d errno %d\n", i, errno);
            break;
        }
    status("full of empty messages", id);
    ctl(id, STAT, &s);
    s.qbytes = 16385;
    show("set qbytes above the limit", ctl(id, SET, &s));
    s.qbytes = 16384;
    s.perm.mode = 0123777;
    s.perm.uid = 1000;
    show("set qbytes at the limit", ctl(id, SET, &s));
    status("after set", id);
    show("receive as owner without permission", rcv(id, 100, 0, NOWAIT, &m));
    show("send without permission", snd(id, 1, "x", 1, NOWAIT));
    s.perm.uid = 0;
    show("set back", ctl(id, SET, &s));
    s.perm.uid = (unsigned)-1;
    show("set bad uid", ctl(id, SET, &s));
    ctl(id, RMID, NULL);

    struct msginfo info;
    memset(&info, 0, sizeof info);
    int q = get(0, 0600);
    snd(q, 1, "abc", 3, 0);
    long max = ctl(0, INFO, &info);
    printf("info: max index %d pool %d map %d max %d mnb %d mni %d ssz %d tql %d seg %u\n", max >= 0, info.msgpool,
           info.msgmap, info.msgmax, info.msgmnb, info.msgmni, info.msgssz, info.msgtql, info.msgseg);
    memset(&info, 0, sizeof info);
    max = ctl(0, MINFO, &info);
    printf("msg info: max index %d pool %d map %d tql %d\n", max >= 0, info.msgpool, info.msgmap, info.msgtql);
    struct msqid64 t;
    long index = q & 0x7fff;
    show("stat by index is the id", ctl(index, MSTAT, &t) == q);
    show("stat any by index", ctl(index, MSTAT_ANY, &t) == q);
    show("stat unused index", ctl(30000, MSTAT, &t));
    show("bad command", ctl(q, 99, &t));
    show("negative command", ctl(q, -1, &t));
    show("stat bad buffer", ctl(q, STAT, (void *)8));
    ctl(q, RMID, NULL);
}

static void waits(void) {
    struct message m;
    int id = get(0, 0600);
    int ends[2];
    pipe(ends);
    // Three receivers wait: for type 2, for any, for any with a small buffer.
    pid_t kids[3];
    long types[3] = {2, 0, 0};
    size_t sizes[3] = {100, 100, 1};
    for (int i = 0; i < 3; i++) {
        kids[i] = fork();
        if (kids[i] == 0) {
            struct message got;
            long r = rcv(id, sizes[i], types[i], 0, &got);
            char line[200];
            int n = snprintf(line, sizeof line, "receiver %d: %ld errno %d type %ld text %.*s\n", i, r,
                             r < 0 ? errno : 0, got.type, r > 0 ? (int)r : 0, got.text);
            write(ends[1], line, n);
            _exit(0);
        }
        usleep(100000);
    }
    status("three waiting", id);
    show("send type 1", snd(id, 1, "first", 5, 0));
    usleep(100000);
    show("send type 2", snd(id, 2, "second", 6, 0));
    usleep(100000);
    show("send type 9", snd(id, 9, "toolong", 7, 0));
    for (int i = 0; i < 3; i++)
        waitpid(kids[i], NULL, 0);
    char buffer[1000];
    long n = read(ends[0], buffer, sizeof buffer);
    if (n > 0)
        fwrite(buffer, 1, n, stdout);
    status("after handing", id);
    received("left", rcv(id, 100, 0, NOWAIT, &m), &m);

    // A sender waits for room, and a removal wakes a waiter with EIDRM.
    struct msqid64 s;
    ctl(id, STAT, &s);
    s.qbytes = 4;
    ctl(id, SET, &s);
    snd(id, 1, "full", 4, 0);
    child_pid = fork();
    if (child_pid == 0) {
        long r = snd(id, 1, "wait", 4, 0);
        printf("waiting sender: %ld errno %d\n", r, r < 0 ? errno : 0);
        _exit(0);
    }
    usleep(100000);
    long room = rcv(id, 100, 0, 0, &m);
    waitpid(child_pid, NULL, 0);
    received("make room", room, &m);
    status("after waiting sender", id);
    rcv(id, 100, 0, 0, &m);
    child_pid = fork();
    if (child_pid == 0) {
        struct message got;
        long r = rcv(id, 100, 0, 0, &got);
        printf("receiver of a removed queue: %ld errno %d\n", r, r < 0 ? errno : 0);
        _exit(0);
    }
    usleep(100000);
    ctl(id, RMID, NULL);
    waitpid(child_pid, NULL, 0);

    // A signal whose handler asks for restarts still ends a wait with EINTR.
    id = get(0, 0600);
    child_pid = fork();
    if (child_pid == 0) {
        struct sigaction handled = {0};
        handled.sa_handler = (void (*)(int))abs;
        handled.sa_flags = SA_RESTART;
        sigaction(SIGUSR1, &handled, NULL);
        struct message got;
        long r = rcv(id, 100, 0, 0, &got);
        printf("interrupted receiver: %ld errno %d\n", r, r < 0 ? errno : 0);
        _exit(0);
    }
    usleep(100000);
    kill(child_pid, SIGUSR1);
    waitpid(child_pid, NULL, 0);
    ctl(id, RMID, NULL);
}

static void permissions(void) {
    int id = get(0x4d53, CREAT | 0600);
    child_pid = fork();
    if (child_pid == 0) {
        setresgid(1000, 1000, 1000);
        setresuid(1000, 1000, 1000);
        show("other user get", get(0x4d53, 0));
        show("other user get asking nothing", get(0x4d53, 0) >= 0);
        show("other user send", snd(id, 1, "x", 1, NOWAIT));
        struct message m;
        show("other user receive", rcv(id, 100, 0, NOWAIT, &m));
        struct msqid64 s;
        show("other user stat", ctl(id, STAT, &s));
        show("other user stat any", ctl(id & 0x7fff, MSTAT_ANY, &s) >= 0);
        show("other user remove", ctl(id, RMID, NULL));
        show("other user make", get(0x4d54, CREAT | 0600) >= 0);
        _exit(0);
    }
    waitpid(child_pid, NULL, 0);
    status("made by another user", get(0x4d54, 0));
    ctl(get(0x4d54, 0), RMID, NULL);
    ctl(id, RMID, NULL);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    self = getpid();
    basics();
    limits();
    waits();
    permissions();
    return 0;
}
