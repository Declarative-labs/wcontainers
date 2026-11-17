// multibyte handling with backref could oob read
#include <stdlib.h>
#include <string.h>
#include <locale.h>
#include <regex.h>
#include "test.h"

int main(void)
{
	char buf[200];
	char pat[] = "\\(a\\|b\\|..*\\)_\\1\\(.\\)\\2";
	char input[] = "ab_\xf1\x81\x81\x81_\xf1\x81\x81\x81__";
	regex_t r;
	regmatch_t rm[3];
	int n;

	/* allows easy oob detection via valgrind */
	char *p = malloc(sizeof input);
	if (!p) {
		t_error("oom\n");
		return t_status;
	}
	memcpy(p, input, sizeof input);

	setlocale(0,"C.UTF-8");
	n = regcomp(&r, pat, 0);
	if (n) {
		regerror(n, &r, buf, sizeof buf);
		t_error("regcomp(%s) returned %d (%s) wanted 0\n", pat, n, buf);
	}
	n = regexec(&r, p, 3, rm, 0);
	if (n) {
		regerror(n, &r, buf, sizeof buf);
		t_error("/%s/ ~ \"%s\", regexec returned %d (%s), wanted 0\n",
			pat, p, n, buf);
	} else if (
	  rm[0].rm_so!=3 || rm[0].rm_eo!=14 ||
	  rm[1].rm_so!=3 || rm[1].rm_eo!=7 ||
	  rm[2].rm_so!=12 || rm[2].rm_eo!=13) {
		t_error("/%s/ ~ \"%s\", got (%ld,%ld)(%ld,%ld)(%ld,%ld), "
			"wanted (3,14)(3,7)(12,13)\n",
			pat, p,
			(long)rm[0].rm_so, (long)rm[0].rm_eo,
			(long)rm[1].rm_so, (long)rm[1].rm_eo,
			(long)rm[2].rm_so, (long)rm[2].rm_eo);
	}
	return t_status;
}
