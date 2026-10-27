# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/devcontainers/base:trixie

ARG TARGETARCH

RUN rm -f /etc/apt/apt.conf.d/docker-clean
RUN --mount=type=cache,id=tproxy-dev-apt-trixie-${TARGETARCH},target=/var/cache/apt,sharing=locked \
    --mount=type=cache,id=tproxy-dev-apt-lists-trixie-${TARGETARCH},target=/var/lib/apt/lists,sharing=locked \
    apt-get -o Acquire::Retries=3 update && \
    apt-get -o Acquire::Retries=3 install -y --no-install-recommends \
        build-essential \
        ebtables \
        iproute2 \
        iptables \
        libpcap-dev \
        net-tools \
        netbase \
        pkg-config \
        procps \
        python3
