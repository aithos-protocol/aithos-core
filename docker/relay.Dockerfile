# syntax=docker/dockerfile:1
# aithos-relay — static musl binary in a FROM scratch image (piste P,
# lot P6 jalon M1). Build from the repo root:
#   docker build --build-context aithos-client=../aithos-client \
#     -f docker/relay.Dockerfile -t aithos-relay:prod .
# Pushed to the provider relay ECR by CI. NO secret: DynamoDB access rides
# the Fargate task role. The relay holds no client key.

FROM rust:1.96-alpine AS build
# musl-dev for the static target; cmake/make/g++/perl for aws-lc-sys
# (the rustls provider inside the DynamoDB SDK builds C on musl);
# ca-certificates: the scratch image needs the root CAs for the AWS TLS
# calls (DynamoDB) — without them every reservation refuses fail-closed.
RUN apk add --no-cache musl-dev cmake make g++ perl ca-certificates
WORKDIR /src
COPY rust/ rust/
COPY vectors/ vectors/
COPY --from=aithos-client . /aithos-client/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml \
      -p aithos-provider --bin aithos-relay \
 && cp rust/target/release/aithos-relay /aithos-relay

FROM scratch
COPY --from=build /aithos-relay /aithos-relay
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# Bascule P7b (2026-07-20, décision ② P7 appliquée au relay) : sous le
# backend dynamodb la task def ne porte plus AUCUN chemin bootstrap — la
# table control-plane est la SEULE source de mappings B.2. relay.json
# SORT de l'image (il reste au dépôt pour le mode memory dev/tests) :
# elle n'embarque plus que le binaire et le CA bundle. Un retour vers une
# task def bootstrap (type M2) repasse par l'image du gate M2, épinglée
# par son digest en ECR (sha256:d8f93851…58250) — le tag :prod ne la
# porte plus.
ENTRYPOINT ["/aithos-relay"]
