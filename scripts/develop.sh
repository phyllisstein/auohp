#!/usr/bin/env bash

set -Eeuxo pipefail

args="$*"

restart_server() {
  echo "Starting development server..."
  pkill -if "start:dev" || true

  [[ -e "/run/secrets/environment" ]] || { echo "Missing environment secrets." && exit 1; }
  source /run/secrets/environment && export GSAP_NPM_TOKEN FONTAWESOME_NPM_TOKEN GITHUB_TOKEN
  yarn packages:dev &
  disown
}

configure_watches() {
  echo "Configuring watches..."

  watchman watch-del-all || true
  watchman watch-project "$PWD"
  for j in scripts/watchman/*.json; do
    echo "Setting watch $j"
    watchman -j <"$j"
  done
}

watch_watchman() {
  pkill -if watchman || true
  watchman --logfile=- --log-level=debug --foreground watch-project "/app"
}

yarn_install() {
  echo "Running yarn install..."
  [[ -e "/run/secrets/environment" ]] || { echo "Missing environment secrets." && exit 1; }
  source /run/secrets/environment && export GSAP_NPM_TOKEN FONTAWESOME_NPM_TOKEN GITHUB_TOKEN
  yarn install
}

case $args in
serve)
  # restart_server
  yarn_install
  configure_watches
  watch_watchman
  ;;

watch)
  yarn_install
  # restart_server
  configure_watches
  watch_watchman
  ;;

watches)
  configure_watches
  ;;

yarn)
  yarn_install
  restart_server
  ;;

*)
  echo "Unknown command: $args"
  ;;
esac
