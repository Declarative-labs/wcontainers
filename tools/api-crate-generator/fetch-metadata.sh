#!/bin/bash
# Fetches the generator's input, Microsoft's Win32 metadata (Microsoft.Windows.SDK.Win32Metadata 63.0.31, MIT or
# Apache-2.0), as the windows-bindgen 0.66.0 package on crates.io ships it, checks both checksums, and prints the path.
#   fetch-metadata.sh <folder>
set -euo pipefail
folder=$1
package=windows-bindgen-0.66.0
# The package's checksum as the crates.io index lists it.
package_sha256=81b7ec123a4eadd44d1f44f76804316b477b2537abed9a2ab950b3c54afa1fcf
winmd_sha256=97d24cf1a9dc3e50782bbf1dba0952bf6a025fa583d8b3ae6c5ef713b463c869
winmd=$folder/$package/default/Windows.Win32.winmd
mkdir -p "$folder"
if ! echo "$winmd_sha256  $winmd" | sha256sum -c --status 2>/dev/null; then
    curl -sSfL -o "$folder/$package.crate" "https://static.crates.io/crates/windows-bindgen/$package.crate"
    echo "$package_sha256  $folder/$package.crate" | sha256sum -c --status
    tar xzf "$folder/$package.crate" -C "$folder" "$package/default/Windows.Win32.winmd"
    echo "$winmd_sha256  $winmd" | sha256sum -c --status
fi
echo "$winmd"
