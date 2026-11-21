# The Linux Test Project at a pinned release, built for ARM64 Linux with glibc (the C library the
# project is written and tested for), installed at /opt/ltp in an image that runs its tests.
# The base image is pinned by digest, Debian's packages by a snapshot of its archive, the release
# by its checksum, and the packages the tests load by version.
ARG BASE=debian:trixie@sha256:9c6e381421d0f48d1b8133e573f33c185ee16cbf505eda9c4a8a4791b25ea8a6
ARG SNAPSHOT=20261117T210000Z

FROM ${BASE} AS pinned
ARG SNAPSHOT
RUN printf '%s\n' \
        'Types: deb' \
        "URIs: http://snapshot.debian.org/archive/debian/${SNAPSHOT}" \
        'Suites: trixie trixie-updates' \
        'Components: main' \
        'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
        '' \
        'Types: deb' \
        "URIs: http://snapshot.debian.org/archive/debian-security/${SNAPSHOT}" \
        'Suites: trixie-security' \
        'Components: main' \
        'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
        > /etc/apt/sources.list.d/debian.sources \
    && echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/snapshot

FROM pinned AS build
ARG LTP_RELEASE
ARG LTP_SHA256=b9837481134fa6eabce91c2fba57d96695f81360468f83e11e6982584ad4d29d
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils make gcc libc6-dev autoconf automake m4 pkgconf bison flex \
        libaio-dev libacl1-dev libattr1-dev libcap-dev libkeyutils-dev libnuma-dev libmnl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /ltp.tar.xz \
        "https://github.com/linux-test-project/ltp/releases/download/${LTP_RELEASE}/ltp-full-${LTP_RELEASE}.tar.xz" \
    && echo "${LTP_SHA256}  /ltp.tar.xz" | sha256sum -c - \
    && mkdir /src && tar -xJf /ltp.tar.xz -C /src --strip-components=1
WORKDIR /src
RUN ./configure --prefix=/opt/ltp && make -j6 all && make install \
    && find /opt/ltp -type f -perm -u+x -exec sh -c 'file="$1"; head -c 4 "$file" | grep -q ELF && strip "$file" || true' _ {} \;

FROM pinned
RUN apt-get update && apt-get install -y --no-install-recommends \
        libaio1t64=0.3.113-8+b1 libacl1=2.3.2-2+b1 libattr1=1:2.5.2-3 \
        libcap2=1:2.75-10+deb13u1+b3 libkeyutils1=1.6.3-6 libnuma1=2.0.19-1 libmnl0=1.0.5-3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /opt/ltp /opt/ltp
