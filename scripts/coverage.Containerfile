FROM docker.io/library/rust:1.100.0-slim-trixie@sha256:4b68057dc8c103af4fa66b5cd29f21b18afec79fadfcd8fb91e295e939f0690c

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates curl g++ git libegl1 libgl1-mesa-dri libudev-dev \
       libxkbcommon-x11-0 pkg-config util-linux xauth xvfb \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt llvm-tools-preview

RUN case "$(uname -m)" in \
      x86_64) \
        target=x86_64-unknown-linux-gnu; \
        cov_sha=b3f68e625481fed9b16444174f3fa5ebcdbde4a1878803a35eabe2dcefcdc41a; \
        nextest_sha=1d4ce302b820c41f4b89c5fb9bce45f0f0c94882c1cf08fc360caf08be6e7df4 ;; \
      aarch64) \
        target=aarch64-unknown-linux-gnu; \
        cov_sha=abf5f13c1520f8756d2192bfaaeadb0208f5b7eaa8cadc15bf847befa42b7360; \
        nextest_sha=3e35e8f6913af92558ba09d7155f74e35dd1f0024ef3cd891637433ae4eacbdd ;; \
      *) echo 'Unsupported container architecture' >&2; exit 1 ;; \
    esac \
    && curl --fail --location --silent --show-error \
       "https://github.com/taiki-e/cargo-llvm-cov/releases/download/v0.9.1/cargo-llvm-cov-${target}.tar.gz" \
       --output /tmp/cov.tar.gz \
    && echo "$cov_sha  /tmp/cov.tar.gz" | sha256sum --check - \
    && tar -xzf /tmp/cov.tar.gz -C /usr/local/cargo/bin cargo-llvm-cov \
    && curl --fail --location --silent --show-error \
       "https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.151/cargo-nextest-0.9.151-${target}.tar.gz" \
       --output /tmp/nextest.tar.gz \
    && echo "$nextest_sha  /tmp/nextest.tar.gz" | sha256sum --check - \
    && tar -xzf /tmp/nextest.tar.gz -C /usr/local/cargo/bin cargo-nextest \
    && rm /tmp/cov.tar.gz /tmp/nextest.tar.gz

WORKDIR /workspace
