This directory stores game files parsed as JSON and solutions to game levels.

The game files are generated using https://github.com/philschatz/puzzlescript and running the following commands:

```sh
yarn compile:ts
node ./build-games-json.js
# mv games/*.json ./path-to-this-directory/
```

The solution files are just moved here and renamed with the `.solutions.json` suffix.