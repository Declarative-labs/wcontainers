# tree.sh: prints one hash of every file of the crate but its logs and REVIEW.md, so each log names the code it ran on.
# Run from the checkout's top folder, on the Mac or on a rented machine.
set -eu
sum() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }
find crates/shared-memory -type f ! -name '*.log' ! -name REVIEW.md ! -path '*/target/*' | LC_ALL=C sort | while read -r file; do
  sum "$file"
done | sum | cut -c1-16
