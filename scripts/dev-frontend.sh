#!/usr/bin/env bash

set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend_dir="$repo_root/crates/cm-web/frontend"

cd "$frontend_dir"

child_pid=""
shutting_down=0

forward_shutdown() {
  shutting_down=1
  if [[ -n "$child_pid" ]] && kill -0 "$child_pid" 2>/dev/null; then
    kill -INT "$child_pid" 2>/dev/null || true
  fi
}

trap 'forward_shutdown' INT TERM

./node_modules/.bin/vite &
child_pid=$!

status=0
wait "$child_pid" || status=$?

if [[ $shutting_down -eq 1 ]]; then
  exit 0
fi

exit "$status"
