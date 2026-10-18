#!/usr/bin/env bash

set -euo pipefail

# Rank transcripts by how much they discuss a topic.
#
#   ./rank-topic.sh terms-stop-the-church.txt [cache-dir] [top-n]
#
# Term files are `tier<TAB>regex` per line. Tier 1 terms are near-certain
# evidence on their own; tier 2 corroborate but are too common to trust alone
# (`communion`, `coffin`, `procession`). This script counts tier-1 only---the
# tier-2 column exists for the clustering work described in NOTES.md, which
# uses them to extend a span but never to anchor one.
#
# The metric here is raw hit count, deliberately. It is *not* length-normalized
# and it is *not* a measure of time spent. See NOTES.md for why the elaborate
# version was built and then set aside.

TERMS="${1:?usage: rank-topic.sh TERMS_FILE [CACHE_DIR] [TOP_N]}"
CACHE="${2:-./cache}"
TOP="${3:-10}"

# Assemble one alternation from the tier-1 patterns. A single grep with a
# combined pattern beats N greps per file: 174 files x 6 terms is 1044 process
# spawns, versus 174. On a corpus this size that is the difference between
# "instant" and "go get coffee".
pattern="$(awk -F'\t' '$1 == 1 { printf "%s%s", sep, $2; sep = "|" }' "$TERMS")"

if [[ -z "$pattern" ]]; then
    echo "no tier-1 terms found in $TERMS" >&2
    exit 1
fi

for f in "$CACHE"/txt/*.txt; do
    printf '%d\t%s\n' \
        "$(grep -ciE "$pattern" "$f" || true)" \
        "$(basename "$f" .txt)"
done \
    | sort -rn \
    | head -n "$TOP" \
    | awk -F'\t' '{ printf "%2d. %-42s %3d hits\n", NR, $2, $1 }'
