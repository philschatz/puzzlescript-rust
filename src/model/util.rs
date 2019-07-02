#[cfg(feature = "serde")]
extern crate serde;

use std::fmt;
use std::hash;
use std::cmp::{Eq, Ordering};

use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Dimension {
    pub width: u16,
    pub height: u16,
}


#[derive(Copy, Clone, Debug)] // Hash is implemented below
pub struct SpriteState {
    name: [char; 20],
    pub index: u16,
    pub collision_layer: u16
}

impl SpriteState {
    pub fn new(name: &String, index: u16, collision_layer: u16) -> Self {
        let mut name_chars = [' ';20];
        for i in 0..name_chars.len() {
            match name.chars().nth(i) {
                None => name_chars[i] = ' ',
                Some(c) => name_chars[i] = c,
            }
        }
        Self {
            name: name_chars,
            index,
            collision_layer,
        }
    }
}

impl hash::Hash for SpriteState {
    // ignore the name field
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.collision_layer.hash(state);
        self.index.hash(state);
    }
}
impl Ord for SpriteState {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.collision_layer.cmp(&other.collision_layer) {
            Ordering::Equal => self.index.cmp(&other.index),
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
        }
    }
}

impl PartialOrd for SpriteState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(match self.collision_layer.cmp(&other.collision_layer) {
            Ordering::Equal => self.index.cmp(&other.index),
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
        })
    }
}

impl PartialEq for SpriteState {
    fn eq(&self, other: &Self) -> bool {
        self.collision_layer == other.collision_layer && self.index == other.index
    }
}

impl Eq for SpriteState { }

impl fmt::Display for SpriteState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let chars: String = self.name.iter().collect();
        write!(f, "{}[{}]", chars.trim(), self.index)
    }
}


#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WantsToMove {
    Stationary,
    Up,
    Down,
    Left,
    Right,
    Action,
    RandomDir,
}

impl WantsToMove {
    pub fn to_cardinal_direction(&self) -> Option<CardinalDirection> {
        match self {
            WantsToMove::Up => Some(CardinalDirection::Up),
            WantsToMove::Down => Some(CardinalDirection::Down),
            WantsToMove::Left => Some(CardinalDirection::Left),
            WantsToMove::Right => Some(CardinalDirection::Right),
            _ => None,
        }
    }
}

impl fmt::Display for WantsToMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            WantsToMove::Stationary => "STATIONARY",
            WantsToMove::Up => "UP",
            WantsToMove::Down => "DOWN",
            WantsToMove::Left => "LEFT",
            WantsToMove::Right => "RIGHT",
            WantsToMove::Action => "ACTION",
            WantsToMove::RandomDir => "RANDOMDIR",
        };
        write!(f, "{}", msg)
    }
}


#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
pub enum CardinalDirection {
    Up,
    Down,
    Left,
    Right
}

impl fmt::Display for CardinalDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            CardinalDirection::Up => "UP",
            CardinalDirection::Down => "DOWN",
            CardinalDirection::Left => "LEFT",
            CardinalDirection::Right => "RIGHT",
        };
        write!(f, "{}", msg)
    }
}



#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpriteAndWantsToMove {
    pub sprite_index: u16,
    pub wants_to_move: WantsToMove
}

impl SpriteAndWantsToMove {
    pub fn new(sprite_index: u16, wants_to_move: WantsToMove) -> Self {
        SpriteAndWantsToMove {
            sprite_index,
            wants_to_move
        }
    }
}


#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16
}

impl Position {
    pub fn new(x: u16, y: u16) -> Self {
        Position { x, y }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "@({},{})", self.x, self.y)
    }
}



#[derive(Clone, PartialEq, Debug)]
pub struct TriggeredCommands {
    pub message: Option<String>,
    pub again: bool,
    pub cancel: bool,
    pub checkpoint: bool,
    pub restart: bool,
    pub win: bool,
    pub sfx: bool,
}

impl TriggeredCommands {
    pub fn new() -> Self {
        Self {
            message: None,
            again: false,
            cancel: false,
            checkpoint: false,
            restart: false,
            win: false,
            sfx: false,
        }
    }
    pub fn did_trigger(&self) -> bool {
        self.again || self.cancel || self.checkpoint || self.restart || self.win || self.message.is_some() || self.sfx
    }
    pub fn merge(&mut self, other: &TriggeredCommands) {
        if self.message.is_none() { self.message = other.message.clone(); }
        self.again |= other.again;
        self.cancel |= other.cancel;
        self.checkpoint |= other.checkpoint;
        self.restart |= other.restart;
        self.win |= other.win;
        self.sfx |= other.sfx;
    }

    pub fn len(&self) -> u16 {
        let mut len = 0;
        if self.again { len += 1; }
        if self.cancel { len += 1; }
        if self.checkpoint { len += 1; }
        if self.restart { len += 1; }
        if self.win { len += 1; }
        if self.sfx { len += 1; }
        if self.message.is_some() { len += 1; }
        len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn again() -> Self {
        let mut t = TriggeredCommands::new();
        t.again = true;
        t
    }
    pub fn cancel() -> Self {
        let mut t = TriggeredCommands::new();
        t.cancel = true;
        t
    }
    pub fn checkpoint() -> Self {
        let mut t = TriggeredCommands::new();
        t.checkpoint = true;
        t
    }
    pub fn restart() -> Self {
        let mut t = TriggeredCommands::new();
        t.restart = true;
        t
    }
    pub fn win() -> Self {
        let mut t = TriggeredCommands::new();
        t.win = true;
        t
    }
    pub fn sfx() -> Self {
        let mut t = TriggeredCommands::new();
        t.sfx = true;
        t
    }
    pub fn message(message: String) -> Self {
        let mut t = TriggeredCommands::new();
        t.message = Some(message);
        t
    }
}

impl fmt::Display for TriggeredCommands {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.again { write!(f, " AGAIN")?; }
        if self.cancel { write!(f, " CANCEL")?; }
        if self.checkpoint { write!(f, " CHECKPOINT")?; }
        if self.restart { write!(f, " RESTART")?; }
        if self.win { write!(f, " WIN")?; }
        if self.sfx { write!(f, " SFX")?; }
        if let Some(message) = &self.message {
            write!(f, " MESSAGE {}", message)?;
        }
        Ok(())
    }
}
