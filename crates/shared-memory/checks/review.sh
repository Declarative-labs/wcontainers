# review.sh: checks that REVIEW.md and the comments match the code and the logs: the count of unsafe items, every
# checks/ file cited exists, no measurement cites REVIEW.md in place of a log, and every log of a run names the code as
# it is now. Run from the checkout's top folder; prints each failure and ends with its count.
set -u
crate=crates/shared-memory
failures=0
fail() { echo "FAIL: $*"; failures=$((failures + 1)); }

marked=$(cat $crate/foreign_implementation/region/implementation.rs $crate/foreign_implementation/host/implementation.rs | grep -c 'unsafe_code')
stated=$(grep -o 'Every unsafe item: [0-9]*' $crate/REVIEW.md | grep -o '[0-9]*$')
[ "$marked" = "$stated" ] || fail "REVIEW.md states $stated unsafe items in a host build; the code marks $marked"
listed=$(sed -n '/^## 2\./,/^## 3\./p' $crate/REVIEW.md | grep -cE '^[0-9]+\. ')
[ "$marked" = "$listed" ] || fail "REVIEW.md lists $listed unsafe items; the code marks $marked"
uncited=$(sed -n '/^## 2\./,/^## 3\./p' $crate/REVIEW.md | grep -E '^[0-9]+\. ' | grep -vcE 'implementation\.rs:[0-9]+')
[ "$uncited" = 0 ] || fail "$uncited unsafe items of REVIEW.md give no file:line"
for cite in $(sed -n '/^## 2\./,/^## 3\./p' $crate/REVIEW.md | grep -E '^[0-9]+\. ' | grep -oE '(region|host)/implementation\.rs:[0-9]+(/[0-9]+)?'); do
  file=$crate/foreign_implementation/${cite%%:*} lines=${cite##*:}
  for line in ${lines//\// }; do
    sed -n "${line}p" "$file" | grep -q unsafe || fail "REVIEW.md cites ${cite%%:*}:$line, a line with no unsafe"
  done
done

for cited in $(grep -rhoE 'checks/[A-Za-z0-9_.-]+\.(log|sh)' --include='*.rs' --include='*.toml' --include='*.md' $crate | sort -u); do
  [ -e "$crate/$cited" ] || fail "$cited is cited and does not exist"
done

grep -rn 'MEASURED, REVIEW.md' --include='*.rs' --include='*.toml' $crate && fail "a measurement cites REVIEW.md, not a log"

now=$(bash $crate/checks/tree.sh)
for log in $crate/checks/x86.log $crate/checks/arm.log; do
  runs=$(grep -c '^code ' "$log")
  [ "$runs" -gt 0 ] || fail "$log names no code"
  stale=$(grep '^code ' "$log" | grep -vc "^code $now\$")
  [ "$stale" = 0 ] || fail "$log has $stale runs on code other than $now"
done

echo "review checks: code $now, $marked unsafe items, $failures failed"
[ "$failures" = 0 ]
