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

ENV RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup
# install toolchain
RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- -y
ENV PATH=/root/.cargo/bin:$PATH

WORKDIR /src
COPY src/main.rs ./src/main.rs
COPY Cargo.* ./
COPY rust-toolchain ./
RUN cargo fetch

COPY src ./src
RUN --mount=type=cache,id=tproxy_cargo_pkg,target=/src/target \ 
    cargo build --release

COPY iptables* ./
COPY config-examples ./config-examples

FROM ubuntu:20.04
RUN apt-get update && apt-get --no-install-recommends -y install iptables iproute2 nodejs npm vim 
RUN npm install http-server
WORKDIR /root
COPY --from=builder /src/target/release/tproxy tproxy
COPY --from=builder /src/iptables.sh iptables.sh
COPY --from=builder /src/config-examples config-examples
ENTRYPOINT ["npx", "http-server"]
