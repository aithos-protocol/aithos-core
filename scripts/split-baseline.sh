#!/usr/bin/env bash
# split-baseline.sh — harnais anti-régression du chantier split repo (SPL-0).
#
# Rejoue `cargo test --workspace` et compare les compteurs de tests, par crate
# et par harnais (y compris les [Summary] cucumber), contre la baseline figée
# dans docs/audits/split/baseline-counts-2026-07-30.tsv. Échoue sur :
#   - tout test ou scénario en échec ;
#   - tout compteur `passed` inférieur à la baseline ;
#   - tout harnais de la baseline absent du run ;
#   - toute variation du nombre de tags @wip dans les fichiers .feature
#     (un @wip activé ou supprimé est un blocage, pas un détail) ;
#   - un code de sortie cargo non nul.
#
# BDER-011 (connu, ouvert) : le harnais cucumber d'aithos-bundle appelle
# `filter_run` sous `harness = false` et sort 0 même quand des scénarios
# échouent. Ce script ne fait donc JAMAIS confiance au seul code de sortie :
# il parse les blocs [Summary] et les lignes `test result:`.
#
# Usage :
#   scripts/split-baseline.sh                 # rejoue et compare
#   SPLIT_BASELINE_LOG=/tmp/x.log …           # fixe l'emplacement du log
#   SPLIT_BASELINE_REUSE_LOG=/path/to/run.log # compare un log existant (pas de run)
#   SPLIT_BASELINE_PRINT=1                    # imprime le TSV courant et sort
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE_TSV="${SPLIT_BASELINE_FILE:-$ROOT/docs/audits/split/baseline-counts-2026-07-30.tsv}"
LOG="${SPLIT_BASELINE_LOG:-$(mktemp /tmp/split-baseline.XXXXXX.log)}"

# --- 1. run (ou réutilisation d'un log existant) ---------------------------
if [ -n "${SPLIT_BASELINE_REUSE_LOG:-}" ]; then
  LOG="$SPLIT_BASELINE_REUSE_LOG"
  CARGO_EXIT=0
else
  echo "[split-baseline] cargo test --workspace … (log: $LOG)"
  cargo test --workspace --manifest-path "$ROOT/rust/Cargo.toml" >"$LOG" 2>&1
  CARGO_EXIT=$?
fi

# --- 2. parse : un TSV `harnais<TAB>métrique<TAB>valeur` --------------------
parse_counts() {
  awk '
    function grab(s, re,   v) { # première capture numérique de re dans s
      if (match(s, re)) { v = substr(s, RSTART, RLENGTH); gsub(/[^0-9]/, "", v); return v }
      return ""
    }
    /^[ \t]+Running unittests / {
      if (match($0, /deps\/[A-Za-z0-9_]+-/)) {
        target = substr($0, RSTART + 5, RLENGTH - 6)
      }
      # src/lib.rs -> <crate>::unittests_lib ; src/main.rs -> <crate>::unittests_main ;
      # src/bin/x.rs -> <cible>::unittests_bin — sans capturer les tests
      # d intégration suivants, qui appartiennent au crate porteur (lib/main).
      if ($0 ~ /src\/bin\//) {
        h = target "::unittests_bin"
      } else {
        crate = target
        kind = ($0 ~ /src\/main\.rs/) ? "unittests_main" : "unittests_lib"
        h = crate "::" kind
      }
      next
    }
    /^[ \t]+Running tests\/[A-Za-z0-9_.]+\.rs \(/ {
      if (match($0, /tests\/[A-Za-z0-9_.]+\.rs/)) {
        t = substr($0, RSTART + 6, RLENGTH - 9)
      }
      h = crate "::" t; next
    }
    /^[ \t]+Doc-tests / { h = $2 "::doctests"; next }
    /^test result:/ {
      p = grab($0, "[0-9]+ passed"); f = grab($0, "[0-9]+ failed")
      print h "\tpassed\t" p
      if (f != "" && f != "0") print h "\tHARD_FAILED\t" f
      next
    }
    /^\[Summary\]/ { insum = 1; next }
    insum && /scenarios? \(/ {
      tot = grab($0, "^[0-9]+"); p = grab($0, "\\([0-9]+ passed")
      print h "\tscenarios_passed\t" p
      if (tot != p) print h "\tSCENARIOS_NOT_ALL_PASSED\t" tot "-" p
      next
    }
    insum && /steps? \(/ {
      tot = grab($0, "^[0-9]+"); p = grab($0, "\\([0-9]+ passed")
      print h "\tsteps_passed\t" p
      if (tot != p) print h "\tSTEPS_NOT_ALL_PASSED\t" tot "-" p
      insum = 0; next
    }
  ' "$1" | sort
}

CUR_TSV="$(mktemp /tmp/split-current.XXXXXX.tsv)"
parse_counts "$LOG" >"$CUR_TSV"

# @wip : compte des tags dans les .feature, jamais dans les .rs ni les docs.
WIP_CUR=$(grep -r "@wip" "$ROOT/features" "$ROOT/rust/crates" --include="*.feature" -h 2>/dev/null | wc -l | tr -d ' ')
printf '@wip\ttags\t%s\n' "$WIP_CUR" >>"$CUR_TSV"
sort -o "$CUR_TSV" "$CUR_TSV"

if [ -n "${SPLIT_BASELINE_PRINT:-}" ]; then cat "$CUR_TSV"; exit 0; fi

# --- 3. compare -------------------------------------------------------------
FAIL=0

if [ "$CARGO_EXIT" -ne 0 ]; then
  echo "[split-baseline] ÉCHEC : cargo test exit=$CARGO_EXIT (log: $LOG)"; FAIL=1
fi

if grep -q "HARD_FAILED\|NOT_ALL_PASSED" "$CUR_TSV"; then
  echo "[split-baseline] ÉCHEC : tests ou scénarios en échec —"
  grep "HARD_FAILED\|NOT_ALL_PASSED" "$CUR_TSV"
  FAIL=1
fi

if [ ! -f "$BASE_TSV" ]; then
  echo "[split-baseline] ÉCHEC : baseline absente ($BASE_TSV)"; exit 1
fi

# Chaque ligne de la baseline doit exister dans le run avec valeur >= baseline.
while IFS="$(printf '\t')" read -r harness metric value; do
  case "$harness" in ''|'#'*) continue;; esac
  cur=$(awk -F'\t' -v h="$harness" -v m="$metric" '$1==h && $2==m {print $3; exit}' "$CUR_TSV")
  if [ -z "$cur" ]; then
    echo "[split-baseline] ÉCHEC : harnais/métrique manquant : $harness $metric (baseline=$value)"
    FAIL=1
  elif [ "$metric" = "tags" ]; then
    if [ "$cur" -ne "$value" ]; then
      echo "[split-baseline] ÉCHEC : @wip a bougé : $cur ≠ baseline $value"
      FAIL=1
    fi
  elif [ "$cur" -lt "$value" ]; then
    echo "[split-baseline] ÉCHEC : baisse : $harness $metric $cur < baseline $value"
    FAIL=1
  elif [ "$cur" -gt "$value" ]; then
    echo "[split-baseline] note : hausse : $harness $metric $cur > baseline $value"
  fi
done <"$BASE_TSV"

if [ "$FAIL" -ne 0 ]; then
  echo "[split-baseline] ROUGE (log: $LOG)"
  exit 1
fi
echo "[split-baseline] VERT — aucun compteur en baisse (log: $LOG)"
