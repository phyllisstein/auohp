#!/usr/bin/env bash
#
# Download missing videos from the Vimeo folder one at a time and transcribe each
# before fetching the next. Safe to restart: a video counts as done once it lands
# in the S3 inbox, and a half-finished download or transcription is simply redone.
#
#   VIMEO_TOKEN=... scripts/vimeo-batch.sh
#
# Staging goes on local NVMe (fast, wiped on reboot); inputs, outputs, and the run
# log go on the S3 Files mount so they outlive the instance.

set -u

REPO="$(cd "$(dirname "$0")/.." && pwd)"
S3=${S3:-/mnt/s3/fs1}
STAGE=${STAGE:-/opt/dlami/nvme/vimeo-stage}
OUT=${OUT:-$S3/out/current}
RUN=${RUN:-$S3/runs/vimeo-$(date -u +%Y%m%d)}
BIN=${BIN:-$REPO/target/release/transcribe}
SUMMARY=$RUN/summary.tsv

export PATH=/usr/local/cuda/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:${LD_LIBRARY_PATH:-}
export INBOX=$S3/in

mkdir -p "$STAGE" "$RUN/logs" "$OUT"
cd "$REPO"

log() { printf '%s  %s\n' "$(date -u +%FT%TZ)" "$*" | tee -a "$RUN/batch.log" >&2; }

python3 scripts/vimeo-sync.py list > "$RUN/missing.tsv" || { log "listing failed"; exit 2; }
log "missing at start: $(wc -l < "$RUN/missing.tsv")"

while :; do
    t0=$(date +%s)
    f=$(python3 scripts/vimeo-sync.py next "$STAGE" 2>>"$RUN/batch.log") || { log "download failed; stopping"; exit 1; }
    [ -z "$f" ] && { log "nothing left to download"; break; }
    name=$(basename "$f"); stem=${name%.*}
    log "downloaded $name ($(( $(date +%s) - t0 ))s, $(du -h "$f" | cut -f1))"

    t1=$(date +%s)
    if "$BIN" --output "$OUT/$stem.json" "$f" 2> "$RUN/logs/$stem.log"; then
        status=ok
    else
        status="FAIL rc=$?"
        rm -f "$OUT/$stem.json"
    fi
    secs=$(( $(date +%s) - t1 ))
    printf '%s\t%s\t%ss\n' "$status" "$name" "$secs" >> "$SUMMARY"
    log "transcribed $name: $status (${secs}s)"

    # Without this guard a failed copy would re-download the same video forever.
    cp "$f" "$INBOX/$name.part" && mv "$INBOX/$name.part" "$INBOX/$name" && rm -f "$f" \
        || { log "couldn't move $name into $INBOX; stopping"; exit 1; }
done
