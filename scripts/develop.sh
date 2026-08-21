#!/usr/bin/env bash

set -euo pipefail

args="$*"

# Where we record each service's process-group id. Deliberately outside /app so
# these never ride the bind mount back onto the host tree.
RUN_DIR=/tmp/auohp-run
mkdir -p "$RUN_DIR"

# How long to wait for a process group to exit after SIGTERM before escalating.
STOP_GRACE_SECONDS=10

# Launch a command as a new session leader. `setsid` makes the child's pid
# double as the process-group id of everything it goes on to spawn---so one
# signal reaches the whole tree.
#
# This is the crux of the old bug: `cargo` forks `rustc --crate-name serde_json`
# and `build-script-build`, whose command lines share no substring with
# "auohp-api". `pkill -f` could never match them, so it decapitated cargo and
# left its children compiling into the target directory.
start_service() {
    local name="$1"
    shift

    setsid "$@" &
    local pgid=$!
    echo "$pgid" >"$RUN_DIR/$name.pgid"
    disown
}

# Same session-leader trick, but blocks until the command finishes. For work a
# caller depends on having completed rather than a service it wants supervised.
#
# Note the deliberate absence of `disown`: keeping this in the job table is what
# makes `wait` possible at all.
run_service() {
    local name="$1"
    shift

    setsid "$@" &
    local pgid=$!
    echo "$pgid" >"$RUN_DIR/$name.pgid"

    # `|| status=$?` keeps `set -e` from aborting here, so the pidfile below is
    # always cleaned up. A non-zero status usually means a newer invocation
    # stopped this group out from under us---which is the intended outcome.
    local status=0
    wait "$pgid" || status=$?

    rm -f "$RUN_DIR/$name.pgid"
    return "$status"
}

# Terminate every process in the group recorded for `name`, returning only once
# they are actually gone. Blocking matters: a fresh `cargo` must not begin while
# a stale `rustc` still holds the target directory, or the two race and cargo
# dies with "failed to create directory ... File exists (os error 17)".
#
# Signal a whole group by negating the pgid: `kill -TERM -"$pgid"`.
stop_service() {
    local name="$1"
    local pidfile="$RUN_DIR/$name.pgid"

    # Nothing recorded means nothing to stop---a first boot, not an error.
    [[ -f "$pidfile" ]] || return 0

    local pgid
    pgid=$(<"$pidfile")

    # Drop the record before signalling anything. Pids recycle, so a pidfile we
    # failed to act on is far safer than one we might later replay against an
    # unrelated group that happens to have inherited the number.
    rm -f "$pidfile"

    # `kill -0` probes for a group's existence without signalling it. A stale
    # pidfile left by a previous container is the ordinary case, not an error.
    kill -0 -"$pgid" 2>/dev/null || return 0

    echo "Stopping process group $pgid ($name)..."
    kill -TERM -"$pgid" 2>/dev/null || true

    # Poll rather than `wait`: these were disowned, and after a container
    # restart they were never our children to begin with.
    local waited=0
    while kill -0 -"$pgid" 2>/dev/null && ((waited < STOP_GRACE_SECONDS)); do
        sleep 1
        waited=$((waited + 1))
    done

    if kill -0 -"$pgid" 2>/dev/null; then
        echo "Group $pgid ignored SIGTERM after ${STOP_GRACE_SECONDS}s; sending SIGKILL."
        kill -KILL -"$pgid" 2>/dev/null || true
        # Bounded, deliberately. Bash-as-PID-1 does not reap disowned orphans,
        # so a dead-but-unreaped process still answers `kill -0` forever---and
        # looping on that would hang the container. A zombie holds no file
        # handles and cannot write to the target directory, which is the only
        # property we actually need before starting the next cargo.
        sleep 1
    fi
}

restart_editor() {
    pushd packages/editor >/dev/null

    echo "Terminate existing editor..."
    stop_service editor

    echo "Starting editor development server..."
    start_service editor yarn editor:dev

    popd >/dev/null
}

restart_search() {
    pushd packages/search >/dev/null

    echo "Terminate existing search component server..."
    stop_service search

    echo "Starting search component development server..."
    start_service search yarn search:dev

    popd >/dev/null
}

restart_api() {
    echo "Terminate existing API..."
    stop_service api

    echo "Starting API development server..."
    ./packages/core/scripts/download-models.sh
    start_service api cargo run --package auohp-api --bin auohp-api
}

configure_watches() {
    echo "Configuring watches..."

    watchman watch-project /app
    for j in scripts/watchman/*.json; do
        echo "Setting watch $j"
        watchman -j <"$j"
    done
}

watch_watchman() {
    echo "Logging watchman..."
    configure_watches
    tail -f /usr/local/var/run/watchman/root-state/log
}

yarn_install() {
    # `/usr/bin/yarn` is a /bin/sh wrapper, so the old `pkill -f "yarn install"`
    # matched the wrapper's command line and nothing else---the real installer
    # underneath it, `node .yarn/releases/yarn-4.18.0.cjs install`, shares no
    # "yarn install" substring and survived every restart to race the next one.
    echo "Terminate existing install..."
    stop_service yarn

    echo "Running yarn install..."
    run_service yarn yarn install
}


[[ -e "/run/secrets/environment" ]] || { echo "Missing secret environment file." && exit 0; }
source /run/secrets/environment && export NEO4J_PASSWORD NEO4J_USERNAME NEO4J_URI NEO4J_DATABASE HF_TOKEN

# Tracing is opt-in, and deliberately switched on only *after* the secret is
# sourced. `set -x` traces the assignments inside a sourced file, so enabling it
# any earlier prints NEO4J_PASSWORD and HF_TOKEN in plaintext to the container
# logs---where the json-file driver then persists them to disk. (The `export`
# above is safe by contrast: it passes names, never values.)
#
# Written as an `if` rather than `[[ ... ]] && set -x` on purpose: under `set -e`
# a bare `&&` list whose test fails returns non-zero and would kill the script.
if [[ -n "${DEVELOP_TRACE:-}" ]]; then
    set -x
fi


case $args in
editor)
    restart_editor
    ;;

search)
    restart_search
    ;;

watch)
    watch_watchman
    ;;

watches)
    configure_watches
    ;;

yarn)
    yarn_install
    restart_editor
    restart_search
    ;;

api)
    restart_api
    ;;

*)
    echo "Unknown command: $args"
    ;;
esac
