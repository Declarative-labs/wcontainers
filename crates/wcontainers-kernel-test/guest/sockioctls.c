// Socket ioctls' order of checks: SIOCATMARK on a datagram socket (also given a bad address), and
// the requests that change the network on an inet and a local socket, given a bad address (copied
// before the power is checked) or a good one.
#define _GNU_SOURCE
#include <errno.h>
#include <net/if.h>
#include <net/route.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <unistd.h>

static void p(const char *what, int r) { printf("%s: %d %s\n", what, r, r < 0 ? strerror(errno) : ""); }

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    int udp = socket(AF_INET, SOCK_DGRAM, 0), tcp = socket(AF_INET, SOCK_STREAM, 0), unix_dgram = socket(AF_UNIX, SOCK_DGRAM, 0);
    int unix_seqpacket = socket(AF_UNIX, SOCK_SEQPACKET, 0), unix_stream = socket(AF_UNIX, SOCK_STREAM, 0);
    int at;
    p("atmark udp", ioctl(udp, SIOCATMARK, &at));
    p("atmark tcp", ioctl(tcp, SIOCATMARK, &at));
    p("atmark unix datagram", ioctl(unix_dgram, SIOCATMARK, &at));
    p("atmark unix seqpacket", ioctl(unix_seqpacket, SIOCATMARK, &at));
    p("atmark unix stream", ioctl(unix_stream, SIOCATMARK, &at));
    p("atmark udp bad address", ioctl(udp, SIOCATMARK, (void *)-4096L));
    struct ifreq ifr;
    memset(&ifr, 0, sizeof ifr);
    strcpy(ifr.ifr_name, "lo");
    unsigned long requests[] = {SIOCSIFFLAGS, SIOCSIFMTU, SIOCSIFADDR, SIOCADDRT, SIOCDELRT, SIOCSARP, SIOCDARP, SIOCADDMULTI};
    const char *names[] = {"siocsifflags", "siocsifmtu", "siocsifaddr", "siocaddrt", "siocdelrt", "siocsarp", "siocdarp", "siocaddmulti"};
    int sockets[] = {udp, unix_stream};
    const char *kinds[] = {"udp", "unix"};
    for (int k = 0; k < 2; k++)
        for (int i = 0; i < 8; i++) {
            char what[64];
            snprintf(what, sizeof what, "%s %s bad address", kinds[k], names[i]);
            p(what, ioctl(sockets[k], requests[i], 0));
            snprintf(what, sizeof what, "%s %s", kinds[k], names[i]);
            static char room[256];
            memcpy(room, &ifr, sizeof ifr);
            p(what, ioctl(sockets[k], requests[i], room));
        }
    return 0;
}
