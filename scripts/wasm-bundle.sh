#!/usr/bin/env bash
# wasm-bundle.sh — build reproductible du bundle WASM de cérémonie (SPL-6).
#
# Le bundle servi par la gateway (`assets/ceremony/aithos_wasm.js` +
# `aithos_wasm_bg.wasm`) est un artefact BUILDÉ depuis `aithos-wasm`. Avant ce
# lot, il était commité à la main et jamais revérifié : une dérive entre le
# crate et l'artefact était indétectable. Ce script ferme le trou :
#
#   scripts/wasm-bundle.sh check   # (défaut) reconstruit depuis la source et
#                                  # échoue si le build OU les assets commités
#                                  # divergent du pin wasm-bundle-digest.json
#   scripts/wasm-bundle.sh regen   # reconstruit, remplace les assets commités
#                                  # et réécrit le pin (à committer ensemble)
#   scripts/wasm-bundle.sh print   # reconstruit et imprime les digests
#
# Recette reproductible : toolchain et wasm-bindgen-cli EXACTS du pin,
# `--locked`, chemins remappés (workspace, CARGO_HOME, HOME) pour que le même
# commit produise les mêmes octets ici et en CI. Fail-closed : une recette qui
# ne correspond pas au pin est une erreur, pas un avertissement.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODE="${1:-check}"
ASSETS="$ROOT/rust/crates/aithos-gateway/assets/ceremony"
PIN="$ASSETS/wasm-bundle-digest.json"
TARGET_DIR="$ROOT/rust/target/wasm-bundle"
OUT="$TARGET_DIR/bindgen"
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"

RUSTC_VERSION="$(rustc --version)"
WBG_VERSION="$(wasm-bindgen --version | awk '{print $2}')"

# --- 1. build + bindgen, chemins remappés --------------------------------
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="$TARGET_DIR"
export RUSTFLAGS="--remap-path-prefix=$ROOT=/aithos --remap-path-prefix=$CARGO_HOME_DIR=/cargo --remap-path-prefix=$HOME=/home"
echo "[wasm-bundle] rustc: $RUSTC_VERSION | wasm-bindgen-cli: $WBG_VERSION"
echo "[wasm-bundle] cargo build -p aithos-wasm --release --target wasm32-unknown-unknown …"
cargo build --quiet --locked -p aithos-wasm --release \
  --target wasm32-unknown-unknown --manifest-path "$ROOT/rust/Cargo.toml"
rm -rf "$OUT"; mkdir -p "$OUT"
wasm-bindgen --target web --out-dir "$OUT" --out-name aithos_wasm \
  "$TARGET_DIR/wasm32-unknown-unknown/release/aithos_wasm.wasm"

sha() { sha256sum "$1" | awk '{print "sha256:" $1}'; }
BUILT_JS="$(sha "$OUT/aithos_wasm.js")"
BUILT_WASM="$(sha "$OUT/aithos_wasm_bg.wasm")"
echo "[wasm-bundle] build   aithos_wasm.js      $BUILT_JS"
echo "[wasm-bundle] build   aithos_wasm_bg.wasm $BUILT_WASM"

if [ "$MODE" = "print" ]; then exit 0; fi

# --- 2. regen : remplacer assets + pin -----------------------------------
if [ "$MODE" = "regen" ]; then
  cp "$OUT/aithos_wasm.js" "$OUT/aithos_wasm_bg.wasm" "$ASSETS/"
  cat >"$PIN" <<EOF
{
  "recipe": {
    "rustc": "$RUSTC_VERSION",
    "wasm_bindgen_cli": "$WBG_VERSION",
    "target": "wasm32-unknown-unknown",
    "profile": "release",
    "build": "scripts/wasm-bundle.sh regen"
  },
  "artifacts": {
    "aithos_wasm.js": "$BUILT_JS",
    "aithos_wasm_bg.wasm": "$BUILT_WASM"
  }
}
EOF
  echo "[wasm-bundle] REGEN : assets + pin réécrits ($PIN) — à committer ensemble."
  exit 0
fi

# --- 3. check : recette, build et assets contre le pin -------------------
[ -f "$PIN" ] || { echo "[wasm-bundle] ÉCHEC : pin absent ($PIN) — lancer regen."; exit 1; }
pin() { python3 -c "import json,sys; print(json.load(open('$PIN'))$1)"; }
PIN_RUSTC="$(pin "['recipe']['rustc']")"
PIN_WBG="$(pin "['recipe']['wasm_bindgen_cli']")"
PIN_JS="$(pin "['artifacts']['aithos_wasm.js']")"
PIN_WASM="$(pin "['artifacts']['aithos_wasm_bg.wasm']")"

FAIL=0
if [ "$PIN_RUSTC" != "$RUSTC_VERSION" ] || [ "$PIN_WBG" != "$WBG_VERSION" ]; then
  echo "[wasm-bundle] ÉCHEC : recette ≠ pin (pin : $PIN_RUSTC + wbg $PIN_WBG ;"
  echo "               ici : $RUSTC_VERSION + wbg $WBG_VERSION)."
  echo "               Aligner la toolchain, ou regen + committer si le bump est voulu."
  exit 1
fi
if [ "$BUILT_JS" != "$PIN_JS" ] || [ "$BUILT_WASM" != "$PIN_WASM" ]; then
  echo "[wasm-bundle] ÉCHEC : la source aithos-wasm a dérivé du pin —"
  echo "               regen + committer assets et pin avec le changement de source."
  FAIL=1
fi
COMMITTED_JS="$(sha "$ASSETS/aithos_wasm.js")"
COMMITTED_WASM="$(sha "$ASSETS/aithos_wasm_bg.wasm")"
if [ "$COMMITTED_JS" != "$PIN_JS" ] || [ "$COMMITTED_WASM" != "$PIN_WASM" ]; then
  echo "[wasm-bundle] ÉCHEC : les assets commités divergent du pin."
  FAIL=1
fi
if [ "$FAIL" -ne 0 ]; then exit 1; fi
echo "[wasm-bundle] VERT — source, assets commités et pin alignés."
