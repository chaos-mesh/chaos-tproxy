# syntax=docker/dockerfile:1

FROM rust:1.99.1-slim-bookworm

ENV DEBIAN_FRONTEND=noninteractive

ARG CC=gcc
ARG TARGETARCH

RUN rm -f /etc/apt/apt.conf.d/docker-clean && \
    echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' > /etc/apt/apt.conf.d/keep-cache
RUN --mount=type=cache,id=tproxy-apt-cache-bookworm-${TARGETARCH},target=/var/cache/apt,sharing=locked \
    --mount=type=cache,id=tproxy-apt-lists-bookworm-${TARGETARCH},target=/var/lib/apt/lists,sharing=locked \
    apt-get update && apt-get install -y build-essential $CC ca-certificates git pkg-config

COPY . /tproxy-build
WORKDIR /tproxy-build
RUN --mount=type=cache,id=tproxy-target-bookworm-${TARGETARCH},target=/tproxy-build/target,sharing=locked \
    --mount=type=cache,id=/root/.cargo/registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=/root/.cargo/git,target=/usr/local/cargo/git \
    cargo test --locked -p chaos-tproxy-proxy --lib && \
    cargo test --locked -p chaos-tproxy-controller --lib && \
    cargo build --locked --release --all && \
    cp /tproxy-build/target/release/chaos-tproxy /tproxy
