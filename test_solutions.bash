cd "$(dirname "$0")" || exit 111
root_dir=$(pwd)
echo "" > stats.txt # clear the file
for game_parsed_json in $(ls "${root_dir}"/games/*.solutions.json); do
    game_parsed="${game_parsed_json%.*}"
    game="${game_parsed%.*}"
    game=$(basename "${game}")

    if [[ $1 != "" && $1 != ${game} ]]; then
        continue
    fi

    already_solved=$(grep "Solved ${game}" ./stats-prev.txt)
    if [[ -f stats-prev.txt && ${already_solved} != "" ]]; then
        echo "Skipping ${game} because it was alerady solved"
        echo "Skipping ${game} because it was alerady solved in a previous iteration." >> stats.txt
        echo "${already_solved}" >> stats.txt
        continue
    fi

    attempted_solutions=0 # Stop after attempting 2 solutions
    index=0
    solutions=$(jq --raw-output ".solutions[].solution" "./games/${game}.solutions.json")
    for solution in ${solutions}; do
        # Stop after attempting 2
        if [[ ${attempted_solutions} -ge 2 ]]; then
            break
        fi
        if [[ ${solution} != "!" && ${solution} != ",!" && ${solution} != "." && ${solution} != ".,,,,," && ${solution} != ".!" && ${solution} != "null" ]]; then
            start=`date +%s`
            echo "echo" -n "'${solution}' | " cargo run --release "./games/${game}.parsed.json" --level "${index}" --scripted
            echo -n "${solution}" | cargo run --release "./games/${game}.parsed.json" --level "${index}" --scripted
            exit_status=$?
            end=`date +%s`
            runtime=$((end-start))
            if [[ ${exit_status} == 0 ]]; then
                echo "Solved ${game} index=${index}" >> stats.txt
            else
                echo "FAILED ${game} index=${index} status=${exit_status}" >> stats.txt
            fi
            ((attempted_solutions++))
        fi
        ((index++))
    done
done
