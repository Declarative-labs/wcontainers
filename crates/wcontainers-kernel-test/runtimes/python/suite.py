# Runs modules of CPython's own test suite (from the source release matching the image, whose
# image leaves the `test` package out) and prints each test's outcome, without times or messages.
import io
import os
import re
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
os.chdir("/tmp")

from test import support

# The suite's warnings name process numbers, which in the reference start after those runc's
# own threads took in the container; the numbers are left out of the comparison.
warn = support.print_warning
support.print_warning = lambda message: warn(re.sub(r"\d+", "N", message))


class Outcomes(unittest.TestResult):
    def __init__(self):
        super().__init__()
        self.lines = []

    def addSuccess(self, test):
        self.lines.append(f"{test.id()} ok")

    def addFailure(self, test, error):
        self.lines.append(f"{test.id()} FAIL")

    def addError(self, test, error):
        self.lines.append(f"{test.id()} ERROR")

    def addSkip(self, test, reason):
        self.lines.append(f"{test.id()} skipped: {reason}")

    def addExpectedFailure(self, test, error):
        self.lines.append(f"{test.id()} expected failure")

    def addUnexpectedSuccess(self, test):
        self.lines.append(f"{test.id()} unexpected success")

    def addSubTest(self, test, subtest, error):
        if error is not None:
            kind = "FAIL" if issubclass(error[0], test.failureException) else "ERROR"
            self.lines.append(f"{subtest.id()} {kind}")


for name in sys.argv[1:]:
    suite = unittest.defaultTestLoader.loadTestsFromName("test." + name)
    outcomes = Outcomes()
    # What the tests print themselves (process numbers among it) is not compared.
    printed, sys.stdout = sys.stdout, io.StringIO()
    try:
        suite.run(outcomes)
    finally:
        sys.stdout = printed
    for line in sorted(outcomes.lines):
        print(line)
    print(f"{name}: {outcomes.testsRun} run", flush=True)
