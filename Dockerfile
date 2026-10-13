
# syntax=docker/dockerfile:experimental

FROM debian:buster-slim

ENV DEBIAN_FRONTEND noninteractive

ARG HTTPS_PROXY
ARG HTTP_PROXY
ARG CC=gcc
ARG TARGETARCH

ENV http_proxy $HTTP_PROXY
ENV https_proxy $HTTPS_PROXY

RUN sed -i 's|deb.debian.org|archive.debian.org|g; /buster-updates/d' /etc/apt/sources.list && \
    apt-get update && apt-get install build-essential $CC curl git pkg-config -y && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain 1.94.1 -y
ENV PATH "/root/.cargo/bin:${PATH}"
ENV RUSTUP_TOOLCHAIN=1.94.1

RUN if [ -n "$HTTP_PROXY" ]; then echo "[http]\n\
proxy = \"${HTTP_PROXY}\"\n\
"\
>> /root/.cargo/config ; fi

COPY . /tproxy-build
WORKDIR /tproxy-build
RUN --mount=type=cache,id=tproxy-target-${TARGETARCH},target=/tproxy-build/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo test --locked -p chaos-tproxy-proxy --lib && \
    cargo test --locked -p chaos-tproxy-controller --lib proxy::config::tests && \
    cargo test --locked -p chaos-tproxy-controller --lib proxy::net::bridge::tests && \
    cargo build --locked --release --all

RUN --mount=type=cache,id=tproxy-target-${TARGETARCH},target=/tproxy-build/target \
    cp /tproxy-build/target/release/chaos-tproxy /tproxy
