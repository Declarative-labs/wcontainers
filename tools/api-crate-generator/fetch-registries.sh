#!/bin/bash
# Fetches the Khronos front end's inputs at pinned commits and checks each checksum: Khronos's gl.xml and egl.xml
# (Apache-2.0) and ANGLE's gl_angle_ext.xml (BSD-3-Clause), which declares ANGLE's own extensions; prints the three paths.
#   fetch-registries.sh <folder>
set -euo pipefail
folder=$1
mkdir -p "$folder"
fetch() {
    local name=$1 url=$2 sha256=$3
    if ! echo "$sha256  $folder/$name" | sha256sum -c --status 2>/dev/null; then
        curl -sSfL -o "$folder/$name" "$url"
        echo "$sha256  $folder/$name" | sha256sum -c --status
    fi
    echo "$folder/$name"
}
fetch gl.xml https://raw.githubusercontent.com/KhronosGroup/OpenGL-Registry/6af574a14089ccfee87efe230ebcdd8742859813/xml/gl.xml \
    b9ca2cfa5c676e901c20d34af3407f1687cde0f1336a5ff7a8974d04c7494ad3
fetch egl.xml https://raw.githubusercontent.com/KhronosGroup/EGL-Registry/8e56e86e65687697ef54114cdb1afabdfac756bf/api/egl.xml \
    3a0f13e1b6309c8662bdb47046a4515c092b91b53e6774b46cd702b67ec53626
fetch gl_angle_ext.xml https://raw.githubusercontent.com/google/angle/6df81ae13d4ce5c42f9b158f014e862948dec678/scripts/gl_angle_ext.xml \
    a5424e89aff2eea1e81937b110a31e3cc3bb7e0746e5882c37ff2ff0928630b9
