# aithos-relay — static musl binary in a FROM scratch image (piste P,
# lot P6 jalon M1). Build from the repo root:
#   docker build -f docker/relay.Dockerfile -t aithos-relay:prod .
# Pushed to the provider relay ECR by CI. NO secret: the bootstrap carries
# PUBLIC tunnel mappings (gateway_pub ↔ tenant ↔ hostname) only; DynamoDB
# access rides the Fargate task role. The relay holds no client key.

FROM rust:1.96-alpine AS build
# musl-dev for the static target; cmake/make/g++/perl for aws-lc-sys
# (the rustls provider inside the DynamoDB SDK builds C on musl);
# ca-certificates: the scratch image needs the root CAs for the AWS TLS
# calls (DynamoDB) — without them every reservation refuses fail-closed.
RUN apk add --no-cache musl-dev cmake make g++ perl ca-certificates
WORKDIR /src
COPY rust/ rust/
COPY vectors/ vectors/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml \
      -p aithos-provider --bin aithos-relay \
 && cp rust/target/release/aithos-relay /aithos-relay

FROM scratch
COPY --from=build /aithos-relay /aithos-relay
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# The relay control-plane bootstrap: the committed p3 gateway mapping —
# public material (the p3 gateway key is a committed test key).
COPY rust/crates/aithos-provider/bootstrap/relay.json /bootstrap/relay.json
ENTRYPOINT ["/aithos-relay"]
