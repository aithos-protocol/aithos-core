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

    # Line 1 is the feature-level tag line: a space-separated list of tags.
    # Every token on it must be a tag, and the canonical tag @<stem> must be
    # one of them, as a whole token; other tags (@wip, plan or surface
    # markers) may sit alongside it, in any order.
    found=0
    tag_line=1
    set -f
    for tag in $first_line; do
        case "$tag" in
            @?*) ;;
            *) tag_line=0 ;;
        esac
        if [ "$tag" = "$expected" ]; then
            found=1
        fi
    done
    set +f

    if [ "$tag_line" -ne 1 ]; then
        printf '%s: line 1 must be a tag line (every token starting with @), got %s\n' "$feature" "$first_line" >&2
        failed=1
    elif [ "$found" -ne 1 ]; then
        printf '%s: expected tag %s on the first line, got %s\n' "$feature" "$expected" "$first_line" >&2
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
