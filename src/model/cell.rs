use std::hash;
use std::fmt;
use std::cmp;

use fnv::FnvHashMap;

use crate::bitset::BitSet;
use crate::model::util::SpriteAndWantsToMove;
use crate::model::util::SpriteState;
use crate::model::util::WantsToMove;
use crate::model::util::Position;
use crate::model::tile::Tile;
use crate::model::tile::TileKind;

#[derive(Clone, PartialEq, Debug)]
pub struct Cell {
    // Needs to store sprites for each collision layer, and the wants_to_move for each collision_layer
    collision_layers: FnvHashMap<u16, SpriteAndWantsToMove>,

    pub sprite_bits: BitSet,

    // key: usize,
}

// type Cache = FnvHashMap<(usize /*Cell*/, usize/*Tile*/, Option<WantsToMove>), bool>;
// type CellKeys = FnvHashMap<String /*Cell*/, usize>;

impl Cell {
    pub fn new() -> Self {
        Self {
            collision_layers: FnvHashMap::default(),
            sprite_bits: BitSet::new(),
            // key: 0
        }
    }

    // The contract for others is that if they do not know the state of the cell they provide a 0
    // so we must never set the state to be 0
    fn invariant(&mut self) {
        // self.update_key();

        // Check that the sprite_bits matches the collision_layers
        // debug_assert_eq!(self.as_map().len(), self.sprite_bits.len() as usize, "Expected vs actual: {:?} {:?}", self.as_map(), self.sprite_bits);
        // for w in self.as_map().values() {
        //     assert!(self.sprite_bits.contains(w.sprite_index), "bitset did not contain a sprite that was in the collision layer map");
        // }

    }

    fn to_fingerprint(&self) -> String {
        // Update the key
        let mut fingerprint = String::from("");
        for (c, w) in &self.collision_layers {
            fingerprint.push_str(&c.to_string());
            fingerprint.push_str(":[");
            fingerprint.push_str(&w.sprite_index.to_string());
            fingerprint.push_str("=");
            fingerprint.push_str(&w.wants_to_move.to_string());
            fingerprint.push_str("] ");
        }
        fingerprint
    }

    /// Returns true if the cell was changed
    pub fn add_sprite(&mut self, sprite: &SpriteState, wants_to_move: WantsToMove) -> bool {
        self.add_sprite_index(sprite.collision_layer, sprite.index, wants_to_move)
    }
    pub fn add_sprite_index(&mut self, collision_layer: u16, sprite_index: u16, wants_to_move: WantsToMove) -> bool {
        if wants_to_move == WantsToMove::RandomDir {
            panic!("BUG: Should never try to set direction to RANDOMDIR at this point");
        }
        let w = SpriteAndWantsToMove::new(sprite_index, wants_to_move);
        match self.collision_layers.get(&collision_layer) {
            None => {
                self.sprite_bits.insert(sprite_index);
                self.collision_layers.insert(collision_layer, w);
                self.invariant();
                true
            },
            Some(w_curr) => {
                if *w_curr == w {
                    false
                } else {
                    self.sprite_bits.remove(w_curr.sprite_index);
                    self.sprite_bits.insert(w.sprite_index);
                    
                    self.collision_layers.insert(collision_layer, w);
                    self.invariant();
                    true
                }
            }
        }
    }

    pub fn remove_collision_layer(&mut self, collision_layer: u16) -> bool {
        match self.collision_layers.remove(&collision_layer) {
            Some(w) => {
                self.sprite_bits.remove(w.sprite_index);
                self.invariant();
                true
            },
            None => false,
        }
    }

    pub fn get_collision_layer(&self, collision_layer: u16) -> Option<&SpriteAndWantsToMove> {
        self.collision_layers.get(&collision_layer)
    }

    pub fn as_map(&self) -> &FnvHashMap<u16, SpriteAndWantsToMove> {
        &self.collision_layers
    }
    pub fn get_wants_to_move(&self, collision_layer: u16) -> Option<WantsToMove> {
        let w = self.collision_layers.get(&collision_layer);
        match w {
            None => None,
            Some(w2) => Some(w2.wants_to_move)
        }
    }
    pub fn set_wants_to_move(&mut self, collision_layer: u16, dir: WantsToMove) -> bool {
        let x = self.collision_layers.get(&collision_layer).expect("Bug: Just setting a new direction. Assumed there was already a sprite which should have been the case");
        if x.wants_to_move == dir {
            false
        } else {
            self.sprite_bits.insert(x.sprite_index);
            self.collision_layers.insert(collision_layer, SpriteAndWantsToMove::new(x.sprite_index, dir));
            self.invariant();
            true
        }
    }
    pub fn has_sprite(&self, sprite: &SpriteState) -> bool {
        match self.collision_layers.get(&sprite.collision_layer) {
            None => false,
            Some(s) => s.sprite_index == sprite.index,
        }
    }
    pub fn has_collision_layer(&self, collision_layer: u16) -> bool {
        self.collision_layers.contains_key(&collision_layer)
    }

    pub fn matches(&self, tile: &Tile, dir: &Option<WantsToMove>) -> bool {
        self.matches_no_cache(&tile, &dir)
        // match self.get_cache_match(&tile, &dir) {
        //     None => {
        //         let b = self.matches_no_cache(&tile, &dir);
        //         self.add_cache_match(&tile, dir, b);
        //         b
        //     },
        //     Some(b) => b,
        // }
    }

    fn matches_no_cache(&self, tile: &Tile, dir: &Option<WantsToMove>) -> bool {
        match &tile.kind {
            TileKind::And => {
                self.matches_all(&tile, &dir) // PERF: 5.6%
            },
            TileKind::Or => {
                self.matches_any(&tile, &dir) // PERF: 4.7%
            }
        }
    }

    fn matches_any(&self, tile: &Tile, dir: &Option<WantsToMove>) -> bool {
        if self.sprite_bits.contains_any(&tile.bits) {
            for collision_layer in &tile.collision_layers {
                let cell_sprite = self.collision_layers.get(&collision_layer);
                if let Some(s) = cell_sprite {
                    if tile.bits.contains(s.sprite_index) {
                        match dir {
                            None => return true,
                            Some(d) => if s.wants_to_move == *d {
                                return true
                            }
                        }
                    }
                }
            }
            false
        } else {
            false
        }
    }

    fn matches_all(&self, tile: &Tile, dir: &Option<WantsToMove>) -> bool {
        if self.sprite_bits.contains_all(&tile.bits) {
            for collision_layer in &tile.collision_layers {
                let cell_sprite_maybe = self.collision_layers.get(&collision_layer);
                if let Some(cell_sprite) = cell_sprite_maybe {
                    // sprites MUST be in different collision layers so there can be only one
                    if tile.bits.contains(cell_sprite.sprite_index) {
                        if let Some(d) = dir {
                            if cell_sprite.wants_to_move != *d {
                                return false
                            }
                        }
                    } else {
                        return false
                    }
                } else {
                    return false
                }
            }
            true

        } else {
            false
        }
    }

    // fn update_key(&mut self) {
    //     let fingerprint = self.to_fingerprint();
    //     let key = CELL_KEYS.with(|c| {
    //         let mut key_lookup = c.borrow_mut();
    //         let new_key = key_lookup.len();
    //         key_lookup.entry(fingerprint).or_insert(new_key).clone()
    //     });
    //     self.key = key;
    // }

    // fn add_cache_match(&self, tile: &Tile, dir: &Option<WantsToMove>, did_match: bool) {
    //     let key = (self.key, tile.id, dir.clone());
    //     CACHE.with(|c| {
    //         let mut cache = c.borrow_mut();
    //         cache.entry(key).or_insert(did_match);
    //     })
    // }
    // fn get_cache_match(&self, tile: &Tile, dir: &Option<WantsToMove>) -> Option<bool> {
    //     let key = (self.key, tile.id, dir.clone());
    //     CACHE.with(|c| {
    //         let cache = c.borrow();
    //         match cache.get(&key) {
    //             None => None,
    //             Some(&b) => Some(b),
    //         }
    //     })
    // }
}

// Used for fingerprinting whether a Tile and direction matches a cell
impl hash::Hash for Cell {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        for (c, w) in &self.collision_layers {
            c.hash(state);
            w.sprite_index.hash(state);
            w.wants_to_move.hash(state);
        }
    }
}

impl cmp::Eq for Cell {}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sprites: Vec<_> = self.as_map().values().map(|s| s.sprite_index).collect();
        write!(f, "Cell({:?})", sprites)
    }
}


// Memoize the cell & tile matching to reduce computation
// thread_local!(static CACHE: cell::RefCell<Cache> = cell::RefCell::new(FnvHashMap::default()));
// thread_local!(static CELL_KEYS: cell::RefCell<CellKeys> = cell::RefCell::new(FnvHashMap::default()));