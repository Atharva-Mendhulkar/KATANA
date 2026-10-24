# Dockerfile - Symlink / alias to Containerfile (PRD §27, §31)
FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    clang \
    llvm \
    libbpf-dev \
    linux-libc-dev \
    pkg-config \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal

WORKDIR /build/katana
COPY . .

RUN bash scripts/check-scope.sh
RUN cargo test --verbose
RUN cargo build --release

FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libbpf1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/katana/target/release/katana /usr/local/bin/katana
COPY --from=builder /build/katana/fixtures /usr/local/share/katana/fixtures

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/katana"]
CMD ["doctor"]
