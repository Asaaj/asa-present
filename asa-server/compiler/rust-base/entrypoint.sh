#!/bin/bash

set -eu

log="$HOME/log-modify"
log_args="$HOME/log-args"

if [[ -z "${PLAYGROUND_ORCHESTRATOR:-}" ]]; then
    timeout=${PLAYGROUND_TIMEOUT:-10}

    modify-cargo-toml >> "${log}"
    echo "$@" >> "${log_args}"

    # Don't use `exec` here. The shell is what prints out the useful
    # "Killed" message
    timeout --signal=KILL ${timeout} "$@"
else
    exec "$@"
fi
