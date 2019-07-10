use log::trace;

use fnv::FnvHashMap;
use rand::Rng;
use std::fmt;

use crate::debugger::ScreenDumper;
use crate::model::board::Board;
use crate::model::bracket::Bracket;
use crate::model::bracket::BracketMatch;
use crate::model::util::Position;
use crate::model::util::TriggeredCommands;

#[derive(Clone, Debug)]
pub enum Command {
    Message(String),
    Again,
    Cancel,
    Checkpoint,
    Restart,
    Win,
    Sfx,
}

impl Command {
    pub fn merge(&self, t: &mut TriggeredCommands) {
        match self {
            Command::Again => t.again = true,
            Command::Cancel => t.cancel = true,
            Command::Checkpoint => t.checkpoint = true,
            Command::Restart => t.restart = true,
            Command::Win => t.win = true,
            Command::Sfx => t.sfx = true,
            Command::Message(m) => t.message = Some(m.clone()),
        }
    }
}

// Converts `[ [1,2], [a,b] ]` to:
// `[ [1,a], [2,a], [1,b], [2,b] ]`
fn build_permutations<T: Clone>(cells: &Vec<Vec<T>>) -> Vec<Vec<T>> {
    let mut tuples: Vec<Vec<T>> = vec![vec![]];
    for row in cells {
        let mut newtuples: Vec<Vec<T>> = vec![];
        for valtoappend in row {
            for tuple in tuples.clone() {
                let mut newtuple = tuple.clone();
                newtuple.push(valtoappend.clone());
                newtuples.push(newtuple);
            }
        }
        tuples = newtuples;
    }
    return tuples;
}

#[derive(Clone, Default, Debug)]
pub struct Rule {
    pub conditions: Vec<Bracket>,
    pub actions: Vec<Bracket>,
    pub commands: TriggeredCommands,
    pub random: bool,
    pub late: bool,
    pub rigid: bool,
    pub causes_board_changes: Option<bool>,
}

impl Rule {
    pub fn prepare_actions(&mut self) {
        if !self.actions.is_empty() {
            assert_eq!(self.conditions.len(), self.actions.len());
        }
        let mut causes_board_changes = false;

        self.conditions
            .iter_mut()
            .zip(&self.actions)
            .for_each(|(c, a)| causes_board_changes |= c.prepare_actions(&a));

        self.causes_board_changes = Some(causes_board_changes);
    }
    pub fn evaluate<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        board: &mut Board,
        triggered: &mut TriggeredCommands,
        eval_once: bool,
    ) -> bool {
        trace!("Evaluating Rule '{}'... ", self);
        let mut applied_pos = vec![];
        let mut board_changed_at_least_once = false;

        // Initially find all the matches for each bracket.
        // The dimensions are condition x match x neighbor
        let matches: Vec<Vec<_>> = self.find_matches(board);
        trace!("Matches found {:?}", matches);

        // If each bracket contains at least 1 match then the rule is satisfied
        if !matches.is_empty() && matches.iter().all(|m| m.len() > 0) {
            // Since the conditions matched, set the commands
            triggered.merge(&self.commands);

            // A rule can have no actions, just commands
            if self.has_only_commands() {
                ScreenDumper::dump(
                    board,
                    triggered,
                    &format!(
                        "Evaluated command-only rule {} '{}'... ",
                        if eval_once { "ONCE" } else { "" },
                        self
                    ),
                );
                return false;
            } else {
                // Evaluate all permutations but check to make sure each perm still matches
                let perms = build_permutations(&matches);
                trace!("Found {} permutations", perms.len());

                for perm in perms {
                    assert_eq!(perm.len(), self.conditions.len());

                    // Populate the magic_or_tiles & check if each condition stil matches
                    let mut magic_or_tiles = FnvHashMap::default();
                    let still_matches = self
                        .conditions
                        .iter()
                        .zip(&perm)
                        .map(|(c, m)| {
                            c.populate_magic_or_tiles(board, &mut magic_or_tiles, m.clone());
                            c.matches(board, m.clone())
                        })
                        .all(|x| x);

                    if still_matches {
                        self.conditions.iter().zip(perm).for_each(|(c, p)| {
                            // Check again that the cell matches because a previous
                            // permutation could have caused the cell to change
                            if c.matches(board, p.clone()) {
                                applied_pos.push(p.clone());
                                board_changed_at_least_once |=
                                    c.evaluate(rng, board, p.clone(), &magic_or_tiles);
                            }
                        });
                    }

                    if eval_once && board_changed_at_least_once {
                        ScreenDumper::dump(
                            board,
                            triggered,
                            &format!(
                                "Evaluated_shouldbefalse? {} '{}'... ",
                                if eval_once { "RNDM" } else { "" },
                                self
                            ),
                        );
                        return board_changed_at_least_once;
                    }
                }
            }
        }
        if !board_changed_at_least_once {
            trace!("unchanged board");
            return false;
        }

        if ScreenDumper::is_enabled() {
            ScreenDumper::dump(
                board,
                &triggered,
                &format!(
                    "Evaluated {} '{}'... ",
                    if eval_once { "ONCE" } else { "" },
                    self
                ),
            );
        }
        trace!("Board changed? {}", board_changed_at_least_once);
        board_changed_at_least_once
    }

    fn find_matches(&self, board: &Board) -> Vec<Vec<BracketMatch>> {
        let mut ret = vec![];

        for c in &self.conditions {
            let mut matches = vec![];
            // Loop over each row or col depending on the bracket dir
            if c.is_horizontal() {
                for y in 0..board.height {
                    let cache = board.row_cache(y);
                    if c.matches_cache(&cache) {
                        for x in 0..board.width {
                            let pos = Position::new(x, y);
                            matches.append(&mut c.find_match(board, &pos));
                        }
                    }
                }
            } else {
                // vertical
                for x in 0..board.width {
                    let cache = board.col_cache(x);
                    if c.matches_cache(&cache) {
                        for y in 0..board.height {
                            let pos = Position::new(x, y);
                            matches.append(&mut c.find_match(board, &pos));
                        }
                    }
                }
            }
            // Bail everything if no match found for the bracket
            if matches.is_empty() {
                return vec![];
            }

            ret.push(matches)
        }
        ret
    }

    fn has_only_commands(&self) -> bool {
        !self
            .causes_board_changes
            .expect("Should have called prepare_actions")
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.has_only_commands() {
            write!(f, "(commands-only)")?
        }
        if self.random {
            write!(f, "RANDOM ")?
        }
        if self.rigid {
            write!(f, "RIGID ")?
        }
        if self.late {
            write!(f, "LATE ")?
        }

        for b in &self.conditions {
            write!(f, "{}", b)?;
        }
        write!(f, " -> ")?;
        for b in &self.actions {
            write!(f, "{}", b)?;
        }
        write!(f, " {}", &self.commands)
    }
}

#[derive(Clone, Debug)]
pub struct RuleGroup {
    pub random: bool,
    pub rules: Vec<Rule>,
}

impl RuleGroup {
    pub fn prepare_actions(&mut self) {
        self.rules.iter_mut().for_each(|r| r.prepare_actions())
    }
    pub fn evaluate<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        board: &mut Board,
        triggered: &mut TriggeredCommands,
        late: bool,
    ) -> bool {
        trace!("Start RuleGroup '{}'... ", self);
        if self.random {
            let rnd = rng.gen_range(0, self.rules.len());
            // Keep trying a rule until one matches
            let mut ret = false;
            let mut offset = 0;
            while !ret {
                if offset > self.rules.len() {
                    break;
                }
                let rule = &self.rules[(rnd + offset) % self.rules.len()];
                if rule.late == late {
                    let before = triggered.clone();
                    ret = rule.evaluate(rng, board, triggered, true);
                    ret |= before != *triggered;
                }
                offset += 1;
            }
            ret
        } else {
            // https://www.puzzlescript.net/Documentation/executionorder.html
            // "So the question is: When I say that each rule is executed in turn to exhaustion,
            // do I mean the few rules you write, or the many rules the interpreter ends up with? "
            let mut board_changed = false;
            let mut iteration = 0;
            loop {
                let mut board_changed_this_iter = false;
                iteration += 1;
                if iteration > 1000 {
                    panic!("RuleGroup Looped too many times")
                };

                let before = triggered.clone();
                self.rules.iter().filter(|r| r.late == late).for_each(|r| {
                    // Rules with only commands would keep running infinitely. So if something was evaluated
                    let mut ret;
                    loop {
                        // keep evaluating the rule until it is false (entanglement-two putting an arrow in a vactube)
                        ret = r.evaluate(rng, board, triggered, false);
                        board_changed_this_iter |= ret;
                        if !ret {
                            break;
                        }
                    }
                });
                board_changed |= board_changed_this_iter;
                if !board_changed_this_iter && before == *triggered {
                    break;
                }
            }
            trace!("RuleGroup board changed? {}", board_changed);
            board_changed
        }
    }
}

impl fmt::Display for RuleGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "STARTGROUP\n")?;
        for r in &self.rules {
            write!(f, "{}\n", r)?;
        }
        write!(f, "ENDGROUP\n")?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RuleLoop {
    pub is_loop: bool,
    pub rules: Vec<RuleGroup>,
}

impl RuleLoop {
    pub fn prepare_actions(&mut self) {
        self.rules.iter_mut().for_each(|r| r.prepare_actions())
    }
    pub fn evaluate<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        board: &mut Board,
        late: bool,
    ) -> TriggeredCommands {
        trace!("Start RuleLoop/Group:loop?{} '{}'... ", self.is_loop, self);
        let mut ret = TriggeredCommands::default();

        let mut iterations = 0;
        loop {
            let mut evaluated_something = false;

            for rule in &self.rules {
                evaluated_something |= rule.evaluate(rng, board, &mut ret, late);
            }

            // Only evaluate the rules once if this is _really_ a RuleGroup
            if !self.is_loop {
                break;
            }

            if !evaluated_something {
                break;
            }

            if self.is_loop {
                trace!("Iteration {}", iterations);
            }
            iterations += 1;
            if iterations > 1000 {
                panic!("Looped more than 1000 times")
            }
        }
        trace!("End RuleLoop.");
        ret
    }
}

impl fmt::Display for RuleLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "STARTLOOP")?;
        for i in &self.rules {
            writeln!(f, "{}", i)?;
        }
        writeln!(f, "ENDLOOP")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::SeedableRng;
    use rand_xorshift::XorShiftRng;

    use crate::model::neighbor::build_t;
    use crate::model::neighbor::build_tile_with_modifier;
    use crate::model::neighbor::tests::check_counts;
    use crate::model::neighbor::Neighbor;
    use crate::model::util::CardinalDirection;
    use crate::model::util::Position;
    use crate::model::util::SpriteState;
    use crate::model::util::WantsToMove;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn new_rng() -> XorShiftRng {
        XorShiftRng::from_seed([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
    }

    #[test]
    fn rule_crawler() {
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false /*random*/, &player, false, None);
        let no_player = build_t(false /*random*/, &player, true, None);

        // RIGHT [ player | NO player ] -> [ | player ]
        let rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![player_any.clone()]),
                    Neighbor::new(vec![no_player.clone()]),
                ],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![]),
                    Neighbor::new(vec![player_any.clone()]),
                ],
            )],
            commands: TriggeredCommands::default(),
            late: false,
            random: false,
            rigid: false,
        };
        let mut rule = RuleGroup {
            random: false,
            rules: vec![rule],
        };

        rule.prepare_actions();
        trace!("{}", rule);

        check_counts(&rule.rules[0].conditions[0].before_neighbors[0], 0, 0, 1);
        check_counts(&rule.rules[0].conditions[0].before_neighbors[1], 0, 1, 0);

        let mut board = Board::new(10, 1);
        let end = Position::new(9, 0);
        board.add_sprite(&Position::new(0, 0), &player, WantsToMove::Stationary);

        rule.evaluate(
            &mut rng,
            &mut board,
            &mut TriggeredCommands::default(),
            false,
        );

        // make sure we did crawl

        let sprite_count = board.as_map(&end).keys().len();
        assert_eq!(sprite_count, 1);
    }

    #[test]
    fn permutations() {
        // Converts `[ [1,2], [a,b] ]` to:
        // `[ [1,a], [2,a], [1,b], [2,b] ]`

        let one = Position::new(11, 11);
        let two = Position::new(22, 22);
        let aaa = Position::new(33, 33);
        let bbb = Position::new(44, 44);

        let src = vec![vec![one, two], vec![aaa, bbb]];

        let dest = build_permutations(&src);

        assert_eq!(dest[0][0], one);
        assert_eq!(dest[0][1], aaa);

        assert_eq!(dest[1][0], two);
        assert_eq!(dest[1][1], aaa);

        assert_eq!(dest[2][0], one);
        assert_eq!(dest[2][1], bbb);

        assert_eq!(dest[3][0], two);
        assert_eq!(dest[3][1], bbb);
    }

    #[test]
    fn recalc_permutations() {
        init();
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_right = build_t(
            false, /*random*/
            &player,
            false,
            Some(WantsToMove::Right),
        );
        let player_stationary = build_t(
            false, /*random*/
            &player,
            false,
            Some(WantsToMove::Stationary),
        );

        // RIGHT [ > player | STATIONARY player ] -> [ > player | > player ]
        let rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![player_right.clone()]),
                    Neighbor::new(vec![player_stationary.clone()]),
                ],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![player_right.clone()]),
                    Neighbor::new(vec![player_right.clone()]),
                ],
            )],
            commands: TriggeredCommands::default(),
            late: false,
            random: false,
            rigid: false,
        };
        let mut rule = RuleGroup {
            random: false,
            rules: vec![rule],
        };

        rule.prepare_actions();
        trace!("{}", rule);

        let mut board = Board::new(4, 1);
        let origin = Position::new(0, 0);
        let middle = Position::new(1, 0);
        let end = Position::new(2, 0);

        board.add_sprite(&origin, &player, WantsToMove::Right);
        board.add_sprite(&middle, &player, WantsToMove::Stationary);
        board.add_sprite(&end, &player, WantsToMove::Stationary);

        assert_eq!(
            board.get_wants_to_move(&middle, player.collision_layer),
            Some(WantsToMove::Stationary)
        );
        assert_eq!(
            board.get_wants_to_move(&end, player.collision_layer),
            Some(WantsToMove::Stationary)
        );

        rule.evaluate(
            &mut rng,
            &mut board,
            &mut TriggeredCommands::default(),
            false,
        );

        assert_eq!(
            board.get_wants_to_move(&middle, player.collision_layer),
            Some(WantsToMove::Right)
        );
        // This is the actual test:
        assert_eq!(
            board.get_wants_to_move(&end, player.collision_layer),
            Some(WantsToMove::Right)
        );
    }

    #[test]
    fn rule_group_keeps_looping() {
        init();
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_right = build_t(
            false, /*random*/
            &player,
            false,
            Some(WantsToMove::Right),
        );
        let player_stationary = build_t(
            false, /*random*/
            &player,
            false,
            Some(WantsToMove::Stationary),
        );

        // RIGHT [ > player | STATIONARY player ] -> [ > player | > player ]
        let rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![player_right.clone()]),
                    Neighbor::new(vec![player_stationary.clone()]),
                ],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![player_right.clone()]),
                    Neighbor::new(vec![player_right.clone()]),
                ],
            )],
            commands: TriggeredCommands::default(),
            late: false,
            random: false,
            rigid: false,
        };
        let mut rule = RuleGroup {
            random: false,
            rules: vec![rule],
        };

        rule.prepare_actions();
        trace!("{}", rule);

        let mut board = Board::new(4, 1);
        let origin = Position::new(0, 0);
        let middle = Position::new(1, 0);
        let end = Position::new(2, 0);

        board.add_sprite(&origin, &player, WantsToMove::Right);
        board.add_sprite(&middle, &player, WantsToMove::Stationary);
        board.add_sprite(&end, &player, WantsToMove::Stationary);

        assert_eq!(
            board.get_wants_to_move(&middle, player.collision_layer),
            Some(WantsToMove::Stationary)
        );
        assert_eq!(
            board.get_wants_to_move(&end, player.collision_layer),
            Some(WantsToMove::Stationary)
        );

        rule.evaluate(
            &mut rng,
            &mut board,
            &mut TriggeredCommands::default(),
            false,
        );

        assert_eq!(
            board.get_wants_to_move(&middle, player.collision_layer),
            Some(WantsToMove::Right)
        );
        // This is the actual test:
        assert_eq!(
            board.get_wants_to_move(&end, player.collision_layer),
            Some(WantsToMove::Right)
        );
    }

    #[test]
    fn random_runs_once() {
        init();
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false /*random*/, &player, false, None);

        // RANDOM RIGHT [ ] -> [ player ]
        let mut rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![])],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![player_any.clone()])],
            )],
            commands: TriggeredCommands::default(),
            late: false,
            random: false, // unused
            rigid: false,
        };
        rule.prepare_actions();
        trace!("{}", rule);

        let mut board = Board::new(2, 1);
        let origin = Position::new(0, 0);
        let end = Position::new(1, 0);

        rule.evaluate(
            &mut rng,
            &mut board,
            &mut TriggeredCommands::default(),
            true,
        ); // RANDOM so run once

        assert!(board.has_sprite(&origin, &player) ^ board.has_sprite(&end, &player));
    }

    #[test]
    fn command_only() {
        init();
        let mut rng = new_rng();

        // RIGHT [ ] -> WIN
        let mut rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(CardinalDirection::Right, vec![])],
            actions: vec![],
            commands: TriggeredCommands {
                win: true,
                ..Default::default()
            },
            late: false,
            random: false,
            rigid: false,
        };
        rule.prepare_actions();

        let mut board = Board::new(1, 1);

        let mut commands = TriggeredCommands::default();
        assert!(
            !rule.evaluate(&mut rng, &mut board, &mut commands, false),
            "Board should not have changed, only the triggered commands"
        );
        assert!(commands.win);
    }

    #[test]
    fn command_and_nonchanging_neighbors() {
        init();
        let mut rng = new_rng();

        // RIGHT [ ] -> [ ] WIN
        let mut rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(CardinalDirection::Right, vec![])],
            actions: vec![Bracket::new(CardinalDirection::Right, vec![])],
            commands: TriggeredCommands {
                win: true,
                ..Default::default()
            },
            late: false,
            random: false,
            rigid: false,
        };
        rule.prepare_actions();

        let mut board = Board::new(1, 1);

        let mut commands = TriggeredCommands::default();
        assert!(
            !rule.evaluate(&mut rng, &mut board, &mut commands, false),
            "Board should not have changed"
        );
        assert!(commands.win);
    }

    #[test]
    fn nonchanging_neighbors_does_not_loop_forever() {
        init();
        let mut rng = new_rng();

        // RIGHT [ ] -> [ ]
        let rule = Rule {
            causes_board_changes: None,
            conditions: vec![Bracket::new(CardinalDirection::Right, vec![])],
            actions: vec![Bracket::new(CardinalDirection::Right, vec![])],
            commands: TriggeredCommands::default(),
            late: false,
            random: false,
            rigid: false,
        };
        let mut rule_loop = RuleLoop {
            is_loop: true,
            rules: vec![RuleGroup {
                random: false,
                rules: vec![rule],
            }],
        };
        rule_loop.prepare_actions();

        let mut board = Board::new(1, 1);

        // test that we do not loop indefinitely
        rule_loop.evaluate(&mut rng, &mut board, false);
    }

    #[test]
    fn rule_exhaustion() {
        init();
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let player_any = build_t(false /*random*/, &player, false, None);

        let crate_sprite = SpriteState::new(&String::from("crate_sprite"), 1, 0);
        let crate_any = build_t(false /*random*/, &crate_sprite, false, None);

        let wall = SpriteState::new(&String::from("wall"), 2, 0);
        let wall_any = build_t(false /*random*/, &wall, false, None);

        // RIGHT [ Crate ] -> [ Player ]
        // + RIGHT [ Crate ] -> [ Wall ]
        let rule1 = Rule {
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![crate_any.clone()])],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![player_any.clone()])],
            )],
            ..Default::default()
        };
        let rule2 = Rule {
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![crate_any.clone()])],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![wall_any.clone()])],
            )],
            ..Default::default()
        };
        let mut rule = RuleGroup {
            random: false,
            rules: vec![rule1, rule2],
        };
        rule.prepare_actions();

        let mut board = Board::new(2, 1);
        let origin = Position::new(0, 0);
        let end = Position::new(1, 0);

        board.add_sprite(&origin, &crate_sprite, WantsToMove::Stationary);
        board.add_sprite(&end, &crate_sprite, WantsToMove::Stationary);

        rule.evaluate(
            &mut rng,
            &mut board,
            &mut TriggeredCommands::default(),
            false,
        );

        assert!(board.has_sprite(&origin, &player));
        assert!(board.has_sprite(&end, &player));
    }

    #[test]
    fn keeps_evaluating_rule_in_group_before_moving_to_next() {
        // entanglement-two (pushing an arrow into a vactube)
        init();
        let mut rng = new_rng();
        let player = SpriteState::new(&String::from("player"), 0, 0);
        let hat = SpriteState::new(&String::from("hat"), 1, 1);
        let movestack = SpriteState::new(&String::from("movestack"), 2, 2);

        let thing_any =
            build_tile_with_modifier(false, true, false, None, &vec![player.clone(), hat.clone()]);

        let no_movestack = build_t(false /*random*/, &movestack, true, None);
        let movestack_any = build_t(false /*random*/, &movestack, false, None);

        // RIGHT [ thing movestack | ] -> [ movestack | thing ]
        // + RIGHT [ movestack ] -> [ ]
        let rule1 = Rule {
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![thing_any.clone(), movestack_any.clone()]),
                    Neighbor::new(vec![]),
                ],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![
                    Neighbor::new(vec![movestack_any.clone()]),
                    Neighbor::new(vec![thing_any.clone()]),
                ],
            )],
            ..Default::default()
        };

        let rule2 = Rule {
            conditions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![movestack_any.clone()])],
            )],
            actions: vec![Bracket::new(
                CardinalDirection::Right,
                vec![Neighbor::new(vec![no_movestack.clone()])],
            )],
            ..Default::default()
        };

        let mut rule = RuleGroup {
            random: false,
            rules: vec![rule1, rule2],
        };
        rule.prepare_actions();

        let mut board = Board::new(2, 1);
        let origin = Position::new(0, 0);
        let end = Position::new(1, 0);

        board.add_sprite(&origin, &player, WantsToMove::Stationary);
        board.add_sprite(&origin, &hat, WantsToMove::Stationary);
        board.add_sprite(&origin, &movestack, WantsToMove::Stationary);

        rule.evaluate(
            &mut rng,
            &mut board,
            &mut TriggeredCommands::default(),
            false,
        );

        assert!(!board.has_sprite(&origin, &player));
        assert!(!board.has_sprite(&origin, &hat));

        assert!(board.has_sprite(&end, &player));
        assert!(board.has_sprite(&end, &hat));
    }

}
