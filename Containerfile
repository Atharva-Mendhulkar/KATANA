# Containerfile - Reproducible test, build, and execution container for Katana (PRD §27, §31)
# Provides clean-room toolchain isolation for e2e validation across distributions.

FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH

# Install core build toolchain, clang with BPF target support, and libbpf headers
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

# Install minimal official Rust stable toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal

WORKDIR /build/katana
COPY . .

# 1. Enforce normative eBPF hook allow-list (PRD §4, §8.2)
RUN bash scripts/check-scope.sh

# 2. Run full 61-test verification matrix from clean checkout (PRD §31)
RUN cargo test --verbose

# 3. Compile optimized release artifact
RUN cargo build --release

# ------------------------------------------------------------------------------
# Minimal runtime deployment stage
# ------------------------------------------------------------------------------
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
