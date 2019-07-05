use std::cmp;
use std::hash;

use fnv::FnvHashSet;

use crate::bitset::BitSet;
use crate::model::board::Board;
use crate::model::util::Position;
use crate::model::util::SpriteState;
use crate::model::util::WantsToMove;

// static mut COUNTER: usize = 0;

#[derive(Clone, PartialEq, Debug)]
pub enum TileKind {
    And,
    Or,
}

#[derive(Clone, Debug)]
pub struct Tile {
    // pub id: usize,
    pub kind: TileKind,
    pub name: String,
    pub bits: BitSet,
    pub collision_layers: FnvHashSet<u16>,
    pub sprites: Vec<SpriteState>,
}

impl Tile {
    pub fn new(kind: TileKind, name: &String, sprites: Vec<SpriteState>) -> Self {
        // let id;
        // unsafe {
        //     COUNTER+=1;
        //     id = COUNTER;
        // }
        let mut bits = BitSet::new();
        sprites.iter().for_each(|s| bits.insert(s.index));

        Self {
            // id,
            kind,
            name: name.clone(),
            bits,
            collision_layers: sprites.iter().map(|s| s.collision_layer).collect(),
            sprites,
        }
    }
    pub fn is_or(&self) -> bool {
        self.kind == TileKind::Or
    }
    pub fn has_single_collision_layer(&self) -> bool {
        self.collision_layers.len() == 1
    }

    pub fn get_sprites(&self) -> &Vec<SpriteState> {
        &self.sprites
    }

    pub fn get_collision_layers(&self) -> &FnvHashSet<u16> {
        &self.collision_layers
    }
}

impl hash::Hash for Tile {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // self.sprites.iter().for_each(|s| s.to_string().hash(state));
        self.name.hash(state)
    }
}

impl cmp::PartialEq for Tile {
    fn eq(&self, other: &Tile) -> bool {
        self.kind == other.kind && self.bits == other.bits
    }
}

impl cmp::Eq for Tile {}

#[derive(Clone, PartialEq, Debug)]
pub struct TileWithModifier {
    pub random: bool,
    pub negated: bool,
    pub tile: Tile,
    pub direction: Option<WantsToMove>,
}

impl TileWithModifier {
    pub fn matches(&self, board: &Board, pos: &Position) -> bool {
        let t = board.matches(&pos, &self.tile, &self.direction);
        self.negated ^ t
    }
}
