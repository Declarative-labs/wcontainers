// File capabilities set and read back through getxattr on a file of the backend.
#include <errno.h>
#include <fcntl.h>
#include <linux/capability.h>
#include <stdio.h>
#include <string.h>
#include <sys/xattr.h>
#include <unistd.h>

static void result(const char *what, long value) {
    if (value < 0) {
        printf("%s: -1 %s\n", what, strerror(errno));
    } else {
        printf("%s: %ld\n", what, value);
    }
}

int main(void) {
    setvbuf(stdout, 0, _IONBF, 0);
    close(open("caps", O_CREAT | O_RDWR | O_TRUNC, 0644));
    struct vfs_cap_data caps = {.magic_etc = VFS_CAP_REVISION_2 | VFS_CAP_FLAGS_EFFECTIVE};
    caps.data[0].permitted = 1 << CAP_NET_RAW;
    result("set file capabilities", setxattr("caps", "security.capability", &caps, sizeof caps, 0));
    struct vfs_cap_data back;
    memset(&back, 0, sizeof back);
    result("get file capabilities", getxattr("caps", "security.capability", &back, sizeof back));
    printf("  magic %x permitted %x\n", back.magic_etc, back.data[0].permitted);
    result("file capabilities length", getxattr("caps", "security.capability", 0, 0));
    return 0;
}
