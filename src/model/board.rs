use fnv::FnvHashMap;
use fnv::FnvHashSet;

use crate::bitset::BitSet;
use crate::model::cell::Cell;
use crate::model::tile::Tile;
use crate::model::tile::TileKind;

use crate::model::util::Position;
use crate::model::util::WantsToMove;
use crate::model::util::SpriteState;
use crate::model::util::SpriteAndWantsToMove;
use crate::model::util::CardinalDirection;
use crate::model::util::Dimension;

pub struct PositionIter {
    size: Dimension,
    dir: CardinalDirection,
    cur: Option<Position>,
}

impl Iterator for PositionIter {
    type Item = Position;
    fn next(&mut self) -> Option<Position> {
        let ret = self.cur;
        match self.cur {
            None => {},
            Some(cur) => {
                self.cur = match self.dir {
                    CardinalDirection::Up => if cur.y == 0 { None } else { Some(Position::new(cur.x, cur.y - 1)) },
                    CardinalDirection::Down => if cur.y == self.size.height - 1 { None } else { Some(Position::new(cur.x, cur.y + 1)) },
                    CardinalDirection::Left => if cur.x == 0 { None } else { Some(Position::new(cur.x - 1, cur.y)) },
                    CardinalDirection::Right => if cur.x == self.size.width - 1 { None } else { Some(Position::new(cur.x + 1, cur.y)) },
                }
            }
        }
        ret
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Neighbors {
    pub size: Dimension,
    pub dir: CardinalDirection,
    pub start: Position,
}

impl Neighbors {
    pub fn iter(&self) -> PositionIter {
        PositionIter {
            size: self.size,
            dir: self.dir,
            cur: Some(self.start),
        }
    }
    pub fn len(&self) -> usize {
        let ret = match self.dir {
            CardinalDirection::Up => self.start.y + 1,
            CardinalDirection::Down => self.size.height - self.start.y,
            CardinalDirection::Left => self.start.x + 1,
            CardinalDirection::Right => self.size.width - self.start.x,
        };
        ret as usize
    }
    pub fn nth(&self, n: usize) -> Option<Position> {
        self.iter().nth(n)
    }
}


#[derive(Clone, Debug)]
pub struct StripeCache {
    pub sprites: BitSet,
    pub dirs: FnvHashSet<(u16, WantsToMove)>,
    // pub staleness: u16,
}

impl StripeCache {
    pub fn new() -> Self {
        Self { 
            sprites: BitSet::new(), 
            dirs: FnvHashSet::default(),
            // staleness: 0,
        }
    }

    pub fn add_sprite_index(&mut self, collision_layer: u16, sprite_index: u16, dir: WantsToMove) {
        self.sprites.insert(sprite_index);
        self.dirs.insert((collision_layer, dir));
    }

    fn remove_collision_layer(&mut self, _: u16) {
        // self.staleness += 1;
    }
    fn set_wants_to_move(&mut self, collision_layer: u16, dir: WantsToMove) {
        self.dirs.insert((collision_layer, dir));
        // self.staleness += 1;
    }

    pub fn contains_all_dirs(&self, dirs: &FnvHashSet<(u16, WantsToMove)>) -> bool {
        for dir in dirs {
            if !self.dirs.contains(&dir) { return false }
        }
        true
    }
}

#[derive(Clone, Debug)]
pub struct Board {
    pub width: u16,
    pub height: u16, // just a helper. We could compute it from the grid
    grid: Vec<Cell>,
    row_cache: Vec<StripeCache>,
    col_cache: Vec<StripeCache>,
}


impl Board {
    pub fn new(width: u16, height: u16) -> Self {
        let grid = vec![Cell::new(); (width * height) as usize];
        let row_cache = vec![StripeCache::new(); height as usize];
        let col_cache = vec![StripeCache::new(); width as usize];
        Board {
            width,
            height,
            grid,
            row_cache,
            col_cache,
        }
    }

    pub fn size(&self) -> Dimension {
        Dimension { width: self.width, height: self.height }
    }

    pub fn from_tiles(grid: &Vec<Vec<Tile>>, background_tile: &Tile) -> Self {
        let mut board = Self::new(grid[0].len() as u16, grid.len() as u16);
        let mut x;
        let mut y = 0;
        for row in grid {
            x = 0;
            for tile in row {
                let pos = Position::new(x, y);
                // add the background tile to every cell
                for sprite in background_tile.get_sprites() {
                    board.add_sprite(&pos, &sprite, WantsToMove::Stationary);
                }

                match tile.kind {
                    TileKind::Or => panic!("OR Tiles are not allowed to define a cell in a level"),
                    TileKind::And => {
                        for sprite in tile.get_sprites() {
                            board.add_sprite(&pos, &sprite, WantsToMove::Stationary);
                        }
                    },
                }
                x += 1;
            }
            y += 1;
        }
        board
    }

    pub fn from_checkpoint(width: u16, height: u16, grid: Vec<Vec<SpriteState>>) -> Self {
        let mut row_cache = vec![StripeCache::new(); height as usize];
        let mut col_cache = vec![StripeCache::new(); width as usize];

        let grid = grid.iter()
            .enumerate()
            .map(|(index, sprites)| {
                let x = index % width as usize;
                let y = index / width as usize;
                let mut cell = Cell::new();

                for sprite in sprites {
                    cell.add_sprite(sprite, WantsToMove::Stationary);
                    row_cache[y].add_sprite_index(sprite.collision_layer, sprite.index, WantsToMove::Stationary);
                    col_cache[x].add_sprite_index(sprite.collision_layer, sprite.index, WantsToMove::Stationary);
                }
                cell
            })
            .collect();
        Self {
            width,
            height,
            grid,
            row_cache,
            col_cache,
        }
    }

    fn pos(&self, pos: &Position) -> usize {
        let x = pos.x;
        let y = pos.y;
        y as usize * self.width as usize + x as usize
    }

    pub fn positions_iter(&self) -> Vec<Position> {
        (0..self.grid.len()).map(|i| self.index_to_pos(i)).collect()
    }

    fn get(&self, pos: &Position) -> &Cell {
        let pos = self.pos(pos);
        self.grid.get(pos).expect("Position not found on the board")
    }

    /// Get a mutable reference to the cell at (x, y).
    fn get_mut(&mut self, pos: &Position) -> &mut Cell {
        let pos = self.pos(pos);
        self.grid.get_mut(pos).expect("Position not found on the board")
    }

    pub fn get_sprite_states(&self, pos: &Position) -> Vec<SpriteState> {
        let name = String::from("made_by_cell");
        let cell = self.get(pos);

        cell.as_map().iter()
            .map(|(c, w)| SpriteState::new(&name, w.sprite_index, *c))
            .collect()
    }

    pub fn get_sprites_and_dir(&self, pos: &Position) -> Vec<(SpriteState, WantsToMove)> {
        let name = String::from("made_by_cell");
        let cell = self.get(pos);

        let mut ret: Vec<_> = cell.as_map().iter()
            .map(|(c, w)| (SpriteState::new(&name, w.sprite_index, *c), w.wants_to_move))
            .collect();
        ret.sort_by(|a, b| a.0.collision_layer.cmp(&b.0.collision_layer) );
        ret
    }

    /// Add sprites at (x, y).
    pub fn add_sprite(&mut self, pos: &Position, sprite: &SpriteState, dir: WantsToMove) -> bool {
        self.row_cache[pos.y as usize].add_sprite_index(sprite.collision_layer, sprite.index, dir);
        self.col_cache[pos.x as usize].add_sprite_index(sprite.collision_layer, sprite.index, dir);
        let cell = self.get_mut(pos);
        cell.add_sprite(sprite, dir)
    }

    pub fn add_sprite_index(&mut self, pos: &Position, collision_layer: u16, sprite_index: u16, dir: WantsToMove) -> bool {
        self.row_cache[pos.y as usize].add_sprite_index(collision_layer, sprite_index, dir);
        self.col_cache[pos.x as usize].add_sprite_index(collision_layer, sprite_index, dir);
        let cell = self.get_mut(pos);
        cell.add_sprite_index(collision_layer, sprite_index, dir)
    }

    pub fn neighbor_positions(&self, pos: &Position, dir: CardinalDirection) -> Neighbors { // PERF: 15.8%
        Neighbors { size: self.size(), dir, start: pos.clone() }
    }

    pub fn neighbor_position(&self, pos: &Position, dir: CardinalDirection) -> Option<Position> {
        self.neighbor_positions(pos, dir).nth(1)
    }

    fn index_to_pos(&self, index: usize) -> Position {
        let x = index as u16 % self.width;
        let y = index as u16 / self.width;
        assert!(x < self.width);
        assert!(y < self.height);
        Position::new(x, y)
    }

    pub fn has_sprite(&self, pos: &Position, sprite: &SpriteState) -> bool {
        let cell = self.get(pos);
        cell.has_sprite(sprite)
    }

    pub fn as_sprites(&self, pos: &Position) -> &BitSet {
        let cell = self.get(pos);
        &cell.sprite_bits
    }

    pub fn matches(&self, pos: &Position, tile: &Tile, dir: &Option<WantsToMove>) -> bool {
        let cell = self.get(pos);
        cell.matches(tile, dir)
    }

    pub fn get_wants_to_move(&self, pos: &Position, collision_layer: u16) -> Option<WantsToMove> {
        let cell = self.get(pos);
        cell.get_wants_to_move(collision_layer)
    }

    pub fn as_map(&self, pos: &Position) -> &FnvHashMap<u16, SpriteAndWantsToMove> {
        let cell = self.get(pos);
        cell.as_map()
    }

    pub fn has_collision_layer(&self, pos: &Position, collision_layer: u16) -> bool {
        let cell = self.get(pos);
        cell.has_collision_layer(collision_layer)
    }

    pub fn get_collision_layer(&self, pos: &Position, collision_layer: u16) -> Option<&SpriteAndWantsToMove> {
        let cell = self.get(pos);
        cell.get_collision_layer(collision_layer)
    }

    pub fn remove_collision_layer(&mut self, pos: &Position, collision_layer: u16) -> bool {
        self.row_cache[pos.y as usize].remove_collision_layer(collision_layer);
        self.col_cache[pos.x as usize].remove_collision_layer(collision_layer);

        let cell = self.get_mut(pos);
        cell.remove_collision_layer(collision_layer)
    }

    pub fn set_wants_to_move(&mut self, pos: &Position, collision_layer: u16, dir: WantsToMove) -> bool {
        self.row_cache[pos.y as usize].set_wants_to_move(collision_layer, dir);
        self.col_cache[pos.x as usize].set_wants_to_move(collision_layer, dir);

        let cell = self.get_mut(pos);
        cell.set_wants_to_move(collision_layer, dir)
    }

    pub fn row_cache(&self, y: u16) -> &StripeCache {
        &self.row_cache[y as usize]
    }

    pub fn col_cache(&self, x: u16) -> &StripeCache {
        &self.col_cache[x as usize]
    }
}

impl PartialEq for Board {
    fn eq(&self, other: &Board) -> bool {
        self.width == other.width && self.height == other.height && self.grid == other.grid
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbor_positions() {
        let board = Board::new(3, 2);
        let origin = Position::new(0, 0);
        let n = board.neighbor_positions(&origin, CardinalDirection::Up);
        assert_eq!(n.len(), 1); // just the origin
        let mut it = n.iter();
        assert_eq!(it.next(), Some(origin));
        assert_eq!(it.next(), None);

        let n = board.neighbor_positions(&origin, CardinalDirection::Down);
        assert_eq!(n.len(), 2);

        let n = board.neighbor_positions(&origin, CardinalDirection::Left);
        assert_eq!(n.len(), 1);

        let n = board.neighbor_positions(&origin, CardinalDirection::Right);
        assert_eq!(n.len(), 3);
    }

    #[test]
    fn stripe_cache() {
        let mut row = StripeCache::new();
        let mut bracket = StripeCache::new();
        bracket.add_sprite_index(0, 0, WantsToMove::Stationary);

        assert!(!row.sprites.contains_all(&bracket.sprites));

        row.add_sprite_index(0, 0, WantsToMove::Stationary);
        assert!(row.sprites.contains_all(&bracket.sprites));
    }

}