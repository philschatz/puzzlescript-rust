#!/bin/bash
set -e

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
        echo "Took ${runtime} to run '${@:2}'"
    fi
}

cargo build --release

echo "..........q" | bench 4 cargo run --release ./games/skipping-stones.parsed.json --primary

cargo test

bench 300 ./test_solutions.bash
