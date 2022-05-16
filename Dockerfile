
# syntax=docker/dockerfile:experimental

FROM debian:buster-slim

ENV DEBIAN_FRONTEND noninteractive

ARG HTTPS_PROXY
ARG HTTP_PROXY
ARG CC=gcc

ENV http_proxy $HTTP_PROXY
ENV https_proxy $HTTPS_PROXY

RUN apt-get update && apt-get install build-essential $CC curl git pkg-config -y && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH "/root/.cargo/bin:${PATH}"

RUN if [ -n "$HTTP_PROXY" ]; then echo "[http]\n\
proxy = \"${HTTP_PROXY}\"\n\
"\
>> /root/.cargo/config ; fi

COPY . /tproxy-build
WORKDIR /tproxy-build
RUN --mount=type=cache,target=/tproxy-build/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release --all

RUN --mount=type=cache,target=/tproxy-build/target \
    cp /tproxy-build/target/release/chaos-tproxy /tproxy
