#!/bin/sh
# Builds the Linux Test Project's image for ARM64 Linux (wc-kernel-ltp:<release>) and keeps its
# files, as `docker export` gives them, in the scratch directory for runs on our kernel.
# Usage: build.sh <scratch directory>
set -eu
release=20260930
scratch=${1:?usage: build.sh <scratch directory>}
here=$(cd "$(dirname "$0")" && pwd)
image="wc-kernel-ltp:$release"
docker build --platform linux/arm64 --build-arg LTP_RELEASE="$release" -t "$image" "$here"
mkdir -p "$scratch"
container=$(docker create --platform linux/arm64 "$image")
docker export "$container" > "$scratch/ltp-$release.tar.partial"
docker rm -f "$container" > /dev/null
mv "$scratch/ltp-$release.tar.partial" "$scratch/ltp-$release.tar"
echo "$image; files in $scratch/ltp-$release.tar"
