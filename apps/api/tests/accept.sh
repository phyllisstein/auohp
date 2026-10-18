#!/usr/bin/env bash
# Acceptance test: run the transcription pipeline on each clip in tests/clips/
# and save JSON output + logs to tests/output/<stem>/{output.json,trace.log}.
#
# Usage:
#   ./tests/accept.sh                   # run all clips
#   ./tests/accept.sh tests/clips/025_lei_chou.mp4  # run one clip
#
# Clips were cut at these random offsets from the source videos:
#   025_lei_chou.mp4       offset=181s
#   082_david_robinson.mp4 offset=1073s
#   the_ashes_action.mp4   offset=1260s

set -euo pipefail

CLIPS_DIR="tests/clips"
OUTPUT_DIR="tests/output"
MODELS_DIR="models"
BINARY="cargo run --features metal --bin transcribe --"

clips=("$@")
if [ ${#clips[@]} -eq 0 ]; then
    clips=("$CLIPS_DIR"/*.mp4)
fi

cargo metal --bin transcribe 2>&1

echo "Cleaning previous outputs..."
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

for clip in "${clips[@]}"; do
    stem=$(basename "$clip" .mp4)
    out_dir="$OUTPUT_DIR/$stem"
    mkdir -p "$out_dir"

    echo "==> $stem"
    $BINARY "$clip" "$MODELS_DIR" \
        > "$out_dir/output.json" \
        2> "$out_dir/trace.log" \
        && echo "    ok  --> $out_dir/output.json" \
        || echo "    FAILED---see $out_dir/trace.log"
done

echo
echo "Done. Results in $OUTPUT_DIR/"
