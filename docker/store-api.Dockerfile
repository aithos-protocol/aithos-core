# syntax=docker/dockerfile:1
# aithos-store-api — static musl binary in a FROM scratch image (piste P,
# lot P1; même doctrine que docker/Dockerfile : tout vient de ce dépôt).
# Build from the repo root:
#   docker build --build-context aithos-client=../aithos-client \
#     -f docker/store-api.Dockerfile -t aithos-store-api:prod .
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
COPY --from=aithos-client . /aithos-client/
RUN cargo build --release --locked --manifest-path rust/Cargo.toml \
      -p aithos-provider --bin aithos-store-api \
 && cp rust/target/release/aithos-store-api /aithos-store-api

FROM scratch
COPY --from=build /aithos-store-api /aithos-store-api
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# Décision ② du gate P7 (2026-07-20, bascule control-plane) : sous le
# backend dynamodb la task def ne porte plus AUCUN chemin bootstrap — la
# table control-plane est la SEULE source de tenants. Les deux bootstraps
# sans tenant du gate étape 6 (prod-none.json, prod-replay-<date>.json)
# SORTENT de l'image : elle n'embarque plus que le binaire et le CA
# bundle. Un retour vers une task def bootstrap (type :6) repasse par
# l'image du gate étape 6, épinglée par son digest en ECR
# (sha256:187cee4c…aeec3) — le tag :prod ne la porte plus.
ENTRYPOINT ["/aithos-store-api"]
