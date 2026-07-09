# aithos-core — static CLI in a FROM scratch image (~8 MB).
# Build from the repo root:  docker build -f docker/Dockerfile -t aithos-core .

FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY rust/ rust/
COPY vectors/ vectors/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml -p aithos-cli \
 && cp rust/target/release/aithos-core /aithos-core

FROM scratch
COPY --from=build /aithos-core /aithos-core
ENTRYPOINT ["/aithos-core"]
