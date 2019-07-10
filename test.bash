#!/bin/bash

try() {
    "${@}"
    status=$?
    if [[ ${status} != 0 ]]; then
        echo "ERROR: Failed to run '${@}'" > /dev/stderr
        exit ${status}
    fi
}

# Check how long a command takes to run. Fail if it is too slow.
bench() {
    expected=$1
    start=$(date +%s)
    "${@:2}"
    end=$(date +%s)
    runtime=$((end-start))
    if [[ ${runtime} > ${expected} ]]; then
        echo "Took too long. Expected ${expected}sec but actually took ${runtime}sec"
        exit 111
    else
        echo "OK. Took ${runtime}sec to run '${@:2}'. Limit: ${expected}"
    fi
}

cargo fmt # might not be installed
try cargo build --release

echo "..........q" | try bench 4 cargo run --release ./games/skipping-stones.parsed.json --primary --level 0 --scripted

try cargo test

try bench 300 ./test_solutions.bash
