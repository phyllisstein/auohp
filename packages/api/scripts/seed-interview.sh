#!/usr/bin/env bash
#
# seed-interview.sh --- POST a seedInterview mutation to the auohp-api
# GraphQL endpoint, with the transcribe(1) JSON output embedded as the
# segments_json variable.
#
# Usage:
#   seed-interview.sh <transcribe.json> \
#       --number 26 \
#       --date 2003-05-16 \
#       --interviewee "Iris Long" \
#       --speakers ./meta/iris-long-speakers.json
#
# Flags:
#   --number N             Interview number (integer).
#   --date YYYY-MM-DD      ISO 8601 date.
#   --interviewee NAME     Display name of the interviewee.
#   --speakers PATH|JSON   Speakers list. Either a path to a JSON file
#                          containing an array, or an inline JSON literal.
#                          Each element: { name, label, role }
#                          where role is INTERVIEWER or INTERVIEWEE.
#   --endpoint URL         GraphQL endpoint (default: $SEED_ENDPOINT or
#                          http://localhost:6060/graphql).
#
# Requires: bash, curl, jq.

set -euo pipefail

ENDPOINT="${SEED_ENDPOINT:-http://localhost:6060/graphql}"
JSON_FILE=""
NUMBER=""
DATE=""
INTERVIEWEE=""
SPEAKERS_ARG=""

usage() {
    sed -n '/^# seed-interview/,/^$/{ s/^# \{0,1\}//; p; }' "$0"
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --number) NUMBER="$2"; shift 2;;
        --date) DATE="$2"; shift 2;;
        --interviewee) INTERVIEWEE="$2"; shift 2;;
        --speakers) SPEAKERS_ARG="$2"; shift 2;;
        --endpoint) ENDPOINT="$2"; shift 2;;
        -h|--help) usage 0;;
        --) shift; break;;
        -*)
            echo "unknown flag: $1" >&2
            usage 1
            ;;
        *)
            if [[ -z "$JSON_FILE" ]]; then
                JSON_FILE="$1"
            else
                echo "unexpected positional arg: $1" >&2
                usage 1
            fi
            shift
            ;;
    esac
done

for var in JSON_FILE NUMBER DATE INTERVIEWEE SPEAKERS_ARG; do
    if [[ -z "${!var}" ]]; then
        echo "missing required argument: $var" >&2
        usage 1
    fi
done

if [[ ! -f "$JSON_FILE" ]]; then
    echo "transcribe JSON not found: $JSON_FILE" >&2
    exit 1
fi

# Resolve speakers: file path if it exists, otherwise treat as inline JSON.
if [[ -f "$SPEAKERS_ARG" ]]; then
    SPEAKERS_JSON=$(cat "$SPEAKERS_ARG")
else
    SPEAKERS_JSON="$SPEAKERS_ARG"
fi

# Validate the speakers JSON parses as an array.
if ! echo "$SPEAKERS_JSON" | jq -e 'type == "array"' >/dev/null 2>&1; then
    echo "--speakers must be a JSON array (or a path to a file containing one)" >&2
    exit 1
fi

read -r -d '' QUERY <<'GRAPHQL' || true
mutation SeedInterview($input: SeedInterviewInput!) {
  seedInterview(input: $input) {
    statementCount
    speakerCount
    transcriptUid
    embeddingsQueued
    interview {
      date
      interviewee
      number
      uid
    }
  }
}
GRAPHQL

# Compose the request body. --slurpfile parses the transcribe JSON so we
# can extract .segments (the pipeline wraps the array in an object);
# tojson re-encodes just the array as a string for the segmentsJson field.
PAYLOAD=$(jq -n \
    --arg query "$QUERY" \
    --slurpfile segmentsFile "$JSON_FILE" \
    --argjson number "$NUMBER" \
    --arg date "$DATE" \
    --arg interviewee "$INTERVIEWEE" \
    --argjson speakers "$SPEAKERS_JSON" \
    '{
        query: $query,
        variables: {
            input: {
                number: $number,
                date: $date,
                interviewee: $interviewee,
                speakers: $speakers,
                segmentsJson: ($segmentsFile[0].segments | tojson)
            }
        }
    }')

echo "POST $ENDPOINT (interview #$NUMBER, $(wc -c < "$JSON_FILE") bytes of segments)" >&2

echo "$PAYLOAD" \
    | curl --fail-with-body -sS -X POST "$ENDPOINT" \
        -H "Content-Type: application/json" \
        --data-binary @- \
    | jq .
