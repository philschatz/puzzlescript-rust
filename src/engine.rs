use std::fmt;

use log::debug;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use crate::model::board::Board;
use crate::model::game::GameData;
use crate::model::game::Input;
use crate::model::game::Level;
use crate::model::util::Position;

#[derive(Debug)]
pub struct Engine {
    rng: XorShiftRng,
    pub game_data: GameData,
    pub current_level: BoardOrMessage,
    undo_stack: Vec<Board>,
    pub current_level_num: u8,
    pub debug_rules: bool,
    pub pending_message: Option<String>,
}

// The main enchilada. Pass in a game and a level and then just call engine.tick(Some(EngineInput::Right))
impl Engine {
    pub fn new(game_data: GameData, current_level_num: u8) -> Self {
        let current = &game_data.levels[current_level_num as usize];
        let current_level = match current {
            Level::Message(message) => BoardOrMessage::Message(message.clone()),
            Level::Map(_) => BoardOrMessage::Board(game_data.to_board(&current)),
        };

        Self {
            rng: new_rng(),
            game_data,
            current_level,
            current_level_num,
            undo_stack: vec![],
            debug_rules: false,
            pending_message: None,
        }
    }

    pub fn from_checkpoint(game_data: GameData, current_level_num: u8, checkpoint: Board) -> Self {
        let current_level = BoardOrMessage::Board(checkpoint);

        Self {
            rng: new_rng(),
            game_data,
            current_level,
            current_level_num,
            undo_stack: vec![],
            debug_rules: false,
            pending_message: None,
        }
    }

    pub fn tick(&mut self, input: Option<EngineInput>) -> TickResult {
        let mut changed = false;
        match input {
            None => debug!("Tick start"),
            Some(i) => debug!("Tick start. Player pressed {}", i),
        }

        if self.pending_message.is_some() {
            match input {
                Some(EngineInput::Action) => {
                    self.pending_message = None;
                    return TickResult::empty().affected();
                }
                _ => return TickResult::empty(),
            }
        }

        match &self.current_level {
            BoardOrMessage::Board(board) => {
                let mut pressed = false;
                let mut new = board.clone();
                match input {
                    None => {}
                    Some(EngineInput::Up) => {
                        pressed = true;
                        self.game_data
                            .evaluate_player_input(&mut self.rng, &mut new, Input::Up)
                    }
                    Some(EngineInput::Down) => {
                        pressed = true;
                        self.game_data
                            .evaluate_player_input(&mut self.rng, &mut new, Input::Down)
                    }
                    Some(EngineInput::Left) => {
                        pressed = true;
                        self.game_data
                            .evaluate_player_input(&mut self.rng, &mut new, Input::Left)
                    }
                    Some(EngineInput::Right) => {
                        pressed = true;
                        self.game_data
                            .evaluate_player_input(&mut self.rng, &mut new, Input::Right)
                    }
                    Some(EngineInput::Action) => {
                        pressed = true;
                        self.game_data
                            .evaluate_player_input(&mut self.rng, &mut new, Input::Action)
                    }
                    Some(EngineInput::Restart) => match self.undo_stack.first() {
                        None => {}
                        Some(b) => new = b.clone(),
                    },
                    Some(EngineInput::Undo) => match self.undo_stack.pop() {
                        None => {}
                        Some(b) => new = b,
                    },
                }
                let t = self
                    .game_data
                    .evaluate(&mut self.rng, &mut new, self.debug_rules);

                let mut new_board = None;
                if !t.cancel {
                    if t.message.is_some() {
                        self.pending_message = Some(t.message.unwrap().clone());
                    }

                    if t.checkpoint {
                        debug!("Checkpoint reached. Clearing Undo Stack");
                        self.undo_stack.clear();
                    }

                    changed = new != *board;

                    if pressed && changed {
                        debug!("Pushing to the Undo Stack");
                        // Keep the undo stack at a manageable size
                        if self.undo_stack.len() > 100 {
                            self.undo_stack.drain(1..50);
                        }
                        self.undo_stack.push(board.clone());
                    }

                    new_board = Some(BoardOrMessage::Board(new));
                }
                match new_board {
                    None => {}
                    Some(n) => self.current_level = n,
                }
                TickResult {
                    changed: changed && !t.cancel,
                    completed_level: if t.win {
                        Some(self.current_level_num)
                    } else {
                        None
                    },
                    checkpoint: if t.checkpoint {
                        Some(self.current_level.unwrap_board().clone())
                    } else {
                        None
                    },
                    accepting_input: !t.again,
                    sfx: t.sfx,
                }
            }
            BoardOrMessage::Message(_) => match input {
                Some(EngineInput::Action) => {
                    TickResult::empty().affected().win(self.current_level_num)
                }
                _ => TickResult::empty(),
            },
        }
    }

    pub fn player_position(&self) -> Option<Position> {
        match &self.current_level {
            BoardOrMessage::Message(_) => None,
            BoardOrMessage::Board(board) => {
                let matches: Vec<Position> = board
                    .positions_iter()
                    .iter()
                    .filter(|&p| board.matches(p, &self.game_data.player_tile, &None))
                    .map(|&p| p)
                    .collect();

                if matches.len() != 1 {
                    None
                } else {
                    match matches.get(0) {
                        None => None,
                        Some(p) => Some(p.clone()),
                    }
                }
            }
        }
    }

    pub fn next_level(&mut self) -> bool {
        self.current_level_num = self.current_level_num + 1;
        self.pending_message = None;

        if self.current_level_num as usize >= self.game_data.levels.len() {
            return false;
        }
        let current = &self.game_data.levels[self.current_level_num as usize];
        self.current_level = match current {
            Level::Message(message) => BoardOrMessage::Message(message.clone()),
            Level::Map(_) => BoardOrMessage::Board(self.game_data.to_board(&current)),
        };
        self.undo_stack.clear();
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EngineInput {
    Up,
    Down,
    Left,
    Right,
    Action,
    Undo,
    Restart,
}

impl EngineInput {
    pub fn to_key(&self) -> char {
        match self {
            EngineInput::Up => 'W',
            EngineInput::Down => 'S',
            EngineInput::Left => 'A',
            EngineInput::Right => 'D',
            EngineInput::Action => 'X',
            EngineInput::Undo => 'Z',
            EngineInput::Restart => 'R',
        }
    }
}

impl fmt::Display for EngineInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            EngineInput::Up => "UP",
            EngineInput::Down => "DOWN",
            EngineInput::Left => "LEFT",
            EngineInput::Right => "RIGHT",
            EngineInput::Action => "ACTION",
            EngineInput::Undo => "UNDO",
            EngineInput::Restart => "RESTART",
        };
        write!(f, "{}", msg)
    }
}

fn new_rng() -> XorShiftRng {
    XorShiftRng::from_seed([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
}

#[derive(Debug)]
pub enum BoardOrMessage {
    Message(String),
    Board(Board),
}

impl BoardOrMessage {
    fn unwrap_board(&self) -> &Board {
        match self {
            BoardOrMessage::Message(_) => panic!("called `unwrap_board()` on a `Message` value"),
            BoardOrMessage::Board(b) => b,
        }
    }
}

pub struct TickResult {
    pub changed: bool,
    pub completed_level: Option<u8>,
    pub checkpoint: Option<Board>,
    pub accepting_input: bool,
    pub sfx: bool,
}

impl TickResult {
    fn empty() -> Self {
        Self {
            changed: false,
            completed_level: None,
            checkpoint: None,
            accepting_input: true,
            sfx: false,
        }
    }

    fn win(mut self, level: u8) -> Self {
        self.completed_level = Some(level);
        self
    }

    fn affected(mut self) -> Self {
        self.changed = true;
        self
    }
}
