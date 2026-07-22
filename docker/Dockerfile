# aithos-core — static CLI in a FROM scratch image (< 15 MB, plan §K).
# Build from the repo root:  docker build -f docker/Dockerfile -t aithos-core:0.1.0 .
# Everything in the image comes from this repository; the image is pushed
# nowhere (bounded packaging, decided 2026-07-11).

# Base pinned for reproducibility — toolchain parity with the suite (1.96).
FROM rust:1.96-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY rust/ rust/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml -p aithos-cli \
 && cp rust/target/release/aithos /aithos

FROM scratch
COPY --from=build /aithos /aithos
ENTRYPOINT ["/aithos"]
