use log::{debug, trace};
use std::cmp;
use std::error::Error;
use std::io::Read;
use fnv::FnvHashMap;

use crate::json;
use crate::model::util::SpriteState;
use crate::model::tile::Tile;
use crate::model::tile::TileKind;
use crate::model::tile::TileWithModifier;
use crate::model::neighbor::Neighbor;
use crate::model::bracket::Bracket;
use crate::model::rule::Command;
use crate::model::rule::Rule;
use crate::model::rule::RuleGroup;
use crate::model::rule::RuleLoop;
use crate::model::game::Sprite;
use crate::model::game::Level;
use crate::model::game::GameData;
use crate::model::game::WinCondition;
use crate::model::game::WinConditionOnQualifier;
use crate::model::game::Metadata;
use crate::model::util::Dimension;
use crate::model::util::TriggeredCommands;
use crate::color::ColorSpace;
use crate::color::Rgb;

fn build_sprites(sprite_lookup: &FnvHashMap<String, SpriteState>, sprite_ids: Vec<String>) -> Vec<SpriteState> {
    let mut ret = vec![];
    for sprite_id in sprite_ids {
        let sprite = sprite_lookup.get(&sprite_id).unwrap();
        ret.push(*sprite);
    }
    ret
}

pub fn parse<R: Read>(file: R) -> Result<GameData, Box<Error>> {
    let ast = json::from_file(file)?;

    let mut sprite_map = FnvHashMap::default(); // Map of UI sprites to SpriteState
    let mut sprite_ui_map = FnvHashMap::default(); 

    let mut sprite_id_map = FnvHashMap::default();

    // There are 2 special sprites or tiles: Player and Background. Find them
    let mut player_tile: Option<Tile> = None;
    let mut background_tile: Option<Tile> = None;

    // adjust colors
    let mut color_map = FnvHashMap::default();
    let mut flattened_colors = FnvHashMap::default();

    let mut color_closeness: Vec<_> = ast.colors.keys().collect();    
    color_closeness.sort_by(|a, b| {
        let a = Rgb::parse(a);
        let b = Rgb::parse(b);
        let distance_a = a.distance(&a.to_closest_256());
        let distance_b = b.distance(&b.to_closest_256());
        if distance_a == distance_b { cmp::Ordering::Equal }
        else if distance_a < distance_b { cmp::Ordering::Less }
        else { cmp::Ordering::Greater }
    });

    for hex in color_closeness {
        let color = Rgb::parse(hex);
        let new_color = if ColorSpace::get_colorspace().is_true_color() {
            color
        } else {
            color.to_variant(&mut flattened_colors)
        };
        color_map.insert(hex, new_color);
    }

    debug!("Collision_layers: {}", ast.collision_layers.len());

    let mut sprite_index_global = 0;
    for (id, sprite_def) in ast.sprites {
        let sprites = sprite_id_map.entry(sprite_def.collision_layer).or_insert(vec![]);
        sprites.push((id.clone(), sprite_def.name.clone()));

        let sprite_ui = Sprite {
            id: sprite_index_global,
            name: sprite_def.name.clone(),
            pixels: sprite_def.pixels.iter().map(|row| row.iter().map(|pixel| match pixel {
                None => None,
                Some(hex) => {
                    Some(match color_map.get(hex) {
                        None => panic!("Could not look up '{}' in {:?}", hex, color_map),
                        Some(color) => color.clone(),
                    })
                },
            }).collect()).collect(),
        };
        trace!("Sprite Index [{}] = {}", sprite_index_global, sprite_ui.name);
        sprite_ui_map.insert(id, sprite_ui);
        sprite_index_global += 1;
    }
    debug!("Sprite Count: {}", sprite_index_global);

    let mut sprite_lookup = FnvHashMap::default();
    for (collision_layer, sprite_ids) in sprite_id_map {
        for (sprite_id, name) in sprite_ids {
            
            // Now that we have both the UI Sprite and the SpriteState, associate them together
            let sprite_ui = sprite_ui_map.get(&sprite_id).unwrap();
            let sprite_state = SpriteState::new(&name, sprite_ui.id as u16, collision_layer);
            sprite_map.insert(sprite_state, sprite_ui.clone());

            // Check if they were a Player or Background sprite
            if sprite_ui.name.to_ascii_lowercase() == "player" {
                player_tile = Some(Tile::new(TileKind::And, &sprite_ui.name.clone(), vec![sprite_state]));
            } else if sprite_ui.name.to_ascii_lowercase() == "background" {
                background_tile = Some(Tile::new(TileKind::And, &sprite_ui.name.clone(), vec![sprite_state]));
            }

            sprite_lookup.insert(sprite_id, sprite_state);
        }
    }

    let mut tile_lookup = FnvHashMap::default();
    for (id, tile_def) in ast.tiles {
        let tile = match tile_def {
            json::Tile::Or {name, sprites} => Tile::new(TileKind::Or, &name, build_sprites(&sprite_lookup, sprites)),
            json::Tile::And {name, sprites} => Tile::new(TileKind::And, &name, build_sprites(&sprite_lookup, sprites)),
            json::Tile::Sprite {name, sprite} => Tile::new(TileKind::And, &name, build_sprites(&sprite_lookup, vec![sprite])),
            json::Tile::Simple {name, sprite} => Tile::new(TileKind::And, &name, build_sprites(&sprite_lookup, vec![sprite])),
        };

        // Check if they were a Player or Background Tile
        if tile.name.to_ascii_lowercase() == "player" {
            player_tile = Some(tile.clone());
        } else if tile.name.to_ascii_lowercase() == "background" {
            background_tile = Some(tile.clone());
        }

        tile_lookup.insert(id, tile);
    }

    let mut twm_lookup = FnvHashMap::default();
    for (id, twm_def) in ast.tiles_with_modifiers {
        let twm = TileWithModifier {
            random: twm_def.random,
            negated: twm_def.negated,
            tile: (&tile_lookup).get(&twm_def.tile).unwrap().clone(),
            direction: twm_def.direction
        };
        twm_lookup.insert(id, twm);
    }

    let mut neighbor_lookup = FnvHashMap::default();
    for (id, neighbor_def) in ast.neighbors {
        let twms = neighbor_def.tile_with_modifiers.iter()
            .map(|twm_id| twm_lookup.get(twm_id).unwrap().clone())
            .collect();
        let neighbor = Neighbor::new(twms);
        neighbor_lookup.insert(id, neighbor);
    }

    let mut bracket_lookup = FnvHashMap::default();
    for (id, bracket_def) in ast.brackets {
        let bracket = match bracket_def {
            json::Bracket::Simple { direction, neighbors } => Bracket::new(direction, neighbors.iter().map(|n| neighbor_lookup.get(n).unwrap().clone()).collect()),
            json::Bracket::Ellipsis { direction, before_neighbors, after_neighbors } => {
                let before = before_neighbors.iter().map(|n| neighbor_lookup.get(n).unwrap().clone()).collect();
                let after = after_neighbors.iter().map(|n| neighbor_lookup.get(n).unwrap().clone()).collect();
                Bracket::new_ellipsis(direction, before, after)
            },
        };
        bracket_lookup.insert(id, bracket);
    }

    let mut command_lookup = FnvHashMap::default();
    for (id, command_def) in ast.commands {
        let command = match command_def {
            json::Command::Again {} => Command::Again,
            json::Command::Cancel {} => Command::Cancel,
            json::Command::Checkpoint {} => Command::Checkpoint,
            json::Command::Restart {} => Command::Restart,
            json::Command::Win {} => Command::Win,
            json::Command::Message { message } => Command::Message(message),
            json::Command::Sfx { sound: _ } => Command::Sfx,
        };
        command_lookup.insert(id, command);
    }

    let mut rule_lookup = FnvHashMap::default();
    let mut tbd = vec![];

    for (id, rule_def) in ast.rule_definitions {
        let mut rule = match rule_def {
            json::RuleDefinition::Simple { directions: _, conditions, actions, commands, random, late, rigid } => {
                let r = match random {
                    Some(v) => v,
                    None => false
                };
                trace!("RuleCaching {}", id);
                let mut triggered = TriggeredCommands::default();
                for command in  commands.iter().map(|c| command_lookup.get(c).unwrap().clone()) {
                    command.merge(&mut triggered);
                }
                Rule { causes_board_changes: None,
                    random: r,
                    late,
                    rigid,
                    conditions: conditions.iter().map(|c| bracket_lookup.get(c).unwrap().clone()).collect(),
                    actions: actions.iter().map(|c| bracket_lookup.get(c).unwrap().clone()).collect(),
                    commands: triggered,
                }
            },
            json::RuleDefinition::Group {random, rules } => {
                // Skip Groups because they need to be inserted later
                trace!("RuleSkipping {}", id);
                tbd.push((id, Some(random), rules));
                continue
            }, 
            json::RuleDefinition::Loop { rules } => {
                // Skip Loops because they need to be inserted later
                trace!("RuleSkipping {}", id);
                tbd.push((id, None, rules));
                continue
            }
        };

        rule.prepare_actions();

        rule_lookup.insert(id, rule);
    }

    // Now add the rule groups since we can look up the simple rules
    let mut rule_group_lookup: FnvHashMap<String, RuleGroup> = FnvHashMap::default();
        let mut rule_loop_lookup: FnvHashMap<String, RuleLoop> = FnvHashMap::default();
    trace!("StartingWithGroups");

    for attempt in 0..5 {
        trace!("Looping attempt {}", attempt);

        for (id, random, rules) in tbd.clone() {

            let mut skip = false;
            let mut subrules = vec![];
            rules.iter().for_each(|r| {
                match rule_lookup.get(r) {
                    Some(r) => {
                        subrules.push(r.clone())
                    },
                    None => {
                        match rule_group_lookup.get(r) {
                            None => skip = true,
                            Some(group) => {
                                subrules.append(&mut group.rules.clone());
                            },
                        }
                    },
                }
            });

            if !skip {
                // Random is also a marker for RuleGroup (as opposed to RuleLoop)
                match random {
                    Some(random) => {
                        let rule_group = RuleGroup {
                            random,
                            rules: subrules
                        };

                        if !rule_group_lookup.contains_key(&id) {
                            trace!("RuleCaching2 {}", id);
                            rule_group_lookup.insert(id, rule_group);
                        }
                    },
                    None => {},
                }
            }
        }
    }

    // Now, build up the RuleLoops
    for (id, random, rules) in tbd.clone() {
        if random.is_some() {
            continue // Skip RuleGroups
        }
        let skip = false;
        let mut subrules = vec![];
        rules.iter().for_each(|r| {
            match rule_group_lookup.get(r) {
                Some(r) => {
                    subrules.push(r.clone())
                },
                None => panic!("Could not find Rule Group for the loop. Maybe it was a single Rule? {}", r),
            }
        });

        if !skip {
            // Random is also a marker for RuleGroup (as opposed to RuleLoop)
            match random {
                Some(_) => {}, // these are for groups which we have already processed
                None => {
                    let rule = RuleLoop {
                        is_loop: true,
                        rules: subrules
                    };

                    if !rule_loop_lookup.contains_key(&id) {
                        trace!("RuleCaching2 {}", id);
                        rule_loop_lookup.insert(id, rule);
                    }

                },
            }
        }
    }


    let rules = ast.rules.iter().map(|r| {
        match rule_lookup.get(r) {
            Some(simple_rule) => RuleLoop { is_loop: false, rules: vec![ RuleGroup { random: simple_rule.random, rules: vec![simple_rule.clone()] } ] },
            None => {
                // Look up in the rule group
                match rule_group_lookup.get(r) {
                    Some(rule_group) => RuleLoop { is_loop: false, rules: vec![ rule_group.clone() ] },
                    None => {
                        match rule_loop_lookup.get(r) {
                            None => panic!("Could not find {}", r),
                            Some(rule_loop) => rule_loop.clone(),
                        }
                    },
                }
            }
        }
    }).collect();
    let levels = ast.levels.iter().map(|l| {
        match l {
            json::Level::Message { message } => Level::Message(message.clone()),
            json::Level::Map { cells } => {
                let tiles = cells.iter().map(|row| row.iter().map(|t| tile_lookup.get(t).unwrap().clone()).collect()).collect();
                Level::Map(tiles)
            }
        }
    }).collect();

    let win_conditions = ast.win_conditions.iter().map(|w| {
        match w {
            json::WinCondition::Simple {qualifier, tile} => {
                let qualifier = match qualifier {
                    json::WinConditionOnQualifier::All => WinConditionOnQualifier::All,
                    json::WinConditionOnQualifier::No => WinConditionOnQualifier::No,
                    json::WinConditionOnQualifier::Some => WinConditionOnQualifier::Some,
                    json::WinConditionOnQualifier::Any => WinConditionOnQualifier::Any,
                };
                let tile = tile_lookup.get(tile).unwrap();
                WinCondition::Simple(qualifier, tile.clone())
            },
            json::WinCondition::On {qualifier, tile, on_tile} => {
                let qualifier = match qualifier {
                    json::WinConditionOnQualifier::All => WinConditionOnQualifier::All,
                    json::WinConditionOnQualifier::No => WinConditionOnQualifier::No,
                    json::WinConditionOnQualifier::Some => WinConditionOnQualifier::Some,
                    json::WinConditionOnQualifier::Any => WinConditionOnQualifier::Any,
                };
                let tile = tile_lookup.get(tile).unwrap();
                let on_tile = tile_lookup.get(on_tile).unwrap();
                WinCondition::On(qualifier, tile.clone(), on_tile.clone())
            }
        }
    }).collect();

    let metadata = Metadata {
        author: ast.metadata.author.clone(),
        homepage: ast.metadata.homepage.clone(),
        youtube: ast.metadata.youtube.clone(),
        zoomscreen: ast.metadata.zoomscreen.map(|d| Dimension{ width: d.width, height: d.height }),
        flickscreen: ast.metadata.flickscreen.map(|d| Dimension{ width: d.width, height: d.height }),
        color_palette: ast.metadata.color_palette.clone(),
        background_color: ast.metadata.background_color.map(|h| Rgb::parse(&h)),
        text_color: ast.metadata.text_color.map(|h| Rgb::parse(&h)),
        realtime_interval: ast.metadata.realtime_interval.clone(),
        key_repeat_interval: ast.metadata.key_repeat_interval.clone(),
        again_interval: ast.metadata.again_interval.clone(),
        no_action: ast.metadata.no_action,
        no_undo: ast.metadata.no_undo,
        run_rules_on_level_start: ast.metadata.run_rules_on_level_start.clone(),
        no_repeat_action: ast.metadata.no_repeat_action,
        throttle_movement: ast.metadata.throttle_movement.unwrap_or(false),
        no_restart: ast.metadata.no_restart.unwrap_or(false),
        require_player_movement: ast.metadata.require_player_movement.unwrap_or(false),
        verbose_logging: ast.metadata.verbose_logging.unwrap_or(false),
    };

    Ok(GameData::new(ast.title, metadata, sprite_map, player_tile.unwrap(), background_tile.unwrap(), rules, levels, win_conditions))
}
