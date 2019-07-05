use std::fmt;

pub mod board;
pub mod bracket;
pub mod cell;
pub mod game;
pub mod neighbor;
pub mod rule;
pub mod tile;
pub mod util;
use neighbor::Neighbor;
use rule::Command;
use tile::Tile;
use tile::TileKind;
use tile::TileWithModifier;

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut ary = String::from("");
        for sprite in &self.sprites {
            ary.push_str(&sprite.to_string());
            ary.push(' ');
        }
        match self.kind {
            TileKind::And => {
                if self.sprites.len() == 1 {
                    write!(f, "{}", ary)
                } else {
                    write!(f, "And({})", ary)
                }
            }
            TileKind::Or => write!(f, "Or({})", ary),
        }
    }
}

impl fmt::Display for TileWithModifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.negated {
            write!(f, "NO ")?;
        }
        if self.random {
            write!(f, "RANDOM ")?;
        }
        if let Some(w) = self.direction {
            write!(f, "{} ", w)?;
        }
        write!(f, "{}", self.tile)
    }
}

impl fmt::Display for Neighbor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut is_first = true;
        for t in &self.tiles_with_modifier {
            if !is_first {
                write!(f, " ")?
            }
            write!(f, "{}", t)?;
            is_first = false;
        }
        Ok(())
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            Command::Again => "AGAIN",
            Command::Cancel => "CANCEL",
            Command::Checkpoint => "CHECKPOINT",
            Command::Restart => "RESTART",
            Command::Win => "WIN",
            Command::Sfx => "SFX",
            Command::Message(_) => "MESSAGE",
        };
        write!(f, "{}", msg)
    }
}
