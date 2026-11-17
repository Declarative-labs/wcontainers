#include <regex.h>
#include "test.h"

#define T(c) ((c) || (t_error(#c " failed\n"),0))

int main(void)
{
	regex_t re;
	regmatch_t rm[2] = {0};
	int c;

	c = T(regcomp(&re, "\\(.\\{1,3\\}\\)\\1", 0) == 0);
	if (c) {
		T(regexec(&re, "foo", 2, rm, 0) == 0);
		c = T(rm[1].rm_so == 1 && rm[1].rm_eo == 2);
		if (!c)
			t_error("got rm_so %ld rm_eo %ld\n", (long)rm[1].rm_so, (long)rm[1].rm_eo);
		regfree(&re);
	}

	c = T(regcomp(&re, "\\(..*.\\)\\1", 0) == 0);
	if (c) {
		T(regexec(&re, "aababa", 2, rm, 0) == 0);
		c = T(rm[1].rm_so == 1 && rm[1].rm_eo == 3);
		if (!c)
			t_error("got rm_so %ld rm_eo %ld\n", (long)rm[1].rm_so, (long)rm[1].rm_eo);
		regfree(&re);
	}

	return t_status;
}
