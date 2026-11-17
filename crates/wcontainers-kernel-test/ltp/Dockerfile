# The Linux Test Project at a pinned release, built for ARM64 Linux with glibc (the C library the
# project is written and tested for), installed at /opt/ltp in an image that runs its tests.
ARG BASE=debian:trixie@sha256:9c6e381421d0f48d1b8133e573f33c185ee16cbf505eda9c4a8a4791b25ea8a6

FROM ${BASE} AS build
ARG LTP_RELEASE
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils make gcc libc6-dev autoconf automake m4 pkgconf bison flex \
        libaio-dev libacl1-dev libattr1-dev libcap-dev libkeyutils-dev libnuma-dev libmnl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /ltp.tar.xz \
        "https://github.com/linux-test-project/ltp/releases/download/${LTP_RELEASE}/ltp-full-${LTP_RELEASE}.tar.xz" \
    && mkdir /src && tar -xJf /ltp.tar.xz -C /src --strip-components=1
WORKDIR /src
RUN ./configure --prefix=/opt/ltp && make -j6 all && make install \
    && find /opt/ltp -type f -perm -u+x -exec sh -c 'file="$1"; head -c 4 "$file" | grep -q ELF && strip "$file" || true' _ {} \;

FROM ${BASE}
RUN apt-get update && apt-get install -y --no-install-recommends \
        libaio1t64 libacl1 libattr1 libcap2 libkeyutils1 libnuma1 libmnl0 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /opt/ltp /opt/ltp
