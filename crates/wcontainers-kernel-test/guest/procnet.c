// The files of /proc/net that describe sockets, interfaces and routes, with sockets of each kind
// in each state. What differs between machines is shown as what it is: inode numbers as the
// descriptor they belong to, ports from the ephemeral range as E, kernel pointers as P, line
// numbers as N, UDP hash buckets as B, time left as T and TCP's reference counts as R; lines
// are sorted.
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

static unsigned long inodes[64];
static int fds[64], count;

static void track(int fd) {
    struct stat st;
    fstat(fd, &st);
    fds[count] = fd;
    inodes[count++] = st.st_ino;
}

// Appends to a line being made.
static void add(char *out, const char *format, ...) {
    va_list list;
    va_start(list, format);
    vsprintf(out + strlen(out), format, list);
    va_end(list);
}

// A word of a line, with what varies between machines shown as what it is.
static void word(char *out, const char *w, int column, int unix_file, int udp) {
    char *end;
    if (unix_file) {
        if (column == 0) {
            add(out, " P:");
            return;
        }
        if (column == 6) {
            unsigned long ino = strtoul(w, &end, 10);
            for (int i = 0; i < count; i++)
                if (inodes[i] == ino && ino) {
                    add(out, " fd%d", fds[i]);
                    return;
                }
            add(out, " %s", ino ? "other" : "0");
            return;
        }
        add(out, " %s", w);
        return;
    }
    if (column == 0) {
        // The line's number, or for UDP its hash bucket.
        add(out, udp ? " B:" : " N:");
        return;
    }
    if (column == 10 && !udp) {
        // TCP's reference count, which holds one more while a timer is pending.
        add(out, " R");
        return;
    }
    if (column == 1 || column == 2) {
        const char *colon = strrchr(w, ':');
        unsigned port = colon ? strtoul(colon + 1, 0, 16) : 0;
        int fixed = port == 0 || (port >= 7000 && port <= 7002) || port == 9000;
        add(out, " %.*s:%s", (int)(colon - w), w, fixed ? colon + 1 : "E");
        return;
    }
    if (column == 5) {
        // The timer and the time left on it.
        add(out, " %.3sT", w);
        return;
    }
    if (column == 9) {
        unsigned long ino = strtoul(w, &end, 10);
        for (int i = 0; i < count; i++)
            if (inodes[i] == ino && ino) {
                add(out, " fd%d", fds[i]);
                return;
            }
        add(out, " %s", ino ? "other" : "0");
        return;
    }
    if (column == 11) {
        add(out, " P");
        return;
    }
    add(out, " %s", w);
}

static void show(const char *path) {
    FILE *f = fopen(path, "r");
    if (!f) {
        printf("%s: %s\n", path, strerror(errno));
        return;
    }
    printf("== %s\n", path);
    char line[1024];
    int first = 1;
    int unix_file = strstr(path, "unix") != 0, udp = strstr(path, "udp") != 0;
    // Lines in the order the sockets were made, as each machine lists them in its own order.
    char *lines[64];
    int n = 0;
    while (fgets(line, sizeof line, f)) {
        size_t length = strlen(line);
        if (first) {
            printf("header %zu: %s", length, line);
            first = 0;
            continue;
        }
        char out[1024] = "";
        // The length without the inode number, whose digits differ, where lines are not padded
        // to a width (IPv4's are); a local socket's is at least five wide.
        char copy[1024];
        strcpy(copy, line);
        char *save, *w = strtok_r(copy, " \n", &save);
        int padded = !unix_file && !strstr(path, "6");
        for (int column = 0; w && !padded; column++, w = strtok_r(0, " \n", &save))
            if (column == (unix_file ? 6 : 9))
                length -= unix_file && strlen(w) < 5 ? 5 : strlen(w);
        add(out, "%zu:", length);
        w = strtok_r(line, " \n", &save);
        for (int column = 0; w; column++, w = strtok_r(0, " \n", &save))
            word(out, w, column, unix_file, udp);
        lines[n++] = strdup(out);
    }
    fclose(f);
    // Sorted, since the order is that of each machine's hash tables.
    for (int i = 0; i < n; i++)
        for (int j = i + 1; j < n; j++)
            if (strcmp(lines[i], lines[j]) > 0) {
                char *t = lines[i];
                lines[i] = lines[j];
                lines[j] = t;
            }
    for (int i = 0; i < n; i++)
        printf("%s\n", lines[i]);
}

static void raw(const char *path) {
    FILE *f = fopen(path, "r");
    if (!f) {
        printf("%s: %s\n", path, strerror(errno));
        return;
    }
    printf("== %s\n", path);
    char line[1024];
    while (fgets(line, sizeof line, f))
        printf("%zu: %s", strlen(line), line);
    fclose(f);
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    int l = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un a = {AF_UNIX, "/tmp/l.sock"};
    bind(l, (void *)&a, sizeof a);
    listen(l, 5);
    track(l);
    int c = socket(AF_UNIX, SOCK_STREAM, 0);
    connect(c, (void *)&a, sizeof a);
    track(c);
    int sp[2];
    socketpair(AF_UNIX, SOCK_DGRAM, 0, sp);
    track(sp[0]);
    track(sp[1]);
    int d = socket(AF_UNIX, SOCK_DGRAM, 0);
    struct sockaddr_un ab = {AF_UNIX};
    memcpy(ab.sun_path, "\0ab\0c", 5);
    bind(d, (void *)&ab, 2 + 5);
    track(d);
    int u = socket(AF_UNIX, SOCK_SEQPACKET, 0);
    track(u);

    struct sockaddr_in t = {AF_INET, htons(7000)};
    t.sin_addr.s_addr = htonl(0x7f000001);
    int tl = socket(AF_INET, SOCK_STREAM, 0);
    bind(tl, (void *)&t, sizeof t);
    listen(tl, 5);
    track(tl);
    int tc = socket(AF_INET, SOCK_STREAM, 0);
    connect(tc, (void *)&t, sizeof t);
    write(tc, "hello", 5);
    track(tc);
    int tc2 = socket(AF_INET, SOCK_STREAM, 0);
    connect(tc2, (void *)&t, sizeof t);
    track(tc2);
    int ta = accept(tl, 0, 0);
    write(ta, "abc", 3);
    track(ta);
    int t6 = socket(AF_INET6, SOCK_STREAM, 0);
    struct sockaddr_in6 s6 = {AF_INET6, htons(7001)};
    bind(t6, (void *)&s6, sizeof s6);
    listen(t6, 5);
    track(t6);
    int c6 = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in t2 = {AF_INET, htons(7001)};
    t2.sin_addr.s_addr = htonl(0x7f000001);
    connect(c6, (void *)&t2, sizeof t2);
    track(c6);
    int ud = socket(AF_INET, SOCK_DGRAM, 0);
    struct sockaddr_in ua = {AF_INET, htons(9000)};
    bind(ud, (void *)&ua, sizeof ua);
    track(ud);
    int uc = socket(AF_INET, SOCK_DGRAM, 0);
    ua.sin_addr.s_addr = htonl(0x7f000001);
    connect(uc, (void *)&ua, sizeof ua);
    send(uc, "xyz", 3, 0);
    track(uc);
    int u6 = socket(AF_INET6, SOCK_DGRAM, 0);
    bind(u6, (void *)&s6, sizeof s6);
    track(u6);
    // A connection closed first from here, which holds its port in TIME_WAIT.
    int tw = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in t3 = {AF_INET, htons(7002)};
    t3.sin_addr.s_addr = htonl(0x7f000001);
    bind(tw, (void *)&t3, sizeof t3);
    listen(tw, 1);
    int twc = socket(AF_INET, SOCK_STREAM, 0);
    connect(twc, (void *)&t3, sizeof t3);
    int twa = accept(tw, 0, 0);
    close(twa);
    usleep(100000);
    close(twc);
    close(tw);
    // A connection whose other end finished sending, and one that finished its own.
    int hc = socket(AF_INET, SOCK_STREAM, 0);
    connect(hc, (void *)&t, sizeof t);
    int ha = accept(tl, 0, 0);
    shutdown(hc, SHUT_WR);
    usleep(100000);
    track(hc);
    track(ha);

    show("/proc/net/tcp");
    show("/proc/net/tcp6");
    show("/proc/net/udp");
    show("/proc/net/udp6");
    show("/proc/net/unix");
    raw("/proc/net/route");
    raw("/proc/net/if_inet6");
    FILE *f = fopen("/proc/net/dev", "r");
    char line[512];
    while (f && fgets(line, sizeof line, f))
        if (strstr(line, "|"))
            printf("%zu: %s", strlen(line), line);
        else if (strstr(line, " lo:") || strstr(line, " eth0:"))
            // The counters, which this machine's traffic moves, are left out.
            printf("%.*s\n", (int)(strchr(line, ':') - line + 1), line);
    if (f)
        fclose(f);
    struct stat st;
    char target[64] = "";
    readlink("/proc/net", target, sizeof target - 1);
    printf("/proc/net -> %s\n", target);
    stat("/proc/self/net", &st);
    printf("net mode %o links %lu uid %u\n", st.st_mode, (unsigned long)st.st_nlink, st.st_uid);
    stat("/proc/net/tcp", &st);
    printf("tcp mode %o links %lu size %lld\n", st.st_mode, (unsigned long)st.st_nlink,
           (long long)st.st_size);
    printf("snmp %d\n", open("/proc/net/nothing", O_RDONLY) < 0 ? errno : 0);
    return 0;
}
