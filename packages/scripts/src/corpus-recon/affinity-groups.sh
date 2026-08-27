#!/usr/bin/env bash

set -euo pipefail

# Roster affinity groups and find the people who bridge them.
#
#   ./affinity-groups.sh [cache-dir]
#
# Affinity groups are the useful intermediate node in this collection. A person
# graph built from who-mentions-whom is saturated---see NOTES.md---but group
# membership is small and bounded, so `Person -> AffinityGroup -> Action` yields
# sparse paths that actually mean something.
#
# Caveat the output does not carry: this reports transcripts that *name* a
# group, which is not the same as membership. Mark Harrington naming Lavender
# Hill Mob does not put him in it. Separating the two wants the membership
# frames (`was in the X`, `part of the X`) rather than a bare mention.

CACHE="${1:-./cache}"
# NOT `GROUPS`. That name is a bash special variable holding the invoking
# user's group IDs, and bash silently refuses to let a script overwrite it---no
# error, no warning. The assignment appears to work, then every `< "$GROUPS"`
# reads from a file named `20` (gid `staff` on macOS) and the script dies
# pointing at a redirect line that is perfectly correct.
GROUP_LIST="$(dirname "$0")/affinity-groups.txt"

memberships="$(mktemp)"
trap 'rm -f "$memberships"' EXIT

# Read the group list into an array up front rather than looping over the file.
#
# The obvious shape---`while read -r group; do ... done < "$GROUP_LIST"`---breaks
# here, because the body pipes grep into another `while read`. Both loops then
# contend for the same input, and the whole construct falls apart in ways that
# report as a nonsense filename. Slurping first sidesteps the question: the loop
# below reads from memory, leaving stdin free for whatever the body wants.
groups=()
while IFS= read -r line; do
    [[ -n "$line" ]] && groups+=("$line")
done < "$GROUP_LIST"

for group in "${groups[@]}"; do
    # `IFS=` with no split characters is what protects the filenames here---
    # they all look like `NNN - Firstname Lastname.txt`, and an earlier pass
    # piped them through xargs and watched every name shatter on its spaces.
    #
    # Newline-delimited rather than NUL: BSD grep quietly ignores -Z when it is
    # combined with -l and emits newlines anyway, so asking for NULs buys
    # nothing on macOS. Safe here because these filenames contain spaces but
    # never newlines. If that ever stops being true, switch to `find -print0`.
    while IFS= read -r path; do
        printf '%s\t%s\n' "$(basename "$path" .txt)" "$group"
    done < <(grep -rlE "\b${group}\b" "$CACHE"/flat/)
done >> "$memberships"

echo "=== rosters ==="
for group in "${groups[@]}"; do
    printf '\n-- %s\n' "$group"
    awk -F'\t' -v g="$group" '$2 == g { print "     " $1 }' "$memberships"
done

echo
echo "=== articulation points: people spanning multiple groups ==="
echo "    (tag these first --- each one wires several components together)"
awk -F'\t' '
    { n[$1]++; groups[$1] = groups[$1] ", " $2 }
    END {
        for (person in n)
            if (n[person] > 1)
                printf "%d\t%s\t%s\n", n[person], person, substr(groups[person], 3)
    }
' "$memberships" \
    | sort -rn \
    | awk -F'\t' '{ printf "  %d groups  %-30s %s\n", $1, $2, $3 }'

echo
printf 'total memberships: %d across %d people\n' \
    "$(wc -l < "$memberships")" \
    "$(cut -f1 "$memberships" | sort -u | wc -l)"
