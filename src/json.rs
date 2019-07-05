// #[cfg(feature = "serde")]
extern crate serde;
use serde::{Serialize, Deserialize};

use fnv::FnvHashMap;
use std::error::Error;
use std::io::Read;
use std::io::BufReader;

use crate::model::util::CardinalDirection;
use crate::model::util::WantsToMove;

#[derive(Serialize, Deserialize, Debug)]
pub struct GameMap {
  pub title: String,
  pub metadata: Metadata,
  pub colors: FnvHashMap<String, String>,
  pub collision_layers: Vec<CollisionLayer>,
  pub commands: FnvHashMap<String, Command<String>>,
  pub sprites: FnvHashMap<String, Sprite<u16>>,
  pub tiles: FnvHashMap<String, Tile<String>>,
  pub tiles_with_modifiers: FnvHashMap<String, TileWithModifier<String>>,
  pub neighbors: FnvHashMap<String, Neighbor<String>>,
  pub brackets: FnvHashMap<String, Bracket<String>>,
  pub rule_definitions: FnvHashMap<String, RuleDefinition<String, String, String>>,
  pub rules: Vec<String>,
  pub levels: Vec<Level<String>>,
  pub win_conditions: Vec<WinCondition<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Dimension {
  pub width: u16,
  pub height: u16
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub youtube: Option<String>,
    pub zoomscreen: Option<Dimension>,
    pub flickscreen: Option<Dimension>,
    pub color_palette: Option<String>,
    pub background_color: Option<String>,
    pub text_color: Option<String>,
    pub realtime_interval: Option<f32>,
    pub key_repeat_interval: Option<f32>,
    pub again_interval: Option<f32>,
    pub no_action: bool,
    pub no_undo: bool,
    pub run_rules_on_level_start: Option<bool>,
    pub no_repeat_action: bool,
    pub scanline: Option<bool>,
    pub throttle_movement: Option<bool>,
    pub no_restart: Option<bool>,
    pub require_player_movement: Option<bool>,
    pub verbose_logging: Option<bool>,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct CollisionLayer {
  id: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command<Sound> {
  Win {},
  Again {},
  Cancel {},
  Checkpoint {},
  Restart {},
  Message { message: String },
  Sfx { sound: Sound },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sprite<CollisionLayer> {
  pub name: String,
  pub collision_layer: CollisionLayer,
  pub pixels: Vec<Vec<Option<String>>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Tile<Sprite> {
  Or {
    name: String,
    sprites: Vec<Sprite>,
    // collision_layers: Vec<C>,
  },
  And {
    name: String,
    sprites: Vec<Sprite>,
    // collision_layers: Vec<C>,
  },
  Sprite { // Like an "And" tile but with only 1 item
    name: String,
    sprite: Sprite,
  },
  Simple {
    name: String,
    sprite: Sprite,
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TileWithModifier<Tile> {
  pub direction: Option<WantsToMove>,
  pub negated: bool,
  pub random: bool,
  pub tile: Tile,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Neighbor<TileWithModifier> {
  pub tile_with_modifiers: Vec<TileWithModifier>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Bracket<Neighbor> {
  Simple {
    direction: CardinalDirection,
    neighbors: Vec<Neighbor>,
  },
  Ellipsis {
    direction: CardinalDirection,
    before_neighbors: Vec<Neighbor>,
    after_neighbors: Vec<Neighbor>,
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RuleDefinition<SubRuleDefinition, Bracket, Command> {
  Simple {
    directions: Vec<CardinalDirection>,
    conditions: Vec<Bracket>,
    actions: Vec<Bracket>,
    commands: Vec<Command>,
    random: Option<bool>,
    late: bool,
    rigid: bool,
  },

  Group {
    random: bool,
    rules: Vec<SubRuleDefinition>,
  },

  Loop {
    rules: Vec<SubRuleDefinition>,
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Level<Tile> {
  Message { message: String },
  Map { cells: Vec<Vec<Tile>> },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WinConditionOnQualifier {
    All,
    No,
    Some,
    Any,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WinCondition<Tile> {
    On { qualifier: WinConditionOnQualifier, tile: Tile, on_tile: Tile },
    Simple { qualifier: WinConditionOnQualifier, tile: Tile},
}



pub fn from_file<R: Read>(file: R) -> Result<GameMap, Box<Error>> {
    let reader = BufReader::new(file);
    let u = serde_json::from_reader(reader)?;
    Ok(u)
}