#!/usr/bin/env bash

set -euo pipefail

# Convert the ACT UP Oral History PDFs to a plain-text cache, once.
#
# Everything else in this directory reads that cache rather than the PDFs. The
# corpus is ~2.65M words---roughly 3.5M tokens, or two dozen context windows if
# you were foolish enough to read it. The entire point of these scripts is to
# answer questions about the collection by returning *counts* instead of prose,
# so the text never has to enter a conversation at all.
#
# `-layout` is not cosmetic. It preserves the original line structure, which is
# what makes line position a usable proxy for position-in-the-interview. Without
# it pdftotext reflows into one stream and line numbers stop meaning anything
# comparable across files.

PDF_DIR="${1:-$HOME/Documents/AUOHP/ACT UP Oral History PDFs}"
OUT_DIR="${2:-./cache}"

mkdir -p "$OUT_DIR/txt" "$OUT_DIR/flat"

count=0
for pdf in "$PDF_DIR"/*.pdf; do
    base="$(basename "$pdf" .pdf)"
    txt="$OUT_DIR/txt/$base.txt"

    # Skip work already done---this cache is meant to be reused across many
    # querying sessions, and re-running pdftotext 174 times is pure waste.
    [[ -f "$txt" ]] || pdftotext -layout "$pdf" "$txt"

    # A flattened, lowercased copy. Speech in these transcripts wraps across
    # line breaks constantly, so any grep wanting a context window wider than a
    # few words needs the newlines gone first. Learned the hard way: searching
    # for `.{70}affinity group.{50}` against the line-oriented copy returns
    # nothing at all, because the phrase almost never sits inside one line.
    [[ -f "$OUT_DIR/flat/$base.txt" ]] || \
        tr '\n' ' ' < "$txt" | tr -s ' ' | tr 'A-Z' 'a-z' > "$OUT_DIR/flat/$base.txt"

    count=$((count + 1))
done

echo "cached $count transcripts to $OUT_DIR"
echo "  txt/  --- line-oriented, original case; for line-position work"
echo "  flat/ --- one line per file, lowercased; for context-window greps"
