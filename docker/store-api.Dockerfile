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
# Décision ② du gate P2/étape 6 (2026-07-20, gravée INFRA-PROVIDER §8) :
# le bootstrap de rejeu `acme` (replay.json, clés des vecteurs committés —
# publiques) DISPARAÎT de l'image prod dès que les backends deviennent
# durables. L'image embarque deux bootstraps SANS preloads ni seeds (le
# binaire refuse de booter autrement, garde fail-closed) :
#   - prod-replay-<date>.json : le tenant de rejeu jetable du gate déployé
#     (bindings pré-genèse seuls — la genèse arrive par le wire) ;
#   - prod-none.json : zéro tenant — l'état de repos post-purge, jusqu'à la
#     bascule P7 (table control-plane + bin admin).
# La task def choisit via AITHOS_STORE_BOOTSTRAP (var bootstrap_path).
COPY rust/crates/aithos-provider/bootstrap/prod-replay-20260720.json /bootstrap/prod-replay-20260720.json
COPY rust/crates/aithos-provider/bootstrap/prod-none.json /bootstrap/prod-none.json
ENTRYPOINT ["/aithos-store-api"]
