use std::cmp;
use std::io::stdout;

use std::cell::RefCell;

use log::trace;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

use tui::layout::Rect;

use crate::color::ColorSpace;
use crate::color::Rgb;
use crate::model::board::Board;
use crate::model::game::Sprite;
use crate::model::util::Position;
use crate::model::util::SpriteState;
use crate::model::util::TriggeredCommands;
use crate::model::util::WantsToMove;
use fnv::FnvHashMap;

thread_local!(static SCREENDUMPER: RefCell<ScreenDumper> = RefCell::new(ScreenDumper::new()));

pub struct ScreenDumper {
    term: Option<RawTerminal<std::io::Stdout>>,
    sprite_size: (u16, u16),
    sprites: Option<FnvHashMap<SpriteState, Sprite>>,
    prev_board: Option<Board>,
    prev_triggered: Option<TriggeredCommands>,

    // for large games, only dump the window
    rect: Option<Rect>,
}

impl ScreenDumper {
    fn new() -> Self {
        Self {
            sprite_size: (0, 0),
            sprites: None,
            term: None,
            prev_board: None,
            prev_triggered: None,
            rect: None,
        }
    }
    pub fn dump(board: &Board, triggered: &TriggeredCommands, message: &String) {
        SCREENDUMPER.with(|obj_cell| {
            let mut obj = obj_cell.borrow_mut();
            obj._dump(board, triggered, message)
        })
    }

    pub fn set_term() -> bool {
        SCREENDUMPER.with(|obj_cell| {
            let mut obj = obj_cell.borrow_mut();
            if obj.term.is_none() {
                match stdout().into_raw_mode() {
                    Ok(x) => {
                        obj.term = Some(x);
                        true
                    }
                    Err(_) => false,
                }
            } else {
                true
            }
        })
    }

    pub fn set_sprites(sprites: Option<FnvHashMap<SpriteState, Sprite>>) {
        SCREENDUMPER.with(|obj_cell| {
            let mut obj = obj_cell.borrow_mut();
            if let Some(sprites) = &sprites {
                let (_, sprite) = sprites.iter().next().unwrap();
                obj.sprite_size = (sprite.pixels[0].len() as u16, sprite.pixels.len() as u16);
            }
            obj.sprites = sprites
        })
    }

    pub fn set_window(rect: Rect) {
        trace!("Setting window size to be {:?}", rect);
        SCREENDUMPER.with(|obj_cell| {
            let mut obj = obj_cell.borrow_mut();
            obj.rect = Some(rect);
        })
    }

    pub fn is_enabled() -> bool {
        SCREENDUMPER.with(|obj_cell| {
            let obj = obj_cell.borrow();
            obj.sprites.is_some()
        })
    }

    fn _enable_raw(&self, enable: bool) {
        match &self.term {
            None => {}
            Some(term) => match enable {
                false => term.suspend_raw_mode().unwrap(),
                true => term.activate_raw_mode().unwrap(),
            },
        }
    }

    fn _dump(&mut self, board: &Board, triggered: &TriggeredCommands, message: &String) {
        if self.sprites.is_none() {
            return;
        }
        let (_, sprite_height) = self.sprite_size;
        // disable raw mode for printing
        self._enable_raw(false);

        let mesh = self.debug_build_mesh(&board, triggered);
        match mesh {
            None => {}
            Some(mesh) => {
                println!("\n{}\n", message);
                let board_size = board.size();
                let rows = match self.rect {
                    None => 0..board_size.height,
                    Some(rect) => rect.y..cmp::min(board_size.height, rect.y + rect.height),
                };

                for y in rows {
                    for row in 0..sprite_height {
                        let cols = match self.rect {
                            None => 0..board_size.width,
                            Some(rect) => rect.x..cmp::min(board_size.width, rect.x + rect.width),
                        };
                        for x in cols {
                            let pos = Position::new(x, y);
                            let uicell = mesh
                                .get(&pos)
                                .expect("Expected to find position in the mesh but did not");
                            uicell.print_row(row as usize);
                        }
                        println!(
                            "{}{}",
                            termion::color::Fg(termion::color::Reset),
                            termion::color::Bg(termion::color::Reset)
                        );
                    }
                }
            }
        }

        self._enable_raw(true);

        self.prev_board = Some(board.clone());
        self.prev_triggered = Some(triggered.clone());
    }

    fn debug_build_mesh(
        &mut self,
        board: &Board,
        triggered: &TriggeredCommands,
    ) -> Option<FnvHashMap<Position, UICell>> {
        let (sprite_width, sprite_height) = self.sprite_size;
        let mut mesh = FnvHashMap::default(); // Position -> UICell
        let did_trigger_change = match &self.prev_triggered {
            None => false,
            Some(t) => t == triggered,
        };
        let mut nothing_changed = did_trigger_change; // if triggered commands changed then the game definitely changed (even though the board didn't)
        match &self.sprites {
            None => unreachable!(),
            Some(sprite_lookup) => {
                for pos in board.positions_iter() {
                    let sprites = board.get_sprites_and_dir(&pos);

                    let is_same = match &self.prev_board {
                        None => false,
                        Some(prev) => {
                            // The level could have changed between debug sessions
                            if prev.width != board.width || prev.height != board.height {
                                false
                            } else {
                                let prev_sprites = prev.get_sprites_and_dir(&pos);
                                sprites == prev_sprites
                            }
                        }
                    };

                    // sprites.append(&mut game.background_tile.get_sprites().clone());

                    // draw the sprite temporarily. Then draw it for real (so fewer pixels actually changed)
                    let mut temp_pixels =
                        vec![Rgb::black(); (sprite_height * sprite_width) as usize];
                    let mut sprite_names = vec![];

                    for (sprite, w) in sprites {
                        let s = sprite_lookup.get(&sprite);
                        let sprite = s.unwrap();
                        if sprite.name.to_ascii_lowercase() != "background" {
                            sprite_names.push((sprite.name.clone(), w));
                        }

                        for sprite_y in 0..sprite_height {
                            for sprite_x in 0..sprite_width {
                                match &sprite.pixels[sprite_y as usize][sprite_x as usize] {
                                    None => { /*transparent*/ }
                                    Some(color) => {
                                        let color = if is_same {
                                            color.to_gray().darken()
                                        } else {
                                            nothing_changed = false;
                                            color.clone()
                                        };

                                        temp_pixels
                                            [(sprite_y * sprite_height + sprite_x) as usize] =
                                            color;
                                    }
                                }
                            }
                        }
                    }

                    // in order to show all the names (if there are more than 5)
                    // flip between sorted ascending & descending
                    if sprite_names.len() > 5 && (pos.x % 2 + pos.y % 2) % 2 == 0 {
                        sprite_names.reverse();
                    }

                    let uicell = UICell {
                        sprite_size: self.sprite_size,
                        pixels: temp_pixels,
                        sprites: sprite_names,
                    };
                    mesh.insert(pos.clone(), uicell);
                }
                if nothing_changed {
                    None
                } else {
                    Some(mesh)
                }
            }
        }
    }
}

struct UICell {
    sprite_size: (u16, u16),
    pixels: Vec<Rgb>,
    sprites: Vec<(String, WantsToMove)>,
}

impl UICell {
    fn to_pretty(&self, s: Option<&(String, WantsToMove)>) -> String {
        let (sprite_width, _) = self.sprite_size;
        match s {
            None => String::from(""),
            Some((name, WantsToMove::Stationary)) => {
                if name.len() > (sprite_width * 2) as usize {
                    let left_end = (sprite_width - 1) as usize;
                    let right_start = name.len() - sprite_width as usize;
                    let left = &name[..left_end];
                    let right = &name[right_start..];
                    let mut s = String::from(left);
                    s.push_str(".");
                    s.push_str(right);
                    s
                } else {
                    name.clone()
                }
            }
            Some((name, dir)) => {
                let mut s = String::from(match dir {
                    WantsToMove::Up => "▲",
                    WantsToMove::Down => "▼",
                    WantsToMove::Left => "◀",
                    WantsToMove::Right => "▶",
                    WantsToMove::Action => "*",
                    WantsToMove::Stationary => unreachable!(),
                    WantsToMove::RandomDir => unreachable!(),
                });
                if name.len() >= (sprite_width * 2) as usize {
                    let left_end = (sprite_width - 1) as usize;
                    let left = &name[..left_end];
                    let right = &name[left_end..];
                    s.push_str(left);
                    s.push_str(".");
                    s.push_str(right);
                } else {
                    s.push_str(name);
                }
                s
            }
        }
    }

    fn print_row(&self, row: usize) {
        let (sprite_width, sprite_height) = self.sprite_size;
        let cs = ColorSpace::get_colorspace();

        let offset = if self.sprites.len() > sprite_height as usize {
            self.sprites.len() - sprite_height as usize + row
        } else {
            row
        };
        let s = self.sprites.get(offset);
        let name = self.to_pretty(s);

        let chars: Vec<_> = name.chars().collect();

        let start = row * sprite_height as usize;
        for col in 0..sprite_width as usize {
            let pixel = self.pixels.get(start + col).unwrap();

            cs.print_bg_color(pixel.r, pixel.g, pixel.b);
            if pixel.is_dark() {
                cs.print_fg_color(255, 255, 255);
            } else {
                cs.print_fg_color(0, 0, 0);
            }
            print!(
                "{}{}",
                chars.get(col * 2).unwrap_or(&' '),
                chars.get(col * 2 + 1).unwrap_or(&' ')
            );
        }
    }
}
