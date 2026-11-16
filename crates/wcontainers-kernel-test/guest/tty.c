// Pseudo-terminals: making a pair through /dev/ptmx, the slave's node, the default settings, the line
// discipline's input handling (canonical editing, echo, control characters, signals from keys, flow
// control, non-canonical reads), output processing, window size, packet mode, the queue sizes, and
// what each side sees when the other closes.
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <termios.h>
#include <unistd.h>

static void show(const char *label, const char *buf, long n) {
    printf("%s: %ld \"", label, n);
    for (long i = 0; i < n; i++) {
        unsigned char c = buf[i];
        if (c >= 32 && c < 127 && c != '"' && c != '\\')
            putchar(c);
        else
            printf("\\x%02x", c);
    }
    printf("\"\n");
}

static void result(const char *label, long r) {
    if (r < 0)
        printf("%s: errno %d\n", label, errno);
    else
        printf("%s: %ld\n", label, r);
}

static int master, slave;
static char name[64];

static void open_pair(void) {
    master = posix_openpt(O_RDWR | O_NOCTTY);
    if (master < 0) {
        result("posix_openpt", -1);
        exit(1);
    }
    result("grantpt", grantpt(master));
    result("unlockpt", unlockpt(master));
    ptsname_r(master, name, sizeof name);
    slave = open(name, O_RDWR | O_NOCTTY);
    result("open slave", slave < 0 ? -1 : 0);
    fcntl(master, F_SETFL, O_NONBLOCK | O_RDWR);
    fcntl(slave, F_SETFL, O_NONBLOCK | O_RDWR);
}

static void close_pair(void) {
    close(slave);
    close(master);
}

// Reads what the slave has, then what the master has, after `input` is typed.
static void type(const char *label, const char *input, long length) {
    char buf[512];
    printf("-- %s\n", label);
    if (length)
        result("write master", write(master, input, length));
    // Every line the slave has; the first read that finds nothing waits for the input to be taken in.
    long n;
    while ((n = read(slave, buf, sizeof buf)) >= 0)
        show("read slave", buf, n);
    result("read slave", n);
    n = read(master, buf, sizeof buf);
    if (n < 0)
        result("read master", n);
    else
        show("read master", buf, n);
}

#define TYPE(label, text) type(label, text, sizeof(text) - 1)

static void drain(void) {
    char buf[512];
    while (read(slave, buf, sizeof buf) > 0) {
    }
    while (read(master, buf, sizeof buf) > 0) {
    }
}

static void set_flags(tcflag_t iflag_on, tcflag_t iflag_off, tcflag_t oflag_on, tcflag_t oflag_off,
                      tcflag_t lflag_on, tcflag_t lflag_off) {
    struct termios t;
    tcgetattr(slave, &t);
    t.c_iflag = (t.c_iflag | iflag_on) & ~iflag_off;
    t.c_oflag = (t.c_oflag | oflag_on) & ~oflag_off;
    t.c_lflag = (t.c_lflag | lflag_on) & ~lflag_off;
    result("tcsetattr", tcsetattr(slave, TCSANOW, &t));
}

static void print_termios(const char *label, int fd) {
    struct termios t;
    memset(&t, 0, sizeof t);
    if (tcgetattr(fd, &t) < 0) {
        result(label, -1);
        return;
    }
    printf("%s: iflag %o oflag %o cflag %o lflag %o line %d cc", label, t.c_iflag, t.c_oflag,
           t.c_cflag, t.c_lflag, t.c_line);
    // The kernel's structure has 19 control characters.
    for (int i = 0; i < 19; i++)
        printf(" %d", t.c_cc[i]);
    printf(" speed %u %u\n", cfgetispeed(&t), cfgetospeed(&t));
}

static void making(void) {
    printf("== making\n");
    int m = posix_openpt(O_RDWR | O_NOCTTY);
    result("posix_openpt", m < 0 ? -1 : 0);
    int number;
    result("TIOCGPTN", ioctl(m, TIOCGPTN, &number));
    printf("number %d\n", number);
    char path[64];
    snprintf(path, sizeof path, "/dev/pts/%d", number);
    result("open locked slave", open(path, O_RDWR | O_NOCTTY));
    int locked = -1;
    result("TIOCGPTLCK", ioctl(m, TIOCGPTLCK, &locked));
    printf("locked %d\n", locked);
    int zero = 0;
    result("TIOCSPTLCK", ioctl(m, TIOCSPTLCK, &zero));
    struct stat st;
    result("stat slave", stat(path, &st));
    printf("slave mode %o uid %d gid %d rdev %u:%u ino %lu nlink %lu blksize %ld\n", st.st_mode,
           st.st_uid, st.st_gid, major(st.st_rdev), minor(st.st_rdev), (unsigned long)st.st_ino,
           (unsigned long)st.st_nlink, (long)st.st_blksize);
    result("fstat master", fstat(m, &st));
    printf("master mode %o rdev %u:%u ino %lu\n", st.st_mode, major(st.st_rdev), minor(st.st_rdev),
           (unsigned long)st.st_ino);
    char link[64];
    snprintf(path, sizeof path, "/proc/self/fd/%d", m);
    long n = readlink(path, link, sizeof link - 1);
    if (n > 0)
        show("master link", link, n);
    snprintf(path, sizeof path, "/dev/pts/%d", number);
    int s = open(path, O_RDWR | O_NOCTTY);
    result("open slave", s < 0 ? -1 : 0);
    unsigned int dev = 0;
    result("TIOCGDEV slave", ioctl(s, TIOCGDEV, &dev));
    printf("dev %u:%u\n", major(dev), minor(dev));
    result("TIOCGDEV master", ioctl(m, TIOCGDEV, &dev));
    printf("dev %u:%u\n", major(dev), minor(dev));
    result("TIOCGPTN slave", ioctl(s, TIOCGPTN, &number));
    int second = posix_openpt(O_RDWR | O_NOCTTY);
    int other;
    ioctl(second, TIOCGPTN, &other);
    printf("second number %d\n", other - number);
    DIR *dir = opendir("/dev/pts");
    struct dirent *entry;
    printf("listing:");
    while ((entry = readdir(dir)))
        printf(" %s/%d", entry->d_name, entry->d_type);
    printf("\n");
    closedir(dir);
    int peer = ioctl(m, TIOCGPTPEER, O_RDWR | O_NOCTTY);
    result("TIOCGPTPEER", peer < 0 ? -1 : 0);
    result("TIOCGPTPEER slave", ioctl(s, TIOCGPTPEER, O_RDWR));
    struct stat a, b;
    fstat(peer, &a);
    fstat(s, &b);
    printf("peer same %d\n", a.st_ino == b.st_ino && a.st_rdev == b.st_rdev);
    close(peer);
    print_termios("slave termios", s);
    print_termios("master termios", m);
    struct winsize ws;
    result("TIOCGWINSZ", ioctl(s, TIOCGWINSZ, &ws));
    printf("winsize %d %d %d %d\n", ws.ws_row, ws.ws_col, ws.ws_xpixel, ws.ws_ypixel);
    int disc = -1;
    result("TIOCGETD", ioctl(s, TIOCGETD, &disc));
    printf("disc %d\n", disc);
    result("TIOCGPGRP no ctty", ioctl(s, TIOCGPGRP, &disc));
    result("TIOCGPGRP master", ioctl(m, TIOCGPGRP, &disc));
    printf("pgrp %d\n", disc);
    result("TIOCGSID master", ioctl(m, TIOCGSID, &disc));
    result("TIOCSTI", ioctl(s, TIOCSTI, "x"));
    result("TIOCEXCL", ioctl(s, TIOCEXCL));
    int excl = -1;
    result("TIOCGEXCL", ioctl(s, TIOCGEXCL, &excl));
    printf("excl %d\n", excl);
    result("open exclusive", open(path, O_RDWR | O_NOCTTY));
    result("TIOCNXCL", ioctl(s, TIOCNXCL));
    result("isatty file", isatty(open("/etc/hostname", O_RDONLY)));
    printf("errno %d\n", errno);
    result("isatty slave", isatty(s));
    result("lseek slave", lseek(s, 0, SEEK_SET));
    result("TIOCSPTLCK slave", ioctl(s, TIOCSPTLCK, &zero));
    result("TCFLSH bad", ioctl(s, TCFLSH, 7));
    result("TCXONC bad", ioctl(s, TCXONC, 9));
    result("TIOCSIG bad", ioctl(m, TIOCSIG, SIGKILL));
    result("TIOCSIG slave", ioctl(s, TIOCSIG, SIGINT));
    result("TIOCSETD", ioctl(s, TIOCSETD, &zero));
    int one = 1;
    result("TIOCSETD 1", ioctl(s, TIOCSETD, &one));
    result("TCSBRK", ioctl(s, TCSBRK, 1));
    result("TIOCMGET", ioctl(s, TIOCMGET, &disc));
    result("TIOCSCTTY", ioctl(s, TIOCSCTTY, 0));
    result("TIOCNOTTY", ioctl(s, TIOCNOTTY));
    result("TIOCVHANGUP", ioctl(s, TIOCVHANGUP));
    result("vhangup", vhangup());
    close(s);
    close(m);
    result("stat after close", stat(path, &st));
    close(second);
}

static void editing(void) {
    printf("== editing\n");
    open_pair();
    TYPE("line", "abc\n");
    TYPE("partial", "abc");
    TYPE("rest", "def\n");
    TYPE("return", "ret\r");
    TYPE("erase", "ab\x7f" "c\n");
    TYPE("erase past start", "\x7f\x7f" "a\n");
    TYPE("kill", "abc\x15xy\n");
    TYPE("werase", "foo bar \x17" "baz\n");
    TYPE("control", "a\x01\x1b" "b\n");
    TYPE("erase control", "a\x01\x7f\n");
    TYPE("tab", "a\tb\x7f\x7f\n");
    TYPE("lnext", "a\x16\x03\x16\x7f" "b\n");
    TYPE("reprint", "abc\x12" "d\n");
    TYPE("eof with text", "abc\x04");
    TYPE("eof alone", "\x04");
    TYPE("eof then line", "\x04xyz\n");
    TYPE("two lines", "one\ntwo\n");
    TYPE("high byte", "\xff\xc3\xa9\n");
    printf("-- small reads\n");
    write(master, "abcdef\n", 7);
    char buf[8];
    long n = read(slave, buf, 3);
    show("read 3", buf, n);
    int queued = -1;
    result("TIOCINQ", ioctl(slave, TIOCINQ, &queued));
    printf("inq %d\n", queued);
    n = read(slave, buf, 8);
    show("read rest", buf, n);
    drain();
    printf("-- echo settings\n");
    set_flags(0, 0, 0, 0, 0, ECHOE);
    TYPE("no echoe", "ab\x7f\n");
    set_flags(0, 0, 0, 0, ECHOPRT, 0);
    TYPE("echoprt", "abc\x7f\x7f" "d\n");
    set_flags(0, 0, 0, 0, ECHOE, ECHOPRT | ECHOKE);
    TYPE("kill no echoke", "abc\x15" "d\n");
    set_flags(0, 0, 0, 0, 0, ECHOK);
    TYPE("kill no echok", "abc\x15" "d\n");
    set_flags(0, 0, 0, 0, ECHOK | ECHOKE, ECHOCTL);
    TYPE("no echoctl", "a\x01\x7f" "b\n");
    set_flags(0, 0, 0, 0, ECHOCTL | ECHONL, ECHO);
    TYPE("echonl", "secret\n");
    set_flags(0, 0, 0, 0, ECHO, ECHONL);
    set_flags(0, 0, 0, 0, 0, IEXTEN);
    TYPE("no iexten", "a\x17\x16" "b\n");
    set_flags(0, 0, 0, 0, IEXTEN, 0);
    printf("-- input flags\n");
    set_flags(INLCR | IGNCR, ICRNL, 0, 0, 0, 0);
    TYPE("inlcr igncr", "a\rb\n" "c\r");
    set_flags(ICRNL | ISTRIP | IUCLC, INLCR | IGNCR, 0, 0, 0, 0);
    TYPE("istrip iuclc", "AbC\xc1\r");
    set_flags(0, ISTRIP | IUCLC, 0, 0, 0, 0);
    printf("-- output\n");
    char out[] = "a\nb\tc\rd\be\n";
    result("write slave", write(slave, out, sizeof out - 1));
    TYPE("onlcr", "");
    set_flags(0, 0, XTABS | OCRNL | OLCUC, ONLCR, 0, 0);
    result("write slave", write(slave, "\tx\ry\n", 5));
    TYPE("xtabs ocrnl olcuc", "");
    set_flags(0, 0, ONOCR | ONLRET, OCRNL | OLCUC, 0, 0);
    result("write slave", write(slave, "\rab\r\n\r", 6));
    TYPE("onocr onlret", "");
    set_flags(0, 0, ONLCR, ONOCR | ONLRET | XTABS | OPOST, 0, 0);
    result("write slave", write(slave, "raw\n", 4));
    TYPE("no opost", "");
    set_flags(0, 0, OPOST, 0, 0, 0);
    TYPE("echo after output", "z\tq\x7f\x7f\n");
    close_pair();
}

static void raw_reads(void) {
    printf("== non-canonical\n");
    open_pair();
    struct termios t;
    tcgetattr(slave, &t);
    cfmakeraw(&t);
    t.c_cc[VMIN] = 1;
    t.c_cc[VTIME] = 0;
    result("raw", tcsetattr(slave, TCSANOW, &t));
    print_termios("raw termios", slave);
    TYPE("raw bytes", "a\x03\x7f\r\n\x04");
    tcgetattr(slave, &t);
    t.c_lflag |= ECHO | ISIG;
    tcsetattr(slave, TCSANOW, &t);
    TYPE("raw echo", "x\x01y");
    TYPE("partial line", "pq");
    t.c_lflag |= ICANON;
    result("canonical again", tcsetattr(slave, TCSANOW, &t));
    TYPE("pushed", "");
    TYPE("after push", "rs\n");
    t.c_lflag &= ~ICANON;
    t.c_cc[VMIN] = 0;
    t.c_cc[VTIME] = 0;
    tcsetattr(slave, TCSANOW, &t);
    int flags = fcntl(slave, F_GETFL);
    fcntl(slave, F_SETFL, flags & ~O_NONBLOCK);
    char buf[16];
    result("vmin 0 vtime 0 empty", read(slave, buf, sizeof buf));
    write(master, "ab", 2);
    result("vmin 0 vtime 0", read(slave, buf, sizeof buf));
    t.c_cc[VMIN] = 3;
    tcsetattr(slave, TCSANOW, &t);
    write(master, "abcd", 4);
    result("vmin 3", read(slave, buf, sizeof buf));
    t.c_cc[VMIN] = 0;
    t.c_cc[VTIME] = 1;
    tcsetattr(slave, TCSANOW, &t);
    result("vtime timeout", read(slave, buf, sizeof buf));
    fcntl(slave, F_SETFL, flags);
    drain();
    int pending;
    write(master, "xyz", 3);
    // A poll that finds nothing waits for the input to be taken in; FIONREAD does not.
    struct pollfd in = {.fd = slave, .events = POLLIN};
    result("poll slave", poll(&in, 1, 10000));
    result("FIONREAD", ioctl(slave, FIONREAD, &pending));
    printf("pending %d\n", pending);
    result("TCFLSH input", tcflush(slave, TCIFLUSH));
    result("read after flush", read(slave, buf, sizeof buf));
    drain();
    write(slave, "out", 3);
    result("TIOCOUTQ", ioctl(slave, TIOCOUTQ, &pending));
    printf("outq %d\n", pending);
    // A poll that finds nothing waits for what the slave wrote to arrive.
    struct pollfd p = {.fd = master, .events = POLLIN};
    result("poll master", poll(&p, 1, 10000));
    result("master FIONREAD", ioctl(master, FIONREAD, &pending));
    printf("master pending %d\n", pending);
    result("TCFLSH output", tcflush(slave, TCOFLUSH));
    result("read master after flush", read(master, buf, sizeof buf));
    close_pair();
}

static volatile sig_atomic_t got[65];
static void note(int signal) { got[signal]++; }

static void flow_and_size(void) {
    printf("== flow and size\n");
    open_pair();
    char buf[64];
    write(master, "\x13", 1);
    result("read slave", read(slave, buf, sizeof buf));
    result("write stopped", write(slave, "held", 4));
    result("read master", read(master, buf, sizeof buf));
    struct pollfd p = {.fd = slave, .events = POLLIN | POLLOUT};
    poll(&p, 1, 0);
    printf("stopped poll %x\n", p.revents);
    write(master, "\x11", 1);
    result("read slave", read(slave, buf, sizeof buf));
    result("write started", write(slave, "go", 2));
    long n = read(master, buf, sizeof buf);
    show("read master", buf, n);
    result("TCOOFF", tcflow(slave, TCOOFF));
    result("write after tcooff", write(slave, "x", 1));
    write(master, "\x11", 1);
    read(slave, buf, sizeof buf);
    result("write after start key", write(slave, "y", 1));
    result("TCOON", tcflow(slave, TCOON));
    result("write after tcoon", write(slave, "z", 1));
    n = read(master, buf, sizeof buf);
    show("read master", buf, n);
    result("TCIOFF", tcflow(slave, TCIOFF));
    n = read(master, buf, sizeof buf);
    show("read master", buf, n);
    result("TCION", tcflow(slave, TCION));
    n = read(master, buf, sizeof buf);
    show("read master", buf, n);
    struct termios t;
    tcgetattr(slave, &t);
    t.c_iflag |= IXANY;
    tcsetattr(slave, TCSANOW, &t);
    write(master, "\x13", 1);
    read(slave, buf, sizeof buf);
    write(master, "k", 1);
    read(slave, buf, sizeof buf);
    n = read(master, buf, sizeof buf);
    show("read master", buf, n);
    result("write after ixany", write(slave, "w", 1));
    n = read(master, buf, sizeof buf);
    show("read master", buf, n);
    struct winsize ws = {.ws_row = 24, .ws_col = 80, .ws_xpixel = 1, .ws_ypixel = 2};
    result("TIOCSWINSZ master", ioctl(master, TIOCSWINSZ, &ws));
    struct winsize got_ws;
    ioctl(slave, TIOCGWINSZ, &got_ws);
    printf("slave winsize %d %d %d %d\n", got_ws.ws_row, got_ws.ws_col, got_ws.ws_xpixel,
           got_ws.ws_ypixel);
    ioctl(master, TIOCGWINSZ, &got_ws);
    printf("master winsize %d %d\n", got_ws.ws_row, got_ws.ws_col);
    printf("-- packet mode\n");
    int one = 1;
    result("TIOCPKT", ioctl(master, TIOCPKT, &one));
    int mode = -1;
    result("TIOCGPKT", ioctl(master, TIOCGPKT, &mode));
    printf("packet %d\n", mode);
    write(slave, "data", 4);
    n = read(master, buf, sizeof buf);
    show("packet read", buf, n);
    tcflush(slave, TCIFLUSH);
    n = read(master, buf, sizeof buf);
    show("packet flush read", buf, n);
    p.fd = master;
    p.events = POLLIN | POLLPRI;
    tcflush(slave, TCOFLUSH);
    poll(&p, 1, 0);
    printf("packet poll %x\n", p.revents);
    n = read(master, buf, sizeof buf);
    show("packet flush write", buf, n);
    write(master, "\x13", 1);
    read(slave, buf, sizeof buf);
    n = read(master, buf, sizeof buf);
    show("packet stop", buf, n);
    write(master, "\x11", 1);
    read(slave, buf, sizeof buf);
    n = read(master, buf, sizeof buf);
    show("packet start", buf, n);
    tcgetattr(slave, &t);
    t.c_iflag &= ~IXON;
    tcsetattr(slave, TCSANOW, &t);
    n = read(master, buf, sizeof buf);
    show("packet noflow", buf, n);
    n = read(master, buf, sizeof buf);
    result("packet empty", n);
    close_pair();
}

static void closing(void) {
    printf("== closing\n");
    open_pair();
    char buf[16];
    write(slave, "left", 4);
    close(slave);
    long n = read(master, buf, sizeof buf);
    show("master read before", buf, n);
    result("master read after slave closed", read(master, buf, sizeof buf));
    struct pollfd p = {.fd = master, .events = POLLIN | POLLOUT};
    poll(&p, 1, 0);
    printf("master poll %x\n", p.revents);
    result("master write", write(master, "x\n", 2));
    slave = open(name, O_RDWR | O_NOCTTY | O_NONBLOCK);
    result("reopen slave", slave < 0 ? -1 : 0);
    result("slave read", read(slave, buf, sizeof buf));
    show("slave got", buf, 2);
    result("master read after reopen", read(master, buf, sizeof buf));
    int second = dup(slave);
    write(master, "kept\n", 5);
    close(master);
    result("slave read after master closed", read(slave, buf, sizeof buf));
    result("slave write", write(slave, "x", 1));
    p.fd = slave;
    poll(&p, 1, 0);
    printf("slave poll %x\n", p.revents);
    struct termios t;
    result("slave tcgetattr", tcgetattr(slave, &t));
    int pgrp = 0;
    result("slave TIOCSPGRP", ioctl(slave, TIOCSPGRP, &pgrp));
    struct stat st;
    result("stat slave", stat(name, &st));
    result("open slave", open(name, O_RDWR));
    result("fstat slave", fstat(second, &st));
    printf("nlink %lu\n", (unsigned long)st.st_nlink);
    close(slave);
    close(second);
}

// Runs every part, or only the one named.
int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    static const struct {
        const char *name;
        void (*run)(void);
    } parts[] = {{"making", making}, {"editing", editing}, {"raw", raw_reads},
                 {"flow", flow_and_size}, {"closing", closing}};
    for (unsigned i = 0; i < sizeof parts / sizeof parts[0]; i++)
        if (argc < 2 || strcmp(argv[1], parts[i].name) == 0)
            parts[i].run();
    return 0;
}
