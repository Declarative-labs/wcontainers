/* A handler of one argument for SIGTRAP, installed before main by this library, which the earlier-action test preloads
   in a child: the handler ends the process with a status of its own. */
#include <signal.h>
#include <unistd.h>

enum { ONE_ARGUMENT_EXIT = 71 };

static void taken(int signal) {
    (void)signal;
    _exit(ONE_ARGUMENT_EXIT);
}

__attribute__((constructor)) static void install(void) {
    struct sigaction action = {0};
    action.sa_handler = taken;
    sigemptyset(&action.sa_mask);
    sigaction(SIGTRAP, &action, 0);
}
