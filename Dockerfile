# syntax=docker/dockerfile:1
FROM debian:bookworm-slim AS resources
ARG TARGETARCH
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl jq openssl && rm -rf /var/lib/apt/lists/*
COPY assets/manifest.json /manifest.json
COPY scripts/fetch-container-assets.sh /fetch-assets.sh
RUN sh /fetch-assets.sh /manifest.json /bundle/assets "$TARGETARCH"

FROM rust:1.96.0-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang libclang-dev && rm -rf /var/lib/apt/lists/*
ENV CARGO_BUILD_JOBS=2
RUN cargo install cargo-chef --version 0.1.78 --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY --from=resources /bundle/assets /opt/pdf2zh/assets
ENV PDF2ZH_ASSETS=/opt/pdf2zh/assets \
    ORT_DYLIB_PATH=/opt/pdf2zh/assets/onnxruntime/lib/libonnxruntime.so
COPY . .
RUN cargo build --release --locked && cargo test --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libstdc++6 libgomp1 && rm -rf /var/lib/apt/lists/* \
    && mkdir /data && chown 10001:10001 /data
COPY --from=builder /build/target/release/pdf2zh-rust /usr/local/bin/pdf2zh-rust
COPY --from=resources /bundle/assets /opt/pdf2zh/assets
COPY LICENSE NOTICE /opt/pdf2zh/
ARG VCS_REF=unknown
LABEL org.opencontainers.image.title="pdf2zh-rust development preview" \
    org.opencontainers.image.source="https://github.com/jmjFortune/pdf2zh-rust" \
    org.opencontainers.image.licenses="AGPL-3.0-only" \
    org.opencontainers.image.revision="$VCS_REF"
ENV HOME=/data PDF2ZH_ASSETS=/opt/pdf2zh/assets \
    ORT_DYLIB_PATH=/opt/pdf2zh/assets/onnxruntime/lib/libonnxruntime.so
WORKDIR /data
USER 10001:10001
VOLUME ["/data"]
EXPOSE 7861
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:7861/api/health || exit 1
ENTRYPOINT ["/usr/local/bin/pdf2zh-rust"]
CMD ["serve", "--listen", "0.0.0.0:7861", "--data-dir", "/data"]
