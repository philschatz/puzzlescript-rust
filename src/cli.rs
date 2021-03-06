extern crate env_logger;
extern crate hex;
extern crate rand;
extern crate rand_xorshift;
extern crate serde;
extern crate serde_json;
extern crate termion;

use log::{debug, info};
use std::error::Error;
use std::fs::File;
use std::io::stdin;
use std::io::stdout;
use std::process;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time;

use termion::event::Key;
use termion::input::TermRead;

use crate::color::ColorSpace;
use crate::debugger::ScreenDumper;
use crate::engine::Engine;
use crate::engine::EngineInput;
use crate::model::board::Board;
use crate::model::game::GameData;
use crate::model::game::SpriteLookup;
use crate::parser;
use crate::save::SaveState;
use crate::terminal::Attribution;
use crate::terminal::Help;
use crate::terminal::PlayPause;
use crate::terminal::RecordingInfo;
use crate::terminal::Spinner;

use termion::screen::AlternateScreen;
use tui::backend::Backend;
use tui::backend::TermionBackend;
use tui::layout::Rect;
use tui::widgets::Widget;
use tui::Terminal;

const IDLE_SECS: u64 = 60;

pub fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let matches = clap_app!(myapp =>
        (about: "Play Puzzle Games in Rust")
        (@arg INPUT: +required "Game file to play")
        (@arg START_LEVEL: --level -l +takes_value "Which level to start playing")
        (@arg IS_SCRIPTED: --scripted "Play 1 level using stdin. Used for running tests")
        (@arg SOUND: --sound "Play sound effects (via the BEL character)")
        (@arg FORCE_PRIMARY_SCREEN: --primary "Do not use the alternate screen (useful for debugging)")
        (@arg NO_FLICK_SCREEN: --noflick "Show the WHOLE level not just the current screen (for finding easter-eggs)")
        (@arg NO_SAVE: --nosave "Do not save")
        (@arg USE_TEST_REPLAY_FILE: --test "Use the {game}.test-replay.json file to replay the games and to save to instead of the default {game}.save.json file")
        (@arg TICK_SPEED: --speed +takes_value "How long the game waits between each tick")
    ).get_matches();

    let game_path = matches.value_of("INPUT").unwrap();
    let start_level = matches
        .value_of("START_LEVEL")
        .map(|s| s.parse().expect("Enter a valid number"));
    let scripted = matches.is_present("IS_SCRIPTED");
    let enable_sound = matches.is_present("SOUND");
    let force_primary_screen = matches.is_present("FORCE_PRIMARY_SCREEN");
    let no_flick_screen = matches.is_present("NO_FLICK_SCREEN");
    let no_save = matches.is_present("NO_SAVE");
    let is_use_test_replay_file = matches.is_present("USE_TEST_REPLAY_FILE");
    let tick_speed = matches
        .value_of("TICK_SPEED")
        .map(|s| s.parse().expect("Enter a valid number"));

    let is_stdin_tty = is_tty("/dev/stdin");

    if scripted || force_primary_screen {
        // Terminal initialization
        let out = stdout();
        let backend = TermionBackend::new(out);
        let mut t = Terminal::new(backend)?;
        play_game(
            &mut t,
            &game_path,
            start_level,
            scripted,
            enable_sound,
            no_flick_screen,
            tick_speed,
            no_save,
            is_stdin_tty,
            is_use_test_replay_file,
        )
    } else {
        // Terminal initialization
        let out = stdout();
        let out = AlternateScreen::from(out);
        let backend = TermionBackend::new(out);
        let mut t = Terminal::new(backend)?;
        t.hide_cursor()?;

        play_game(
            &mut t,
            &game_path,
            start_level,
            scripted,
            enable_sound,
            no_flick_screen,
            tick_speed,
            no_save,
            is_stdin_tty,
            is_use_test_replay_file,
        )
    }
}

fn play_game<B: Backend>(
    terminal: &mut Terminal<B>,
    path: &str,
    start_level: Option<u8>,
    scripted: bool,
    enable_sound: bool,
    no_flick_screen: bool,
    tick_speed: Option<u64>,
    no_save: bool,
    is_stdin_tty: bool,
    is_use_test_replay_file: bool,
) -> Result<(), Box<dyn Error>> {
    let save_path = if is_use_test_replay_file {
        format!("{}.test-replay.json", path)
    } else {
        format!("{}.save.json", path)
    };
    let mut game = read_game_from_file(path)?;

    if no_flick_screen {
        game.metadata.flickscreen = None;
        game.metadata.zoomscreen = None;
    }

    warn_if_alpha_transparency(&game);

    let sprite_lookup = SpriteLookup::new(&game.sprites);

    let mut attribution = Attribution::new(
        game.title.clone(),
        game.metadata.author.clone(),
        game.metadata.homepage.clone(),
    );
    let mut spinner = Spinner::new();
    let mut help = Help::new();
    let mut play_pause = PlayPause::new();
    let mut recording_info = RecordingInfo::default();
    let mut debug_keypresses = String::from("");

    let (start_level, checkpoint, mut inputs) = SaveState::read_from_file(&save_path)
        .map(|save_data| {
            match start_level {
                None => {
                    let level = &game.levels[save_data.level as usize];
                    let checkpoint = save_data.checkpoint.map(|checkpoint| {
                        let (width, height) = level.size();
                        Board::from_checkpoint(
                            width,
                            height,
                            checkpoint
                                .iter()
                                .map(|names| {
                                    names
                                        .iter()
                                        .map(|name| sprite_lookup.to_id(name).unwrap().clone())
                                        .collect()
                                })
                                .collect(),
                        )
                    });
                    (save_data.level, checkpoint, save_data.inputs)
                }
                Some(level_num) => {
                    // clear the current level since we will be playing it
                    let mut inputs = save_data.inputs.clone();
                    if inputs.len() >= level_num as usize {
                        inputs[level_num as usize] = String::from("");
                    }
                    (level_num, None, inputs)
                }
            }
        })
        .unwrap_or_else(|_| (start_level.unwrap_or(0), None, vec![]));

    fn add_input(
        inputs: &mut Vec<String>,
        debug_keypresses: &mut String,
        current_level_num: u8,
        input: char,
    ) {
        while inputs.len() <= current_level_num as usize {
            inputs.push(String::from(""))
        }
        inputs[current_level_num as usize].push(input);
        debug_keypresses.push(input);
    }

    let mut engine = match checkpoint {
        None => Engine::new(game, start_level),
        Some(checkpoint) => Engine::from_checkpoint(game, start_level, checkpoint),
    };

    // Enable raw mode so we get keys
    if !scripted && is_stdin_tty {
        ScreenDumper::set_term();
    }

    clear_screen();

    let mut sleep_time = match tick_speed {
        None => {
            if scripted {
                0
            } else {
                match &engine.game_data.metadata.realtime_interval {
                    None => 100,
                    Some(sec) => (sec * 1000.0) as u64,
                }
            }
        }
        Some(s) => s,
    };

    let save_game = |current_level_num: u8,
                     inputs: Vec<String>,
                     board: Option<Board>|
     -> Result<(), Box<dyn Error>> {
        if no_save {
            return Ok(());
        }
        let checkpoint = board.map(|board| {
            board
                .positions_iter()
                .iter()
                .map(|p| {
                    board
                        .get_sprite_states(&p)
                        .iter()
                        .map(|s| sprite_lookup.to_name(&s.index).unwrap().clone())
                        .collect()
                })
                .collect()
        });
        let save = SaveState {
            version: 1,
            inputs,
            level: current_level_num,
            checkpoint: checkpoint,
        };
        save.write_to_file(&save_path)
    };

    let mut keys = 0;
    let mut scripted_did_win = false;
    let mut tick_without_input = engine
        .game_data
        .metadata
        .run_rules_on_level_start
        .unwrap_or(false);
    let mut last_input = time::Instant::now();
    let (stdin_channel, _handle) = spawn_stdin_channel();
    sleep(100); // wait for thread to look into stdin
    loop {
        let start_tick = time::Instant::now();
        let mut should_tick;
        let mut input = None;

        let key = stdin_channel.try_recv();
        debug!("Input: Received {:?}", key);
        match key {
            Ok(key) => {
                last_input = time::Instant::now();
                let was_paused = play_pause.paused;
                play_pause.resume();
                should_tick = match key {
                    Key::Esc | Key::Ctrl('c') | Key::Char('q') => break,
                    Key::Char('?') | Key::Char('h') => {
                        help.toggle();
                        false
                    }
                    Key::Up | Key::Char('w') | Key::Char('W') => {
                        input = input.or(Some(EngineInput::Up));
                        keys += 1;
                        true
                    }
                    Key::Down | Key::Char('s') | Key::Char('S') => {
                        input = input.or(Some(EngineInput::Down));
                        keys += 1;
                        true
                    }
                    Key::Left | Key::Char('a') | Key::Char('A') => {
                        input = input.or(Some(EngineInput::Left));
                        keys += 1;
                        true
                    }
                    Key::Right | Key::Char('d') | Key::Char('D') => {
                        input = input.or(Some(EngineInput::Right));
                        keys += 1;
                        true
                    }
                    Key::Char(' ') | Key::Char('x') | Key::Char('X') | Key::Char('!') => {
                        input = input.or(Some(EngineInput::Action));
                        keys += 1;
                        true
                    }
                    Key::Char('\n') => {
                        if !is_stdin_tty {
                            false // newlines via stdin are just ignored
                        } else {
                            input = input.or(Some(EngineInput::Action));
                            keys += 1;
                            true
                        }
                    }
                    Key::Char('z') | Key::Char('Z') | Key::Char('u') => {
                        input = input.or(Some(EngineInput::Undo));
                        keys += 1;
                        true
                    }
                    Key::Char('R') | Key::Char('r') => {
                        input = input.or(Some(EngineInput::Restart));
                        keys += 1;
                        true
                    }
                    Key::Char('p') => {
                        if !was_paused {
                            play_pause.pause()
                        };
                        false
                    }
                    Key::Char('c') => {
                        terminal.draw(|_|{})?/*repaint*/;
                        false
                    }
                    // Solution files keys. These are not pressed, they are piped in
                    Key::Char('#') => false,
                    Key::Char('.') | Key::Char(',') => true,
                    Key::Null => {
                        println!("Done reading input");
                        process::exit(111)
                    }
                    // Debugging
                    Key::Char('$') => {
                        println!("Key dump: {:?}", debug_keypresses);
                        process::exit(0)
                    }
                    Key::Char('~') | Key::Char('`') | Key::Char('\\') => {
                        if is_stdin_tty {
                            // ensure the dumper can enable/disable raw mode
                            ScreenDumper::set_term();
                        }
                        engine.debug_rules = !engine.debug_rules;
                        if engine.debug_rules {
                            terminal.draw(|_|{})?/*repaint*/;
                            true
                        } else {
                            clear_screen();
                            false
                        }
                    }
                    Key::Char('n') => engine.debug_rules,
                    Key::Char('-') | Key::Char('_') => {
                        if sleep_time >= 50 {
                            sleep_time -= 50;
                            play_bell()
                        };
                        false
                    }
                    Key::Char('=') | Key::Char('+') => {
                        if sleep_time < 1000 {
                            sleep_time += 50;
                            play_bell()
                        };
                        false
                    }
                    _ => true,
                };
            }
            Err(TryRecvError::Empty) => {
                if scripted {
                    if scripted_did_win {
                        break;
                    }
                    if keys > 0 {
                        panic!("Level did not complete. Maybe more input is needed or more likely, the logic is flawed");
                    } else {
                        panic!(
                            "BUG: Level did not complete but no keys were pressed. Maybe more input is needed"
                        );
                    }
                }

                if !is_stdin_tty {
                    return Ok(());
                }
                should_tick = !engine.debug_rules; // Do not tick when debugger is on.

                // Pause the game when idle (only for realtime games)
                if !play_pause.paused
                    && engine.game_data.metadata.realtime_interval.is_some()
                    && last_input.elapsed().as_secs() > IDLE_SECS
                {
                    play_pause.pause();
                    should_tick = false;
                }

                if engine.game_data.metadata.realtime_interval.is_none() {
                    should_tick = false
                }
            }
            Err(TryRecvError::Disconnected) => panic!("Channel disconnected"),
        }

        if tick_without_input {
            should_tick = true;
        }

        if !should_tick {
            if !engine.debug_rules {
                // *******************************************
                //   This is Copy/Pasta'd in multiple places
                // *******************************************
                terminal.draw(|mut f| {
                    let size = f.size();
                    let top = Rect::new(size.x, size.y, size.width, 1);
                    let main = Rect::new(size.x, size.y + 1, size.width, size.height - 2);
                    let bottom = Rect::new(size.x, main.bottom(), size.width, 1);

                    engine.render(&mut f, main);
                    play_pause.render(&mut f, main);
                    attribution.render(&mut f, top);
                    help.render(&mut f, bottom);
                    spinner.render(&mut f, bottom);
                    if !is_stdin_tty {
                        recording_info.render(&mut f, bottom);
                    }
                })?;
            }

            // Copy/Pasta
            let elapsed_time = start_tick.elapsed().as_millis();
            if elapsed_time < sleep_time as u128 {
                sleep(sleep_time - (elapsed_time as u64));
            }

            continue;
        }

        if play_pause.paused {
            sleep(100);
            continue;
        }

        // Tick!
        let tr = engine.tick(input);

        tick_without_input = !tr.accepting_input;

        if !engine.debug_rules {
            // *******************************************
            //   This is Copy/Pasta'd in multiple places
            // *******************************************
            terminal.draw(|mut f| {
                let size = f.size();
                let top = Rect::new(size.x, size.y, size.width, 1);
                let main = Rect::new(size.x, size.y + 1, size.width, size.height - 2);
                let bottom = Rect::new(size.x, main.bottom(), size.width, 1);

                engine.render(&mut f, main);
                play_pause.render(&mut f, main);
                attribution.render(&mut f, top);
                help.render(&mut f, bottom);
                spinner.render(&mut f, bottom);
                if !is_stdin_tty {
                    recording_info.render(&mut f, bottom);
                }
            })?;
        }

        if tr.changed {
            let tick_char = if tr.accepting_input { '.' } else { ',' };
            add_input(
                &mut inputs,
                &mut debug_keypresses,
                engine.current_level_num,
                input.map(|i| i.to_key()).unwrap_or(tick_char),
            );
        }

        if tr.sfx && enable_sound {
            play_bell();
        }

        if tr.completed_level.is_some() {
            scripted_did_win = true;
            if !engine.next_level() {
                save_game(engine.current_level_num, inputs.clone(), None)?;
                println!("You beat all the levels in the game!");
                break;
            }
        }

        if tr.checkpoint.is_some() {
            recording_info.increment();
        }

        if tr.checkpoint.is_some() {
            add_input(
                &mut inputs,
                &mut debug_keypresses,
                engine.current_level_num,
                '#',
            );
            save_game(engine.current_level_num, inputs.clone(), tr.checkpoint)?;
        }

        if tr.completed_level.is_some() {
            save_game(engine.current_level_num, inputs.clone(), None)?;
        }

        // Copy/Pasta
        let elapsed_time = start_tick.elapsed().as_millis();
        if elapsed_time < sleep_time as u128 {
            sleep(sleep_time - (elapsed_time as u64));
        }
    }
    Ok(())
}

fn read_game_from_file(path: &str) -> Result<GameData, Box<dyn Error>> {
    info!("Reading {:?}", path);

    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let game = parser::parse(file)?;

    info!("Parsed {:?}", path);

    Ok(game)
}

// https://stackoverflow.com/a/55201400
fn spawn_stdin_channel() -> (Receiver<Key>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<Key>();
    let handle = thread::spawn(move || loop {
        for key in stdin().keys() {
            let key = key.unwrap();
            debug!("Sending {:?}", key);
            tx.send(key).unwrap();
        }
    });
    (rx, handle)
}

fn sleep(millis: u64) {
    let duration = time::Duration::from_millis(millis);
    thread::sleep(duration);
}

fn clear_screen() {
    print!(
        "{}{}{}",
        termion::color::Fg(termion::color::Reset),
        termion::color::Bg(termion::color::Reset),
        termion::clear::All
    );
}
fn play_bell() {
    print!("\x07")
}

fn warn_if_alpha_transparency(game: &GameData) {
    if !ColorSpace::get_colorspace().is_true_color() {
        for sprite in game.sprites.values() {
            if sprite.contains_alpha_pixel() {
                println!("\n\n\n\n{}WARNING:{} This game uses color gradients. Consider using a TRUECOLOR supported terminal.", termion::color::Fg(termion::color::Yellow), termion::style::Reset);
                sleep(5000);
                return;
            }
        }
    }
}

fn is_tty(file: &str) -> bool {
    File::create(file)
        .map(|f| termion::is_tty(&f))
        .unwrap_or(false)
}
