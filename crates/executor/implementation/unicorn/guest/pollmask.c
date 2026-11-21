// What poll and select report for each kind of file one process can open: devices, tmpfs files and
// directories, /proc files and directories (those built), memory files, O_PATH descriptors and
// fresh sockets.

#define _GNU_SOURCE
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <linux/netlink.h>
#include <netinet/in.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

static void show(const char *name, int fd) {
    if (fd < 0) {
        printf("%s: not opened\n", name);
        return;
    }
    struct pollfd all = {fd, (short)0xffff, 0};
    int polled = poll(&all, 1, 0);
    // Twice: some files report a change only on the first poll after it.
    struct pollfd again = {fd, (short)0xffff, 0};
    poll(&again, 1, 0);
    fd_set in, out, ex;
    FD_ZERO(&in);
    FD_ZERO(&out);
    FD_ZERO(&ex);
    FD_SET(fd, &in);
    FD_SET(fd, &out);
    FD_SET(fd, &ex);
    struct timeval none = {0, 0};
    int selected = select(fd + 1, &in, &out, &ex, &none);
    printf("%s: poll %d revents %#x then %#x, select %d in %d out %d ex %d\n", name, polled,
           (unsigned short)all.revents, (unsigned short)again.revents, selected,
           FD_ISSET(fd, &in) != 0, FD_ISSET(fd, &out) != 0, FD_ISSET(fd, &ex) != 0);
    close(fd);
}

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    mkdir("/dev/shm/p", 0700);
    int file = open("/dev/shm/p/f", O_RDWR | O_CREAT, 0600);
    close(file);
    const char *paths[] = {
        "/dev/null", "/dev/zero", "/dev/full", "/dev/random", "/dev/urandom", "/dev/shm/p/f",
        "/dev/shm/p", "/dev", "/dev/pts", "/proc/self/comm", "/proc/self/cmdline",
        "/proc/self/mounts", "/proc/self/status", "/proc/self/maps",
        "/proc/self/limits", "/proc/sys/kernel/hostname",
        "/proc/sys/kernel/ostype", "/proc/self", "/proc", "/proc/self/fd", "/proc/meminfo",
        "/proc/cpuinfo", "/proc/uptime", "/proc/version",
    };
    for (unsigned i = 0; i < sizeof paths / sizeof *paths; i++) {
        show(paths[i], open(paths[i], O_RDONLY));
    }
    show("/dev/random written", open("/dev/random", O_WRONLY));
    show("/dev/shm/p/f written", open("/dev/shm/p/f", O_WRONLY));
    show("/dev/shm/p/f O_PATH", open("/dev/shm/p/f", O_PATH));
    show("memfd", (int)syscall(SYS_memfd_create, "m", 0));
    show("secret memory", (int)syscall(447, 0));
    show("netlink", socket(AF_NETLINK, SOCK_RAW, 0));
    show("udp", socket(AF_INET, SOCK_DGRAM, 0));
    show("tcp", socket(AF_INET6, SOCK_STREAM, 0));
    return 0;
}
