#!/usr/bin/env bash

set -Eeuxo pipefail

args="$*"

restart_editor() {
    echo "Terminate existing editor..."
    pkill -f "packages/editor" || true

    echo "Starting editor development server..."
    cd /app/packages/editor
    yarn start:dev &
    disown
}

restart_api() {
    echo "Terminate existing API..."
    pkill -f "auohp-api" || true

    echo "Starting API development server..."
    cd /app/packages/api
    ./scripts/download-models.sh
    cargo run &
    disown
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
    pkill -f "yarn install" || true
    echo "Running yarn install..."
    yarn install
}


[[ -e "/run/secrets/environment" ]] || { echo "Missing secret environment file." && exit 0; }
source /run/secrets/environment && export NEO4J_PASSWORD NEO4J_USERNAME NEO4J_URI NEO4J_DATABASE HF_TOKEN


case $args in
editor)
    restart_editor
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
    ;;

api)
    restart_api
    ;;

*)
    echo "Unknown command: $args"
    ;;
esac
