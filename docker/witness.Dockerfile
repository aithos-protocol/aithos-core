# aithos-witness — static musl binary in a FROM scratch image (piste P,
# lot A / P5; même doctrine que store-api.Dockerfile : tout vient de ce
# dépôt). Build from the repo root:
#   docker build -f docker/witness.Dockerfile -t aithos-witness:prod .
# Pushed to the provider ECR; the image carries NO secret — the signing
# key lives in KMS and never enters the process (annexe C.1); AWS access
# rides the Fargate task role.

# Base pinned for reproducibility — toolchain parity with the suite (1.96).
FROM rust:1.96-alpine AS build
# musl-dev for the static target; cmake/make/g++/perl for aws-lc-sys
# (the rustls provider inside the AWS SDKs builds C on musl);
# ca-certificates: the ONLY filesystem the scratch image needs beyond the
# binary — without root CAs the SDK cannot reach KMS/DynamoDB/S3 over TLS
# and the emitter cannot boot (fail-closed).
RUN apk add --no-cache musl-dev cmake make g++ perl ca-certificates
WORKDIR /src
COPY rust/ rust/
COPY vectors/ vectors/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml \
      -p aithos-provider --bin aithos-witness \
 && cp rust/target/release/aithos-witness /aithos-witness

FROM scratch
COPY --from=build /aithos-witness /aithos-witness
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# The witness observes and signs observations — never authority (§4).
# The image embarks the binary and the CA bundle, nothing else: no
# bootstrap, no key, no client material (doctrine, motif P7/P7b).
ENTRYPOINT ["/aithos-witness"]
