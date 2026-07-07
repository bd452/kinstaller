# Linux build environment for Kindle device binaries (koxtoolchain + Rust).
#
# Used from macOS via OrbStack/Docker: ./scripts/build-in-container.sh kindlehf
#
# koxtoolchain host tools are Linux x86_64 — force amd64 even on Apple Silicon Macs.
FROM --platform=linux/amd64 ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    file \
    git \
    wget \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/rustup/bin:$PATH \
    KOXTOOLCHAIN_ROOT=/opt/x-tools/x-tools

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup target add \
        armv7-unknown-linux-gnueabihf \
        armv7-unknown-linux-gnueabi

# KindleModding cross toolchains (same tarballs as KPM CI).
RUN mkdir -p /opt/x-tools \
    && wget -q https://github.com/KindleModding/koxtoolchain/releases/latest/download/kindlehf.tar.gz -O - \
        | tar -xzf - -C /opt/x-tools \
    && wget -q https://github.com/KindleModding/koxtoolchain/releases/latest/download/kindlepw2.tar.gz -O - \
        | tar -xzf - -C /opt/x-tools

WORKDIR /work

# Warm-check toolchains at image build time.
RUN /opt/x-tools/x-tools/arm-kindlehf-linux-gnueabihf/bin/arm-kindlehf-linux-gnueabihf-gcc --version \
    && /opt/x-tools/x-tools/arm-kindlepw2-linux-gnueabi/bin/arm-kindlepw2-linux-gnueabi-gcc --version
