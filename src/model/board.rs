use std::cmp;
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
    pub fn last(&self) -> Option<Position> {
        self.iter().last()
    }
}

fn direction_iter(width: usize, height: usize, index: usize, dir: CardinalDirection, len: usize) -> Vec<usize> {
    let row = index / width;
    let col = index % width;
    
    match dir {
        CardinalDirection::Right => {
            let end = index - col + width;
            let range = index..cmp::min(index + len, end);
            range.into_iter().collect()
        },
        CardinalDirection::Left => {
            let start = index - col;
            let start = if index < len { start } else { cmp::max(index - len, start) }; // prevent overflow
            let range = start..=index;
            range.into_iter().rev().collect()
        }
        CardinalDirection::Up => {
            let start = if row < len { 0 } else { cmp::max(row - len, 0) }; // prevent overflow
            let range = start..=row;
            range.into_iter().map(|x| (x * width + col)).rev().collect()
        }
        CardinalDirection::Down => {
            let range = row..cmp::min(row + len, height);
            range.into_iter().map(|x| (x * width + col)).collect()
        }
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

    fn remove_collision_layer(&mut self, collision_layer: u16) {
        // self.staleness += 1;
    }
    fn set_wants_to_move(&mut self, collision_layer: u16, dir: WantsToMove) {
        self.dirs.insert((collision_layer, dir));
        // self.staleness += 1;
    }

    pub fn contains_all(&self, other: &Self) -> bool {
        self.sprites.contains_all(&other.sprites)
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

    // provide iterator of all the indices to check in a given direction starting with `start`
    fn neighbors(&self, index: usize, dir: CardinalDirection, len: usize) -> Vec<usize> {
        direction_iter(self.width as usize, self.height as usize, index, dir, len)
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
        let ret = cell.remove_collision_layer(collision_layer);

        // if self.row_cache[pos.y as usize].staleness > 0 {
        //     self.recache_row(pos.y);
        // }
        // if self.col_cache[pos.x as usize].staleness > 0 {
        //     self.recache_col(pos.x);
        // }

        ret
    }

    pub fn set_wants_to_move(&mut self, pos: &Position, collision_layer: u16, dir: WantsToMove) -> bool {
        self.row_cache[pos.y as usize].set_wants_to_move(collision_layer, dir);
        self.col_cache[pos.x as usize].set_wants_to_move(collision_layer, dir);

        let cell = self.get_mut(pos);
        let ret = cell.set_wants_to_move(collision_layer, dir);

        // if self.row_cache[pos.y as usize].staleness > 0 {
        //     self.recache_row(pos.y);
        // }
        // if self.col_cache[pos.x as usize].staleness > 0 {
        //     self.recache_col(pos.x);
        // }

        ret
    }

    pub fn row_cache(&self, y: u16) -> &StripeCache {
        &self.row_cache[y as usize]
    }

    pub fn col_cache(&self, x: u16) -> &StripeCache {
        &self.col_cache[x as usize]
    }

    fn recache_row(&mut self, y: u16) {
        let mut row_cache = StripeCache::new();
        for x in 0..self.width {
            for (c, sw) in self.as_map(&Position::new(x, y)) {
                row_cache.add_sprite_index(*c, sw.sprite_index, sw.wants_to_move);
            }
        }
        self.row_cache[y as usize] = row_cache;
    }

    fn recache_col(&mut self, x: u16) {
        let mut col_cache = StripeCache::new();
        for y in 0..self.height {
            for (c, sw) in self.as_map(&Position::new(x, y)) {
                col_cache.add_sprite_index(*c, sw.sprite_index, sw.wants_to_move);
            }
        }
        self.col_cache[x as usize] = col_cache;
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
    fn neighbors() {
        let width = 3;
        let height = 3;

        // check origin
        assert_eq!(direction_iter(width, height, 0, CardinalDirection::Up, 3), vec![0]);
        assert_eq!(direction_iter(width, height, 0, CardinalDirection::Down, 3), vec![0, 3, 6]);
        assert_eq!(direction_iter(width, height, 0, CardinalDirection::Left, 3), vec![0, ]);
        assert_eq!(direction_iter(width, height, 0, CardinalDirection::Right, 3), vec![0, 1, 2]);

        // check center
        assert_eq!(direction_iter(width, height, 4, CardinalDirection::Up, 3), vec![4, 1]);
        assert_eq!(direction_iter(width, height, 4, CardinalDirection::Down, 3), vec![4, 7]);
        assert_eq!(direction_iter(width, height, 4, CardinalDirection::Left, 3), vec![4, 3]);
        assert_eq!(direction_iter(width, height, 4, CardinalDirection::Right, 3), vec![4, 5]);

        // check bottom-right
        assert_eq!(direction_iter(width, height, 8, CardinalDirection::Up, 3), vec![8, 5, 2]);
        assert_eq!(direction_iter(width, height, 8, CardinalDirection::Down, 3), vec![8]);
        assert_eq!(direction_iter(width, height, 8, CardinalDirection::Left, 3), vec![8, 7, 6]);
        assert_eq!(direction_iter(width, height, 8, CardinalDirection::Right, 3), vec![8]);
    }

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

        assert!(!row.contains_all(&bracket));

        row.add_sprite_index(0, 0, WantsToMove::Stationary);
        assert!(row.contains_all(&bracket));
    }

}