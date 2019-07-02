use log::{debug, trace, warn};

use std::fmt;
use fnv::FnvHashMap;
use fnv::FnvHashSet;
use rand::Rng;

use crate::bitset::BitSet;
use crate::model::tile::Tile;
use crate::model::util::Position;
use crate::model::board::Neighbors;
use crate::model::util::WantsToMove;
use crate::model::util::SpriteState;
use crate::model::util::CardinalDirection;
use crate::model::util::TriggeredCommands;
use crate::model::neighbor::Neighbor;
use crate::model::board::Board;
use crate::model::board::StripeCache;

#[derive(Clone, PartialEq, Debug)]
pub struct BracketMatch {
    pub before_positions: Neighbors,
    pub after_positions: Option<Neighbors>,
}

pub fn vec_of_optionals_to_vec<T>(v: Vec<Option<T>>) -> Vec<T> {
    v.into_iter().filter(|x| x.is_some()).map(|x| x.unwrap()).collect()
}

#[derive(Clone, Debug)]
pub struct Bracket {
    dir: CardinalDirection,
    pub before_neighbors: Vec<Neighbor>,
    pub after_neighbors: Vec<Neighbor>,
    all_sprites: BitSet,
    any_sprites: BitSet,
    sprite_movements_present: FnvHashSet<(u16, WantsToMove)>, // TODO: Decide if this check is useful. Speed is about the same
}

impl Bracket {
    pub fn new(dir: CardinalDirection, before_neighbors: Vec<Neighbor>) -> Self {
        let mut all_sprites = BitSet::new();
        let mut any_sprites = BitSet::new();
        let mut sprite_movements_present = FnvHashSet::default();
        for n in &before_neighbors {
            n.populate_cache(&mut all_sprites, &mut any_sprites, &mut sprite_movements_present);
        }
        Self {
            dir,
            before_neighbors,
            after_neighbors: vec![],
            all_sprites,
            any_sprites,
            sprite_movements_present,
        }
    }
    pub fn new_ellipsis(dir: CardinalDirection, before_neighbors: Vec<Neighbor>, after_neighbors: Vec<Neighbor>) -> Self {
        let mut all_sprites = BitSet::new();
        let mut any_sprites = BitSet::new();
        let mut sprite_movements_present = FnvHashSet::default();
        for n in &before_neighbors {
            n.populate_cache(&mut all_sprites, &mut any_sprites, &mut sprite_movements_present);
        }
        for n in &after_neighbors {
            n.populate_cache(&mut all_sprites, &mut any_sprites, &mut sprite_movements_present);
        }
        Self {
            dir,
            before_neighbors,
            after_neighbors,
            all_sprites,
            any_sprites,
            sprite_movements_present,
        }
    }
    pub fn prepare_actions(&mut self, action: &Bracket) -> bool {
        assert_eq!(self.before_neighbors.len(), action.before_neighbors.len());
        assert_eq!(self.after_neighbors.len(), action.after_neighbors.len());

        let mut has_actions = false;
        self.before_neighbors.iter_mut()
            .zip(&action.before_neighbors)
            .for_each(|(c, a)| has_actions |= c.prepare_actions(&a));
        self.after_neighbors.iter_mut()
            .zip(&action.after_neighbors)
            .for_each(|(c, a)| has_actions |= c.prepare_actions(&a));
        has_actions
    }

    pub fn matches(&self, board: &Board, m: BracketMatch) -> bool {
        assert!(self.before_neighbors.len() <= m.before_positions.len() as usize);
        // if !self.after_neighbors.is_empty() {
        //     assert!(self.after_neighbors.len() <= m.after_positions.unwrap().len() as usize);
        // }

        let matches = self.before_neighbors.iter()
            .zip(m.before_positions.iter())
            .map(|(n, pos)| {
                n.matches(board, &pos)
            })
            .all(|x| x);
        
        match m.after_positions {
            None => matches,
            Some(after_neighbors) => {
                if matches {
                    self.after_neighbors.iter()
                        .zip(after_neighbors.iter())
                        .map(|(n, pos)| {
                            n.matches(board, &pos)
                        })
                        .all(|x| x)

                } else {
                    false
                }
            }
        }
    }

    fn inner_find_match(&self, board: &Board, start_pos: &Position, self_neighbors: &Vec<Neighbor>) -> Option<Neighbors> {
        let neighbors = board.neighbor_positions(start_pos, self.dir);
        if self_neighbors.len() > neighbors.len() {
            None
        } else {
            // Check the row/col before iterating over every cell
            let cache = match self.dir {
                CardinalDirection::Up
                | CardinalDirection::Down => board.col_cache(start_pos.x),
                CardinalDirection::Left
                | CardinalDirection::Right => board.row_cache(start_pos.y),
            };
            if cache.sprites.contains_any(&self.any_sprites) && cache.sprites.contains_all(&self.all_sprites) && cache.contains_all_dirs(&self.sprite_movements_present) {
                if self.find_still_matched(board, self_neighbors, &neighbors) {
                    Some(neighbors)
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    pub fn find_match(&self, board: &Board, start_pos: &Position) -> Vec<BracketMatch> {
        // Simple case for the non-ellipsis bracket
        let before = self.inner_find_match(board, start_pos, &self.before_neighbors);
        if self.after_neighbors.is_empty() {
            match before {
                None => vec![],
                Some(before) => vec![BracketMatch { before_positions: before, after_positions: None }]
            }
        } else {
            // Ellipsis bracket case
            match before {
                None => vec![],
                Some(before) => {
                    let start_neighbor = before.nth(self.before_neighbors.len());

                    match start_neighbor {
                        None => vec![],
                        Some(start_neighbor) => {
                            board.neighbor_positions(&start_neighbor, self.dir).iter()
                                .map(|start| {
                                    let after = self.inner_find_match(board, &start, &self.after_neighbors);
                                    match after {
                                        None => None,
                                        Some(after) => {
                                            Some(BracketMatch { before_positions: before.clone(), after_positions: Some(after)})
                                        }
                                    }
                                })
                                .filter(|m| m.is_some())
                                .map(|m| m.unwrap())
                                .collect()
                        }
                    }
                }
            }
        }
    }

   fn find_still_matched(&self, board: &Board, self_neighbors: &Vec<Neighbor>, neighbors: &Neighbors) -> bool { // PERF_INSIDE: 26.5%
        self_neighbors.iter()
            .zip(neighbors.iter())
            .map(|(n, p)| {
                n.matches(board, &p)
            })
            .all(|x| x)
    }

    // pub fn find_all_still_matched(&self, board: &Board, all_neighbors_and_states: Vec<BracketMatch>) -> Vec<BracketMatch> { // PERF_INSIDE: 38.2%
    //     vec_of_optionals_to_vec(all_neighbors_and_states.iter().map(|neighbors_and_states| self.find_still_matched(board, neighbors_and_states.clone())).collect())
    // }

    pub fn evaluate<R: Rng + ?Sized>(&self, rng: &mut R, board: &mut Board, m: BracketMatch, magic_or_tiles: &FnvHashMap<Tile, Vec<SpriteState>>) -> bool {
        assert!(self.before_neighbors.len() <= m.before_positions.len());
        let mut something_changed = false;
        self.before_neighbors.iter()
            .zip(m.before_positions.iter())
            .for_each(|(n, pos)| if n.evaluate(rng, board, &pos, magic_or_tiles) { something_changed = true });
        
        if !self.after_neighbors.is_empty() {
            self.after_neighbors.iter()
                .zip(m.after_positions.unwrap().iter())
                .for_each(|(n, pos)| if n.evaluate(rng, board, &pos, magic_or_tiles) { something_changed = true });
        }
        something_changed
    }

    pub fn populate_magic_or_tiles(&self, board: &Board, magic_or_tiles: &mut FnvHashMap<Tile, Vec<SpriteState>>, m: BracketMatch) {
        assert!(self.before_neighbors.len() <= m.before_positions.len());
        // if !self.after_neighbors.is_empty() {
        //     assert!(self.after_neighbors.len() <= m.after_positions.unwrap().len());
        // }
        self.before_neighbors.iter()
            .zip(m.before_positions.iter())
            .for_each(|(n, pos)| n.populate_magic_or_tiles(magic_or_tiles, board, &pos));

        if !self.after_neighbors.is_empty() {
            self.after_neighbors.iter()
                .zip(m.after_positions.unwrap().iter())
                .for_each(|(n, pos)| n.populate_magic_or_tiles(magic_or_tiles, board, &pos));
        }

    }

    pub fn is_horizontal(&self) -> bool {
        match self.dir {
            CardinalDirection::Left
            | CardinalDirection::Right => true,
            CardinalDirection::Up
            | CardinalDirection::Down => false,
        }
    }

    pub fn matches_cache(&self, cache: &StripeCache) -> bool {
        // TODO: check sprite_movements_present in the row/col cache
        cache.sprites.contains_any(&self.any_sprites) && cache.sprites.contains_all(&self.all_sprites) && cache.contains_all_dirs(&self.sprite_movements_present)
    }
}

impl fmt::Display for Bracket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} [", self.dir)?;
        let mut is_first = true;
        for n in &self.before_neighbors {
            if !is_first { write!(f, "|")? }
            write!(f, "{}", n)?;
            is_first = false;
        }
        if !self.after_neighbors.is_empty() {
            write!(f, "| ...")?;
            for n in &self.after_neighbors {
                if !is_first { write!(f, "|")? }
                write!(f, "{}", n)?;
                is_first = false;
            }
        }
        write!(f, "]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::model::util::WantsToMove;
    use crate::model::neighbor::build_t;
    use crate::model::neighbor::tests::new_rng;
    use crate::model::neighbor::tests::check_counts;
    use crate::model::rule::Rule;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn bracket() {
        let player_sprite = SpriteState::new(&String::from("player"), 0, 0);
        let crate_sprite = SpriteState::new(&String::from("crate"), 1, 0); // same collision but different sprite index
        
        let origin = Position::new(0, 0);
        let center = Position::new(1, 1);
        let corner = Position::new(2, 2);

        let mut board = Board::new(3, 3);
        
        board.add_sprite(&origin, &player_sprite, WantsToMove::Stationary);
        board.add_sprite(&center, &player_sprite, WantsToMove::Stationary);
        board.add_sprite(&corner, &player_sprite, WantsToMove::Stationary);

        let bracket = Bracket::new(CardinalDirection::Down,
            vec![
                Neighbor::new(vec![build_t(false/*random*/, &player_sprite, false, None)]),
                Neighbor::new(vec![build_t(false/*random*/, &crate_sprite, true, None)]),
            ]
        );

        println!("Board: '{:?}'", board);
        assert!(bracket.matches(&board, BracketMatch { before_positions: Neighbors { size: board.size(), dir: CardinalDirection::Down, start: origin }, after_positions: None} ));
        assert!(bracket.matches(&board, BracketMatch { before_positions: Neighbors { size: board.size(), dir: CardinalDirection::Down, start: center }, after_positions: None} ));
    }

    #[test]
    fn find_still_matches() {
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false/*random*/, &player, false, None);
        let no_player = build_t(false/*random*/, &player, true, None);

        // RIGHT [ Player | NO Player] -> [ | Player ]
        let condition = Bracket::new(CardinalDirection::Right, vec![Neighbor::new(vec![player_any.clone()]), Neighbor::new(vec![no_player.clone()]) ]);
        let action = Bracket::new(CardinalDirection::Right, vec![Neighbor::new(vec![]), Neighbor::new(vec![player_any.clone()]) ]);

        let mut rule = Rule { causes_board_changes: None,
            conditions: vec![condition],
            actions:    vec![action],
            commands:   TriggeredCommands::new(),
            late: false,
            random: false,
            rigid: false,
        };
        rule.prepare_actions();

        check_counts(&rule.conditions[0].before_neighbors[0], 0, 0, 1);
        check_counts(&rule.conditions[0].before_neighbors[1], 0, 1, 0);
        
        let mut board = Board::new(3, 1);
        let origin = Position::new(0, 0);
        let middle = Position::new(1, 0);
        let end = Position::new(2, 0);

        board.add_sprite(&origin, &player, WantsToMove::Stationary);

        let c = &rule.conditions[0];
        let m = c.find_match(&board, &origin);
        assert_eq!(c.find_match(&board, &middle).len(), 0);

        assert!(c.find_still_matched(&board, &rule.conditions[0].before_neighbors, &m[0].before_positions));
        
        c.evaluate(&mut rng, &mut board, m[0].clone(), &FnvHashMap::default());

        assert!(!c.find_still_matched(&board, &rule.conditions[0].before_neighbors, &m[0].before_positions));

        assert!(!board.has_sprite(&origin, &player));
        assert!(board.has_sprite(&middle, &player));
        assert!(!board.has_sprite(&end, &player));

        assert_eq!(c.find_match(&board, &origin).len(), 0);
        assert_eq!(c.find_match(&board, &middle).len(), 1);
        assert_eq!(c.find_match(&board, &end).len(), 0);

        let m = c.find_match(&board, &middle);
        c.evaluate(&mut rng, &mut board, m[0].clone(), &FnvHashMap::default());

        assert_eq!(c.find_match(&board, &origin).len(), 0);
        assert_eq!(c.find_match(&board, &middle).len(), 0);
        assert_eq!(c.find_match(&board, &end).len(), 0);

        assert!(!board.has_sprite(&origin, &player));
        assert!(!board.has_sprite(&middle, &player));
        assert!(board.has_sprite(&end, &player));

    }

    #[test]
    fn empty_neighbor() {
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false/*random*/, &player, false, None);

        let whale = SpriteState::new(&String::from("whale"), 1, 0);
        let whale_any = build_t(false/*random*/, &whale, false, None);

        // RIGHT [ whale | ] -> [ whale | player ]
        let mut condition = Bracket::new(CardinalDirection::Right, vec![Neighbor::new(vec![whale_any.clone()]), Neighbor::new(vec![]) ]);
        let action = Bracket::new(CardinalDirection::Right, vec![Neighbor::new(vec![whale_any.clone()]), Neighbor::new(vec![player_any.clone()]) ]);

        condition.prepare_actions(&action);

        check_counts(&condition.before_neighbors[0], 0, 0, 0);
        check_counts(&condition.before_neighbors[1], 0, 1, 0);
        
        let mut board = Board::new(2, 1);
        let origin = Position::new(0, 0);
        let end = Position::new(1, 0);

        board.add_sprite(&origin, &whale, WantsToMove::Stationary);

        let m = condition.find_match(&board, &origin);
        assert_eq!(m.len(), 1);

        condition.evaluate(&mut rng, &mut board, m[0].clone(), &FnvHashMap::default());

        assert!(board.has_sprite(&origin, &whale));
        assert!(board.has_sprite(&end, &player));
    }

    #[test]
    fn empty_ellipsis_neighbor() {
        init();
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false/*random*/, &player, false, None);

        let whale = SpriteState::new(&String::from("whale"), 2, 2);
        let whale_any = build_t(false/*random*/, &whale, false, None);

        // RIGHT [ whale | ... | ] -> [ whale | ... | player ]
        let mut condition = Bracket::new_ellipsis(CardinalDirection::Right, vec![Neighbor::new(vec![whale_any.clone()])], vec![Neighbor::new(vec![]) ]);
        let action = Bracket::new_ellipsis(CardinalDirection::Right, vec![Neighbor::new(vec![whale_any.clone()])], vec![Neighbor::new(vec![player_any.clone()]) ]);

        condition.prepare_actions(&action);

        check_counts(&condition.before_neighbors[0], 0, 0, 0);
        check_counts(&condition.after_neighbors[0], 0, 1, 0);
        
        let mut board = Board::new(3, 1);
        let origin = Position::new(0, 0);
        let middle = Position::new(1, 0);
        let end = Position::new(2, 0);

        board.add_sprite(&origin, &whale, WantsToMove::Stationary);

        let m = condition.find_match(&board, &origin);
        assert_eq!(m.len(), 2);

        m.iter().for_each(|m| {
            if condition.matches(&mut board, m.clone()) {
                condition.evaluate(&mut rng, &mut board, m.clone(), &FnvHashMap::default());
            }
        });

        assert!(board.has_sprite(&origin, &whale));
        assert!(!board.has_sprite(&origin, &player));
        assert!(board.has_sprite(&middle, &player));
        assert!(board.has_sprite(&end, &player));
    }
    
    #[test]
    fn empty_ellipsis_neighbor_rule() {
        init();
        let mut rng = new_rng();

        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false/*random*/, &player, false, None);

        let whale = SpriteState::new(&String::from("whale"), 1, 0);
        let whale_any = build_t(false/*random*/, &whale, false, None);

        // RIGHT [ whale | ... | ] -> [ whale | ... | player ]
        let condition = Bracket::new_ellipsis(CardinalDirection::Right, vec![Neighbor::new(vec![whale_any.clone()])], vec![Neighbor::new(vec![]) ]);
        let action = Bracket::new_ellipsis(CardinalDirection::Right, vec![Neighbor::new(vec![whale_any.clone()])], vec![Neighbor::new(vec![player_any.clone()]) ]);

        let mut rule = Rule { causes_board_changes: None,
            conditions: vec![condition],
            actions: vec![action],
            commands: TriggeredCommands::new(),
            late: false,
            random: false,
            rigid: false,
        };
        rule.prepare_actions();

        let mut board = Board::new(3, 1);
        let origin = Position::new(0, 0);
        let middle = Position::new(1, 0);
        let end = Position::new(1, 0);

        board.add_sprite(&origin, &whale, WantsToMove::Stationary);

        rule.evaluate(&mut rng, &mut board, &mut TriggeredCommands::new(), false);

        assert!(!board.has_sprite(&origin, &player));
        assert!(board.has_sprite(&middle, &player));
        assert!(board.has_sprite(&end, &player));
    }
}