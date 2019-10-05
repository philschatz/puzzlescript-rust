cd "$(dirname "$0")" || exit 111
root_dir=$(pwd)

replay_game() {
    solution=$1
    game=$2
    index=$3
    extra_arg=$4

    start=`date +%s`
    echo "echo" -n "'${solution}' | " cargo run --release "./games/${game}.parsed.json" --level "${index}" --scripted ${extra_arg}
    echo -n "${solution}" | cargo run --release "./games/${game}.parsed.json" --level "${index}" --scripted ${extra_arg}
    exit_status=$?
    end=`date +%s`
    runtime=$((end-start))
    if [[ ${extra_arg} != "" ]]; then
        if [[ ${exit_status} == 0 ]]; then
            echo "Solved ${game} index=${index}" >> stats.txt
        else
            echo "FAILED ${game} index=${index} status=${exit_status}" >> stats.txt
        fi
    fi
    return ${exit_status}
}


if [[ "$(command -v jq)" == "" ]]; then
    echo "Install jq before running tests: https://stedolan.github.io/jq/download/"
    exit 111
fi

echo "" > stats.txt # clear the file

for game_parsed_json in $(ls "${root_dir}"/games/*.parsed.json); do
    game_parsed="${game_parsed_json%.*}"
    game="${game_parsed%.*}"
    game=$(basename "${game}")

    if [[ $1 != "" && $1 != ${game} ]]; then
        continue
    fi

    if [[ -f "${root_dir}/games/${game}.parsed.json.test-replay.json" ]]; then

        if [[ -f "${root_dir}/games/${game}.parsed.json.save.json" ]]; then
            echo "It seems like you solved a new part of this game. Consider running 'mv ${root_dir}/games/${game}.parsed.json.save.json ${root_dir}/games/${game}.parsed.json.test-replay.json' to update the test file"
            exit 110
        fi

        solutions=$(jq --raw-output ".inputs[]" "./games/${game}.parsed.json.test-replay.json")

    else
        echo "Skipping ${game} because no solutions were found" >> stats.txt
        continue
    fi

    attempted_solutions=0 # Stop after attempting 2 solutions
    index=0
    for solution in ${solutions}; do
        # Stop after attempting 2
        if [[ ${attempted_solutions} -ge 99 ]]; then
            break
        fi
        if [[ ${solution} == *"#"* ]]; then
            echo "Skipping ${game} level=${index} because it contains checkpoints" >> stats.txt
            ((index++))
            continue
        fi
        if [[ ${solution} != "X" && ${solution} != "!" && ${solution} != "?" && ${solution} != ",!" && ${solution} != "." && ${solution} != ".,,,,," && ${solution} != ".!" && ${solution} != "null" && ${solution} != "" ]]; then
            replay_game "${solution}" "${game}" "${index}" --nosave
            # If we replayed successfully then play it again and save the .save.json file
            # if [[ $? == 0 ]]; then
            #     replay_game "${solution}" "${game}" "${index}" ""
            # fi
            ((attempted_solutions++))
        fi
        ((index++))
    done

done
