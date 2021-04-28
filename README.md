# What is this?

It is a [PuzzleScript][puzzlescript-url] interpreter written in Rust to play games in your terminal!

# Screencaps

Here are some screencaps of games being played.

### Gravirinth ([original](https://pedropsi.github.io/gravirinth.html))

(click to see the ascii screencast)

<a href="https://asciinema.org/a/262824"><img width="300" alt="video of the beginning of Gravirinth" src="https://asciinema.org/a/262824.png"/></a>

### Mirror Isles ([original](http://www.draknek.org/games/puzzlescript/mirrors.php))

This screencast shows playing the game in a terminal using ASCII and ANSI colors.

![mirror-isles](https://user-images.githubusercontent.com/253202/47133542-ce0d1700-d26e-11e8-851f-233d27aaf0b8.gif)


### Pot Wash Panic! ([original](https://hauntpun.itch.io/pot-wash-panic))

(click to see the ascii screencast)

<a href="https://asciinema.org/a/188014?t=25"><img width="300" alt="video of install and a couple games" src="https://asciinema.org/a/188014.png"/></a>


### Hack the Net ([original](http://www.draknek.org/games/puzzlescript/hack-the-net.php))

<a href="https://asciinema.org/a/188016"><img width="300" alt="video of a couple levels of Hack-the-Net" src="https://asciinema.org/a/188016.png"/></a>

### Skipping Stones to Lonely Homes ([original](http://www.draknek.org/games/puzzlescript/skipping-stones.php))

<a href="https://asciinema.org/a/189279?t=20"><img width="300" alt="video of the beginning of Skipping Stones (BIG)" src="https://asciinema.org/a/189279.png"/></a>

### Entanglement ([original](http://www.richardlocke.co.uk/release/entanglement/chapter-1/))

<a href="https://asciinema.org/a/212372?t=18"><img width="300" alt="video of the beginning of Entanglement" src="https://asciinema.org/a/212372.png"/></a>


# Install

1. Install Rust
1. Clone this repository
1. Run `cargo run --release` to get the help message
1. Run `cargo run --release -- ./games/{game}.parsed.json` to play a game
    - As you complete levels, it will create a save file in the same directory as the game

# Test

- `./test.bash` : runs all the tests
- `cargo test` : runs unit tests
- `./test_solutions.bash` : replays real games and verifies the solutions still work

## Flamegraph

Flamegraphs are great for finding performance problems. Here's how to generate one:

```bash
echo "DDD" | sudo cargo flamegraph ./games/roll-those-sixes.parsed.json -- --level 0 --scripted
echo "S.D.S.D.DWAASAWWW..............q" | sudo cargo flamegraph ./games/entanglement-one.parsed.json -- --level 3 --scripted
```

# TODO

- [ ] Build WebAssembly version and add example of using it in a browser
- [ ] Add a parser so the original `script.txt` files can be used instead of the `*.parsed.json` files


[puzzlescript-url]: https://github.com/philschatz/puzzlescript