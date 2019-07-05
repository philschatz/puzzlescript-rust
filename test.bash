#!/bin/bash

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

cargo build --release || exit 110

echo "..........q" | bench 4 cargo run --release ./games/skipping-stones.parsed.json --primary --level 0 --scripted || exit 110

cargo test || exit 110

cargo fmt # might not be installed

bench 300 ./test_solutions.bash || exit 110
