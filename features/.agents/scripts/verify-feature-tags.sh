#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
feature_dir="$repo_root/features"
checked=0
failed=0

for feature in "$feature_dir"/*.feature; do
    [ -e "$feature" ] || continue
    name="${feature##*/}"
    stem="${name%.feature}"
    expected="@$stem"
    first_line="$(sed -n '1p' "$feature")"
    second_line="$(sed -n '2p' "$feature")"
    checked=$((checked + 1))

    if [ "$first_line" != "$expected" ]; then
        printf '%s: expected first line %s, got %s\n' "$feature" "$expected" "$first_line" >&2
        failed=1
    fi
    if [[ "$second_line" != "Feature:"* ]]; then
        printf '%s: expected Feature declaration on line 2\n' "$feature" >&2
        failed=1
    fi
done

if [ "$checked" -eq 0 ]; then
    printf 'no feature files found under %s\n' "$feature_dir" >&2
    exit 1
fi
if [ "$failed" -ne 0 ]; then
    exit 1
fi

printf 'feature tags ok (%s files)\n' "$checked"
