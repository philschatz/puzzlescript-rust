use log::{debug, trace};
extern crate rand_core;
extern crate rand_xorshift;

use fnv::FnvHashMap;
use std::fmt;
use std::time;

use rand::Rng;

use crate::color::Rgb;
use crate::model::board::Board;
use crate::model::bracket::Bracket;
use crate::model::neighbor::Neighbor;
use crate::model::rule::Rule;
use crate::model::rule::RuleGroup;
use crate::model::rule::RuleLoop;
use crate::model::tile::Tile;
use crate::model::tile::TileWithModifier;
use crate::model::util::CardinalDirection;
use crate::model::util::Dimension;
use crate::model::util::Position;
use crate::model::util::SpriteState;
use crate::model::util::TriggeredCommands;
use crate::model::util::WantsToMove;

use crate::debugger::ScreenDumper;

#[derive(Clone, Debug)]
pub enum Level {
    Message(String),
    Map(Vec<Vec<Tile>>),
}

impl Level {
    pub fn size(&self) -> (u16, u16) {
        match self {
            Level::Message(_) => panic!("Trying to get a Grid out of a Message level"),
            Level::Map(m) => (m[0].len() as u16, m.len() as u16),
        }
    }
}

#[derive(Debug)]
pub enum WinConditionOnQualifier {
    All,
    No,
    Some,
    Any,
}

#[derive(Debug)]
pub enum WinCondition {
    On(WinConditionOnQualifier, Tile, Tile),
    Simple(WinConditionOnQualifier, Tile),
}

impl WinCondition {
    fn update_acc(&self, board: &Board, pos: &Position, acc: (u16, u16)) -> (u16, u16) {
        match self {
            WinCondition::Simple(_, tile) => {
                if board.matches(pos, tile, &None) {
                    (acc.0 + 1, acc.1)
                } else {
                    (acc.0, acc.1)
                }
            }
            WinCondition::On(_, tile, on_tile) => {
                if board.matches(pos, tile, &None) {
                    if board.matches(pos, on_tile, &None) {
                        (acc.0 + 1, acc.1 + 1)
                    } else {
                        (acc.0 + 1, acc.1)
                    }
                } else {
                    (acc.0, acc.1)
                }
            }
        }
    }
    fn satisfies_acc(&self, acc: (u16, u16)) -> bool {
        let ret = match self {
            WinCondition::Simple(WinConditionOnQualifier::No, _) => acc.0 == 0,
            WinCondition::Simple(WinConditionOnQualifier::Any, _) => acc.0 > 0,
            WinCondition::Simple(WinConditionOnQualifier::Some, _) => acc.0 > 0,
            WinCondition::Simple(WinConditionOnQualifier::All, _) => {
                unreachable!("A simple WinCondition does not have an ALL qualifier")
            }
            WinCondition::On(WinConditionOnQualifier::No, _, _) => acc.1 == 0,
            WinCondition::On(WinConditionOnQualifier::Any, _, _) => acc.1 > 0,
            WinCondition::On(WinConditionOnQualifier::Some, _, _) => acc.1 > 0,
            WinCondition::On(WinConditionOnQualifier::All, _, _) => acc.0 == acc.1,
        };
        if ret {
            debug!("Win condition satistied: {} acc={:?}", self, acc);
        }
        ret
    }
}

impl fmt::Display for WinCondition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WinCondition::Simple(q, tile) => write!(f, "{:?} {}", q, tile),
            WinCondition::On(q, tile, on_tile) => write!(f, "{:?} {} ON {}", q, tile, on_tile),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Sprite {
    pub id: usize,
    pub name: String,
    pub pixels: Vec<Vec<Option<Rgb>>>,
}

impl Sprite {
    pub fn contains_alpha_pixel(&self) -> bool {
        self.pixels.iter().flat_map(|x| x).any(|color| match color {
            None => false,
            Some(c) => c.a != 0,
        })
    }
}

#[derive(Debug)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Action,
}

fn build_input_rule(player_tile: &Tile, wants_to_move: WantsToMove) -> RuleLoop {
    let mut ret = RuleLoop {
        is_loop: false,
        rules: vec![RuleGroup {
            random: false,
            rules: vec![Rule {
                causes_board_changes: None,
                late: false,
                random: false,
                rigid: false,
                commands: TriggeredCommands::default(),
                conditions: vec![Bracket::new(
                    CardinalDirection::Right, // does not matter since just one neighbor
                    vec![Neighbor::new(vec![TileWithModifier {
                        random: false,
                        negated: false,
                        tile: player_tile.clone(),
                        direction: Some(WantsToMove::Stationary),
                    }])],
                )],
                actions: vec![Bracket::new(
                    CardinalDirection::Right, // does not matter since just one neighbor
                    vec![Neighbor::new(vec![TileWithModifier {
                        random: false,
                        negated: false,
                        tile: player_tile.clone(),
                        direction: Some(wants_to_move),
                    }])],
                )],
            }],
        }],
    };

    ret.prepare_actions();
    ret
}

#[derive(Default, Debug)]
pub struct Metadata {
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub youtube: Option<String>,
    pub zoomscreen: Option<Dimension>,
    pub flickscreen: Option<Dimension>,
    pub color_palette: Option<String>,
    pub background_color: Option<Rgb>,
    pub text_color: Option<Rgb>,
    // in seconds
    pub realtime_interval: Option<f32>,
    pub key_repeat_interval: Option<f32>,
    pub again_interval: Option<f32>,
    pub no_action: bool,
    pub no_undo: bool,
    pub run_rules_on_level_start: Option<bool>,
    pub no_repeat_action: bool,
    pub throttle_movement: bool,
    pub no_restart: bool,
    pub require_player_movement: bool,
    pub verbose_logging: bool,
}

#[derive(Debug)]
pub struct GameData {
    pub title: String,
    pub metadata: Metadata,
    _sprite_size: (u16, u16),
    pub sprites: FnvHashMap<SpriteState, Sprite>,
    pub player_tile: Tile,
    pub background_tile: Tile,
    pub rules: Vec<RuleLoop>,
    pub levels: Vec<Level>,
    pub win_conditions: Vec<WinCondition>,
    pub input_rule_up: RuleLoop,
    pub input_rule_down: RuleLoop,
    pub input_rule_left: RuleLoop,
    pub input_rule_right: RuleLoop,
    pub input_rule_action: RuleLoop,
}

impl GameData {
    pub fn new(
        title: String,
        metadata: Metadata,
        sprites: FnvHashMap<SpriteState, Sprite>,
        player_tile: Tile,
        background_tile: Tile,
        rules: Vec<RuleLoop>,
        levels: Vec<Level>,
        win_conditions: Vec<WinCondition>,
    ) -> Self {
        let sprite_size = match sprites.iter().next() {
            None => (5, 5),
            Some((_, sprite)) => (sprite.pixels[0].len() as u16, sprite.pixels.len() as u16),
        };

        Self {
            input_rule_up: build_input_rule(&player_tile, WantsToMove::Up),
            input_rule_down: build_input_rule(&player_tile, WantsToMove::Down),
            input_rule_left: build_input_rule(&player_tile, WantsToMove::Left),
            input_rule_right: build_input_rule(&player_tile, WantsToMove::Right),
            input_rule_action: build_input_rule(&player_tile, WantsToMove::Action),

            title,
            metadata,
            _sprite_size: sprite_size,
            sprites,
            background_tile,
            player_tile,
            rules,
            levels,
            win_conditions,
        }
    }

    pub fn sprite_size(&self) -> (u16, u16) {
        self._sprite_size
    }

    fn evaluate_rules<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        board: &mut Board,
        late: bool,
    ) -> TriggeredCommands {
        let start_time = time::Instant::now();
        let mut t = TriggeredCommands::default();
        self.rules
            .iter()
            .map(|r| r.evaluate(rng, board, late))
            .for_each(|c| t.merge(&c));
        trace!("Rule Evaluation took {}sec", start_time.elapsed().as_secs());
        t
    }

    fn evaluate_post(&self, board: &mut Board, triggered: &TriggeredCommands) {
        // Move all the sprites in cells that want to move
        let mut did_change;
        loop {
            did_change = false;

            let mut to_stationary = vec![];
            let mut to_move = vec![];

            // Determine which cells to clear to Stationary and which sprites to move.
            for pos in board.positions_iter() {
                for (c, sw) in board.as_map(&pos) {
                    match sw.wants_to_move.to_cardinal_direction() {
                        None => {
                            if sw.wants_to_move != WantsToMove::Stationary {
                                to_stationary.push((pos, c.clone()))
                            }
                        }
                        Some(dir) => match board.neighbor_position(&pos, dir) {
                            None => to_stationary.push((pos, c.clone())),
                            Some(neighbor_pos) => {
                                if !board.has_collision_layer(&neighbor_pos, *c) {
                                    to_move.push((pos, c.clone(), neighbor_pos, sw.sprite_index));
                                }
                            }
                        },
                    }
                }
            }

            // Now, perform the changes since we are no longer borrowing elements
            for (pos, c) in to_stationary {
                trace!("POST: Marking sprite {} as stationary @ {}", c, pos);
                did_change |= board.set_wants_to_move(&pos, c, WantsToMove::Stationary);
            }
            for (pos, c, neighbor_pos, sprite_index) in to_move {
                if !board.has_collision_layer(&neighbor_pos, c) {
                    debug!(
                        "POST: Moving sprite {} from {} to {}",
                        sprite_index, pos, neighbor_pos
                    );
                    did_change |= board.remove_collision_layer(&pos, c);
                    did_change |= board.add_sprite_index(
                        &neighbor_pos,
                        c,
                        sprite_index,
                        WantsToMove::Stationary,
                    )
                } else {
                    debug!("POST: Tried to move sprite {} from {} to {} but something became in-the-way", sprite_index, pos, neighbor_pos);
                }
            }

            if ScreenDumper::is_enabled() {
                ScreenDumper::dump(
                    board,
                    triggered,
                    &String::from("post-action WantsToMoves. iteration done"),
                );
            }

            if !did_change {
                break;
            }
        }

        // Finally, clear all the WantsToMove because the elements were not able to move (they were blocked)
        let mut to_stationary = vec![];
        for pos in board.positions_iter() {
            for (c, sw) in board.as_map(&pos) {
                if sw.wants_to_move != WantsToMove::Stationary {
                    to_stationary.push((pos, c.clone()));
                }
            }
        }
        for (pos, c) in to_stationary {
            trace!(
                "POST: Finally, Marking sprite {} as stationary @ {}",
                c,
                pos
            );
            board.set_wants_to_move(&pos, c, WantsToMove::Stationary);
        }
    }

    fn check_win_conditions(&self, board: &Board) -> bool {
        if self.win_conditions.len() == 0 {
            return false;
        }
        let initial: Vec<_> = self.win_conditions.iter().map(|_| (0, 0)).collect();

        let accumulated = board.positions_iter().iter().fold(initial, |accs, pos| {
            self.win_conditions
                .iter()
                .zip(accs)
                .map(|(w, acc)| w.update_acc(board, pos, acc))
                .collect()
        });

        // Check if any win condition is satisfied
        self.win_conditions
            .iter()
            .zip(accumulated)
            .all(|(w, acc)| w.satisfies_acc(acc))
    }

    pub fn evaluate<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        board: &mut Board,
        debug_rules: bool,
    ) -> TriggeredCommands {
        // enable/disable screen dumping for each rule
        let has_sprites = ScreenDumper::is_enabled();
        if debug_rules && !has_sprites {
            ScreenDumper::set_sprites(Some(self.sprites.clone()));
        } else if !debug_rules && has_sprites {
            ScreenDumper::set_sprites(None);
        }

        let mut t = self.evaluate_rules(rng, board, false);
        // Short-circuit if we already cancelled
        if t.cancel {
            trace!("CANCEL command found while evaluating the non-late rules");
            return t;
        }
        self.evaluate_post(board, &t);
        if ScreenDumper::is_enabled() {
            ScreenDumper::dump(board, &t, &String::from("Resolved remaining wantstomoves"));
        }

        if ScreenDumper::is_enabled() {
            println!("Evaluating LATE rules...");
        }
        t.merge(&self.evaluate_rules(rng, board, true));
        if ScreenDumper::is_enabled() {
            println!("Evaluated LATE rules");
        }
        t.win |= self.check_win_conditions(board);
        t
    }

    pub fn evaluate_player_input<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        board: &mut Board,
        input: Input,
    ) {
        let input_rule = match input {
            Input::Up => &self.input_rule_up,
            Input::Down => &self.input_rule_down,
            Input::Left => &self.input_rule_left,
            Input::Right => &self.input_rule_right,
            Input::Action => &self.input_rule_action,
        };
        input_rule.evaluate(rng, board, false);
    }

    pub fn to_board(&self, level: &Level) -> Board {
        match level {
            Level::Map(grid) => Board::from_tiles(grid, &self.background_tile),
            Level::Message(_) => panic!("Should have found a Map to play"),
        }
    }

    pub fn lookup_sprite(&self, sprite: SpriteState) -> &Sprite {
        self.sprites.get(&sprite).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::model::neighbor::build_t;
    use crate::model::neighbor::tests::new_rng;
    use crate::model::tile::TileKind;
    use crate::model::util::Position;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn did_trigger(t: &TriggeredCommands) -> bool {
        t.again || t.cancel || t.checkpoint || t.restart || t.win || t.message.is_some() || t.sfx
    }

    #[test]
    fn game_player_didnt_move_because_of_level_boundary() {
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false, &player, false, None);
        let player_right = build_t(false, &player, false, Some(WantsToMove::Right));
        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![Rule {
                    causes_board_changes: None,
                    conditions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone()])],
                    )],
                    actions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_right.clone()])],
                    )],
                    commands: TriggeredCommands::default(),
                    late: false,
                    random: false,
                    rigid: false,
                }],
            }],
        };
        rule.prepare_actions();

        // 1x1 board
        let origin = Position::new(0, 0);
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![Tile::new(
            TileKind::And,
            &String::from("t1"),
            vec![player],
        )]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            player_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![],
        );
        let mut board = game.to_board(&level);

        game.evaluate(&mut rng, &mut board, false);

        assert!(board.has_sprite(&origin, &player));
        assert_eq!(
            board.get_wants_to_move(&origin, player.collision_layer),
            Some(WantsToMove::Stationary)
        )
    }

    #[test]
    fn game_player_didnt_move_because_of_collision() {
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let crate_sprite = SpriteState::new(&String::from("Crate"), 1, 0);
        let player_any = build_t(false, &player, false, None);
        let player_right = build_t(false, &player, false, Some(WantsToMove::Right));
        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![Rule {
                    causes_board_changes: None,
                    conditions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone()])],
                    )],
                    actions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_right.clone()])],
                    )],
                    commands: TriggeredCommands::default(),
                    late: false,
                    random: false,
                    rigid: false,
                }],
            }],
        };
        rule.prepare_actions();

        // 2x1 board
        let origin = Position::new(0, 0);
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![
            Tile::new(TileKind::And, &String::from("t1"), vec![player]),
            Tile::new(TileKind::And, &String::from("t2"), vec![crate_sprite]),
        ]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            player_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![],
        );
        let mut board = game.to_board(&level);

        game.evaluate(&mut rng, &mut board, false);

        assert!(board.has_sprite(&origin, &player));
        assert_eq!(
            board.get_wants_to_move(&origin, player.collision_layer),
            Some(WantsToMove::Stationary)
        )
    }

    #[test]
    fn game_player_move_and_sent_command() {
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let player_any = build_t(false, &player, false, None);
        let player_right = build_t(false, &player, false, Some(WantsToMove::Right));
        let background_any = build_t(false, &background, false, None);

        // [ player ] -> [ RIGHT player ] AGAIN RESTART CHECKPOINT WIN SFX MESSAGE YouWin!
        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![Rule {
                    causes_board_changes: None,
                    conditions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone()])],
                    )],
                    actions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_right.clone()])],
                    )],
                    commands: TriggeredCommands {
                        again: true,
                        cancel: false,
                        restart: true,
                        checkpoint: true,
                        win: true,
                        message: Some(String::from("YouWin!")),
                        sfx: true,
                    },
                    late: false,
                    random: false,
                    rigid: false,
                }],
            }],
        };
        rule.prepare_actions();

        // 2x1 board
        let origin = Position::new(0, 0);
        let dest = Position::new(1, 0);
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![
            Tile::new(TileKind::And, &String::from("t1"), vec![player]),
            Tile::new(TileKind::And, &String::from("t2"), vec![background]),
        ]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![],
        );
        let mut board = game.to_board(&level);

        assert!(board.has_sprite(&origin, &player));

        let t = game.evaluate(&mut rng, &mut board, false);

        assert!(!board.has_sprite(&origin, &player));

        assert!(board.has_sprite(&dest, &player));
        assert_eq!(
            board.get_wants_to_move(&dest, player.collision_layer),
            Some(WantsToMove::Stationary)
        );

        assert_eq!(did_trigger(&t), true);
        assert_eq!(t.again, true);
        assert_eq!(t.cancel, false);
        assert_eq!(t.checkpoint, true);
        assert_eq!(t.restart, true);
        assert_eq!(t.win, true);
        assert_eq!(t.sfx, true);
        assert_eq!(t.message, Some(String::from("YouWin!")));
    }

    #[test]
    fn game_cancelled_move() {
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let cat = SpriteState::new(&String::from("cat"), 2, 0);
        let player_any = build_t(false, &player, false, None);
        let background_any = build_t(false, &background, false, None);
        let cat_any = build_t(false, &cat, false, None);

        // [ player ] -> [ cat ] CANCEL
        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![Rule {
                    causes_board_changes: None,
                    conditions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone()])],
                    )],
                    actions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![cat_any.clone()])],
                    )],
                    commands: TriggeredCommands {
                        again: false,
                        cancel: true,
                        restart: false,
                        checkpoint: false,
                        win: false,
                        message: None,
                        sfx: false,
                    },
                    late: false,
                    random: false,
                    rigid: false,
                }],
            }],
        };
        rule.prepare_actions();

        // 2x1 board
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![
            Tile::new(TileKind::And, &String::from("t1"), vec![player]),
            Tile::new(TileKind::And, &String::from("t2"), vec![background]),
        ]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![],
        );
        let mut board = game.to_board(&level);

        let t = game.evaluate(&mut rng, &mut board, false);

        assert_eq!(did_trigger(&t), true);
        assert_eq!(t.cancel, true);
    }

    #[test]
    fn game_player_did_move_eventually() {
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let player_any = build_t(false, &player, false, None);
        let player_right = build_t(false, &player, false, Some(WantsToMove::Right));
        let background_any = build_t(false, &background, false, None);
        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![Rule {
                    causes_board_changes: None,
                    conditions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone()])],
                    )],
                    actions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_right.clone()])],
                    )],
                    commands: TriggeredCommands::default(),
                    late: false,
                    random: false,
                    rigid: false,
                }],
            }],
        };
        rule.prepare_actions();

        // 3x1 board
        let origin = Position::new(0, 0);
        let middle = Position::new(1, 0);
        let right = Position::new(2, 0);
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![
            Tile::new(TileKind::And, &String::from("t1"), vec![player]),
            Tile::new(TileKind::And, &String::from("t2"), vec![player]),
            Tile::new(TileKind::And, &String::from("t1"), vec![background]),
        ]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![],
        );
        let mut board = game.to_board(&level);

        game.evaluate_rules(&mut rng, &mut board, false);

        // Verify that the rules marked all the players as wanting to move RIGHT
        assert_eq!(
            board.get_wants_to_move(&origin, player.collision_layer),
            Some(WantsToMove::Right)
        );
        assert_eq!(
            board.get_wants_to_move(&middle, player.collision_layer),
            Some(WantsToMove::Right)
        );
        assert_eq!(
            board.get_wants_to_move(&right, player.collision_layer),
            None
        );

        game.evaluate_post(&mut board, &TriggeredCommands::default());

        assert!(!board.has_sprite(&origin, &player));
        assert!(board.has_sprite(&middle, &player));
        assert!(board.has_sprite(&right, &player));
    }

    #[test]
    fn post_resolution_does_not_lose_sprites() {
        init();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let rock = SpriteState::new(&String::from("rock"), 2, 0);
        let player_any = build_t(false, &player, false, None);
        let background_any = build_t(false, &background, false, None);

        // 2x2 board
        let top = Position::new(1, 0);
        let left = Position::new(0, 1);
        let end = Position::new(1, 1);

        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![],
            vec![],
            vec![],
        );
        let mut board = Board::new(2, 2);

        assert!(board.add_sprite(&top, &rock, WantsToMove::Down));
        assert!(board.add_sprite(&left, &player, WantsToMove::Right));

        game.evaluate_post(&mut board, &TriggeredCommands::default());

        assert!(board.has_sprite(&end, &rock));
        assert!(!board.has_sprite(&end, &player));
        assert!(board.has_sprite(&left, &player));
    }

    #[test]
    fn evals_late_rules_after_moving() {
        init();
        // Expected: move the player right AND THEN replace it with a star (not before moving)
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let star = SpriteState::new(&String::from("Star"), 2, 2);
        let player_any = build_t(false, &player, false, None);
        let player_right = build_t(false, &player, false, Some(WantsToMove::Right));
        let background_any = build_t(false, &background, false, None);
        let star_any = build_t(false, &star, false, None);

        // [ Player ] -> [ RIGHT Player ]
        // LATE [ Player ] -> [ Star ]
        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![
                    Rule {
                        causes_board_changes: None,
                        conditions: vec![Bracket::new(
                            CardinalDirection::Right,
                            vec![Neighbor::new(vec![player_any.clone()])],
                        )],
                        actions: vec![Bracket::new(
                            CardinalDirection::Right,
                            vec![Neighbor::new(vec![player_right.clone()])],
                        )],
                        commands: TriggeredCommands::default(),
                        late: false,
                        random: false,
                        rigid: false,
                    },
                    Rule {
                        causes_board_changes: None,
                        conditions: vec![Bracket::new(
                            CardinalDirection::Right,
                            vec![Neighbor::new(vec![player_any.clone()])],
                        )],
                        actions: vec![Bracket::new(
                            CardinalDirection::Right,
                            vec![Neighbor::new(vec![star_any.clone()])],
                        )],
                        commands: TriggeredCommands::default(),
                        late: true,
                        random: false,
                        rigid: false,
                    },
                ],
            }],
        };
        rule.prepare_actions();

        // 2x1 board
        let origin = Position::new(0, 0);
        let right = Position::new(1, 0);
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![
            Tile::new(TileKind::And, &String::from("t1"), vec![player]),
            Tile::new(TileKind::And, &String::from("t1"), vec![background]),
        ]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![],
        );
        let mut board = game.to_board(&level);

        game.evaluate(&mut rng, &mut board, false);

        // Verify that the player moved right AND THEN switched to be a star
        assert!(!board.has_sprite(&origin, &player));
        assert!(!board.has_sprite(&origin, &star));
        assert!(board.has_sprite(&right, &star));
    }

    #[test]
    fn winconditions_are_anded() {
        init();
        // Expected: move the player right AND THEN replace it with a star (not before moving)
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let star = SpriteState::new(&String::from("Star"), 2, 2);
        let player_any = build_t(false, &player, false, None);
        let background_any = build_t(false, &background, false, None);
        let star_any = build_t(false, &star, false, None);

        let win1 = WinCondition::Simple(WinConditionOnQualifier::No, player_any.tile.clone());
        let win2 = WinCondition::Simple(WinConditionOnQualifier::No, star_any.tile.clone());

        // 1x1 board
        let origin = Position::new(0, 0);
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![Tile::new(
            TileKind::And,
            &String::from("t1"),
            vec![player],
        )]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![],
            vec![level.clone()],
            vec![win1, win2],
        );
        let mut board = game.to_board(&level);

        // verify that _both_ win conditions must be satisfied
        let t = game.evaluate(&mut rng, &mut board, false);
        assert_eq!(t.win, false);

        board.remove_collision_layer(&origin, player.collision_layer);

        // verify that _both_ win conditions ARE satisfied
        let t = game.evaluate(&mut rng, &mut board, false);
        assert_eq!(t.win, true);
    }

    #[test]
    fn winconditions_on_works() {
        init();
        // Expected: move the player right AND THEN replace it with a star (not before moving)
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let star = SpriteState::new(&String::from("Star"), 2, 2);
        let player_any = build_t(false, &player, false, None);
        let background_any = build_t(false, &background, false, None);
        let star_any = build_t(false, &star, false, None);

        let win1 = WinCondition::On(
            WinConditionOnQualifier::Some,
            player_any.tile.clone(),
            star_any.tile.clone(),
        );

        // 1x1 board
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![Tile::new(
            TileKind::And,
            &String::from("t1"),
            vec![player, star],
        )]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![],
            vec![level.clone()],
            vec![win1],
        );
        let mut board = game.to_board(&level);

        // verify that the ON condition is be satisfied
        let t = game.evaluate(&mut rng, &mut board, false);
        assert_eq!(t.win, true);
    }

    #[test]
    fn winconditions_on_works_when_late() {
        init();
        // Expected: move the player right AND THEN replace it with a star (not before moving)
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let background = SpriteState::new(&String::from("background"), 1, 1); // needs to be diff collision layer so player moves
        let star = SpriteState::new(&String::from("Star"), 2, 2);
        let player_any = build_t(false, &player, false, None);
        let background_any = build_t(false, &background, false, None);
        let star_any = build_t(false, &star, false, None);

        let mut rule = RuleLoop {
            is_loop: false,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![Rule {
                    conditions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone()])],
                    )],
                    actions: vec![Bracket::new(
                        CardinalDirection::Right,
                        vec![Neighbor::new(vec![player_any.clone(), star_any.clone()])],
                    )],
                    late: true, // late
                    ..Default::default()
                }],
            }],
        };
        rule.prepare_actions();

        let win1 = WinCondition::On(
            WinConditionOnQualifier::Some,
            player_any.tile.clone(),
            star_any.tile.clone(),
        );

        // 1x1 board
        let mut rng = new_rng();
        let level = Level::Map(vec![vec![Tile::new(
            TileKind::And,
            &String::from("t1"),
            vec![player, star],
        )]]);
        let game = GameData::new(
            String::from("test"),
            Metadata::default(),
            FnvHashMap::default(),
            player_any.tile.clone(),
            background_any.tile.clone(),
            vec![rule],
            vec![level.clone()],
            vec![win1],
        );
        let mut board = game.to_board(&level);

        // verify that the ON condition is be satisfied
        let t = game.evaluate(&mut rng, &mut board, false);
        assert_eq!(t.win, true);
    }

}

#[derive(Debug)]
pub struct SpriteLookup {
    id_to_name: FnvHashMap<u16, String>,
    name_to_id: FnvHashMap<String, SpriteState>,
}

impl SpriteLookup {
    pub fn new(map: &FnvHashMap<SpriteState, Sprite>) -> Self {
        let mut id_to_name = FnvHashMap::default();
        let mut name_to_id = FnvHashMap::default();
        for (k, v) in map {
            id_to_name.insert(k.index, v.name.clone());
            name_to_id.insert(v.name.clone(), k.clone());
        }
        Self {
            id_to_name,
            name_to_id,
        }
    }

    pub fn to_name(&self, id: &u16) -> Option<&String> {
        self.id_to_name.get(id)
    }

    pub fn to_id(&self, name: &String) -> Option<&SpriteState> {
        self.name_to_id.get(name)
    }
}
