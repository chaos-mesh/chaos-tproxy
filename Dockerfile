# syntax = docker/dockerfile:1.0-experimental

FROM ubuntu:20.04 as builder
RUN ln -fs /usr/share/zoneinfo/America/New_York /etc/localtime

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates curl file libssl-dev \
    build-essential \
    autoconf automake autotools-dev libtool xutils-dev \
    pkgconf cmake && \
    rm -rf /var/lib/apt/lists/*

# install toolchain
RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain nightly-2021-03-16 -y
ENV PATH=/root/.cargo/bin:$PATH

WORKDIR /src
COPY src ./src
COPY Cargo.* ./
COPY rust-toolchain ./
RUN --mount=type=cache,id=tproxy_cargo_pkg,target=/root/.cargo cargo build --release

COPY iptables* ./
COPY example ./example

FROM ubuntu:20.04
RUN apt-get update && apt-get --no-install-recommends -y install iptables iproute2 nodejs npm vim 
RUN npm install http-server
WORKDIR /root
COPY --from=builder /src/target/release/tproxy tproxy
COPY --from=builder /src/iptables.sh iptables.sh
COPY --from=builder /src/example example
ENTRYPOINT ["npx", "http-server"]
