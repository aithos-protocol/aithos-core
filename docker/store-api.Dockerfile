# aithos-store-api — static musl binary in a FROM scratch image (piste P,
# lot P1; même doctrine que docker/Dockerfile : tout vient de ce dépôt).
# Build from the repo root:
#   docker build -f docker/store-api.Dockerfile -t aithos-store-api:prod .
# Pushed to the provider ECR by .github/workflows/provider-image.yml; the
# image carries NO secret — the bootstrap embarks public did.json material
# only, DynamoDB access rides the Fargate task role.

# Base pinned for reproducibility — toolchain parity with the suite (1.96).
FROM rust:1.96-alpine AS build
# musl-dev for the static target; cmake/make/g++/perl for aws-lc-sys
# (the rustls provider inside the DynamoDB SDK builds C on musl);
# ca-certificates: the ONLY filesystem the scratch image needs beyond the
# binary — without root CAs the SDK cannot reach DynamoDB over TLS and
# every enveloped request refuses 503 (fail-closed, caught at the P1 gate).
RUN apk add --no-cache musl-dev cmake make g++ perl ca-certificates
WORKDIR /src
COPY rust/ rust/
COPY vectors/ vectors/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml \
      -p aithos-provider --bin aithos-store-api \
 && cp rust/target/release/aithos-store-api /aithos-store-api

FROM scratch
COPY --from=build /aithos-store-api /aithos-store-api
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# The replay bootstrap: the committed p1 vector's tenant/DID/did.json —
# public material, drift-guarded by a unit test against vectors/.
COPY rust/crates/aithos-provider/bootstrap/replay.json /bootstrap/replay.json
ENTRYPOINT ["/aithos-store-api"]
