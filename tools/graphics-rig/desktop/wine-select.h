/* Picks the tests of one of Wine's Direct3D test files to run: wine-tests.ps1 includes this before
   the file and makes each test its START_TEST runs conditional on wc_wanted(<its name>). Runs those
   named in the environment variable WC_WINE_ONLY (comma separated), or all when it is unset. */
#include <stdlib.h>
#include <string.h>

static int wc_wanted(const char *name)
{
    const char *only = getenv("WC_WINE_ONLY"), *at;
    size_t length = strlen(name);

    if (!only)
        return 1;
    for (at = only; (at = strstr(at, name)); at += length)
        if ((at == only || at[-1] == ',') && (at[length] == ',' || !at[length]))
            return 1;
    return 0;
}
