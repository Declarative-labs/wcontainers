#!/bin/sh
# Builds the Linux Test Project's image for ARM64 Linux (wc-kernel-ltp:<release>), the image whose
# /srv holds its files for the file backend to serve as our kernel's root
# (wc-kernel-ltp-served:<release>), and keeps its files, as `docker export` gives them, in the
# scratch directory.
# Usage: build.sh <scratch directory>
set -eu
release=20260930
scratch=${1:?usage: build.sh <scratch directory>}
here=$(cd "$(dirname "$0")" && pwd)
image="wc-kernel-ltp:$release"
docker build --platform linux/arm64 --build-arg LTP_RELEASE="$release" -t "$image" "$here"
# The backend's server runs on alpine:3.22, as in the other tests; /.dockerenv is what docker adds
# to a container's root.
printf 'FROM %s AS ltp\nFROM alpine:3.22\nCOPY --from=ltp / /srv/\nRUN touch /srv/.dockerenv\n' "$image" |
    docker build --platform linux/arm64 -t "wc-kernel-ltp-served:$release" -
mkdir -p "$scratch"
container=$(docker create --platform linux/arm64 "$image")
docker export "$container" > "$scratch/ltp-$release.tar.partial"
docker rm -f "$container" > /dev/null
mv "$scratch/ltp-$release.tar.partial" "$scratch/ltp-$release.tar"
echo "$image; files in $scratch/ltp-$release.tar"
