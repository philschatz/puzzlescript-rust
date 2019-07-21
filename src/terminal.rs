use std::cmp;
use std::format;
use std::time;

use log::trace;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::Color;
use tui::style::Modifier;
use tui::style::Style;
use tui::widgets::Block;
use tui::widgets::Borders;
use tui::widgets::Widget;

use crate::color::ColorSpace;
use crate::color::Rgb;
use crate::debugger::ScreenDumper;
use crate::engine::BoardOrMessage;
use crate::engine::Engine;
use crate::model::util::Position;

// Temporary grid of pixels. This is used to render the
// level using '  ' or '▄' depending on the size of the terminal
struct Grid {
    is_true_color: bool,
    width: u16,
    height: u16,
    colors: Vec<Rgb>,
    background: Rgb,
}

impl Grid {
    fn new(width: u16, height: u16, background: Rgb) -> Self {
        Self {
            is_true_color: ColorSpace::get_colorspace().is_true_color(),
            width,
            height,
            colors: vec![background; width as usize * height as usize],
            background,
        }
    }

    fn set(&mut self, x: u16, y: u16, color: Rgb) {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height);

        let i = x as usize + y as usize * self.width as usize;
        // Alpha Transparency Support only for truecolor screens
        if color.a != 0 {
            if self.is_true_color {
                self.colors[i] = color.on_top_of(&self.colors[i]);
            } else {
                // Skip the decal since we are color-constrained (entanglement-two)
                self.colors[i] = color.on_top_of(&self.colors[i]);
            }
        } else {
            self.colors[i] = color;
        }
    }

    fn rendered_rect(&self, area: &Rect) -> Rect {
        if self.width * 2 > area.width || self.height > area.height {
            Rect::new(area.x, area.y, self.width, self.height / 2)
        } else {
            Rect::new(area.x, area.y, self.width * 2, self.height)
        }
    }

    fn render(&self, area: &Rect, buf: &mut Buffer) {
        if self.width * 2 > area.width || self.height > area.height {
            self.render_small(area, buf);
        } else {
            self.render_large(area, buf);
        }
    }

    fn render_small(&self, area: &Rect, buf: &mut Buffer) {
        for row in 0..(self.height + 1) / 2 { // add one in case of odd--numbered rows
            for x in 0..self.width {
                let y_top = row * 2;
                let y_bottom = row * 2 + 1;
                let top_color = self.colors[(x + y_top * self.width) as usize];
                let bottom_color = self
                    .colors
                    .get((x + y_bottom * self.width) as usize)
                    .unwrap_or(&self.background);

                if x >= area.width || row >= area.height {
                    continue;
                }

                let x = x + area.left();
                let y = row + area.top();

                buf.get_mut(x, y)
                    .set_bg(self.to_color(&top_color))
                    .set_fg(self.to_color(bottom_color))
                    .set_char('▄');
            }
        }
    }

    fn render_large(&self, area: &Rect, buf: &mut Buffer) {
        for y in 0..self.height {
            for x in 0..self.width {
                let color = self.colors[(x + y * self.width) as usize];

                if x * 2 >= area.width || y >= area.height {
                    continue;
                }

                let x = x * 2 + area.left();
                let y = y + area.top();

                buf.get_mut(x, y)
                    .set_bg(self.to_color(&color))
                    .set_char(' ');
                buf.get_mut(x + 1, y)
                    .set_bg(self.to_color(&color))
                    .set_char(' ');
            }
        }
    }

    fn to_color(&self, rgb: &Rgb) -> Color {
        if self.is_true_color {
            Color::Rgb(rgb.r, rgb.g, rgb.b)
        } else {
            // See termion::AnsiValue::rgb(r,g,b)
            let r = rgb.r / 51;
            let g = rgb.g / 51;
            let b = rgb.b / 51;
            Color::Indexed(16 + 36 * r + 6 * g + b)
        }
    }
}

impl Widget for Engine {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        if let Some(msg) = &self.pending_message {
            MessageWindow::new(msg.clone()).draw(area, buf);
            return;
        }

        match &self.current_level {
            BoardOrMessage::Message(msg) => MessageWindow::new(msg.clone()).draw(area, buf),
            BoardOrMessage::Board(board) => {
                let (sprite_width, sprite_height) = self.game_data.sprite_size();
                let board_size = board.size();
                let is_flickscreen = self.game_data.metadata.flickscreen.is_some();
                let screen_size = self.game_data.metadata.flickscreen.or(self.game_data.metadata.zoomscreen);
                let game_window = match screen_size {
                    None => Rect::new(0, 0, board_size.width, board_size.height),
                    Some(flick) => {
                        let width = cmp::min(flick.width, board_size.width); // see atlas-shrank
                        let height = cmp::min(flick.height, board_size.height);
                        let player = self.player_position().unwrap();
                        if is_flickscreen {
                            Rect::new(
                                player.x / width * width,
                                player.y / height * height,
                                width,
                                height,
                            )
                        } else {
                            // zoomscreen
                            Rect::new(
                                if player.x >= width / 2 { player.x - width / 2 } else { 0 },
                                if player.y >= height / 2 { player.y - height / 2 } else { 0 },
                                width,
                                height,
                            )
                        }
                    }
                };

                trace!(
                    "Board: {:?}, Is flickscreen? {:?}",
                    board_size,
                    self.game_data.metadata.flickscreen
                );
                ScreenDumper::set_window(game_window);

                let mut grid = Grid::new(
                    game_window.width * sprite_width,
                    game_window.height * sprite_height,
                    self.game_data
                        .metadata
                        .background_color
                        .unwrap_or(Rgb::black()),
                );

                let is_visible = |pos: &Position| {
                    pos.x >= game_window.left()
                        && pos.y >= game_window.top()
                        && pos.x < game_window.right()
                        && pos.y < game_window.bottom()
                };

                for cell_pos in board.positions_iter() {
                    if !is_visible(&cell_pos) {
                        continue;
                    }

                    let mut sprites = board.get_sprite_states(&cell_pos);
                    sprites.sort();

                    for sprite in sprites {
                        let sprite = self.game_data.lookup_sprite(sprite);

                        for sprite_y in 0..sprite_height {
                            for sprite_x in 0..sprite_width {
                                if let Some(rgb) =
                                    sprite.pixels[sprite_y as usize][sprite_x as usize]
                                {
                                    let x = sprite_x as u16 + cell_pos.x * sprite_width;
                                    let y = sprite_y as u16 + cell_pos.y * sprite_height;

                                    // shift for flickscreen games
                                    let x = x - game_window.left() * sprite_width;
                                    let y = y - game_window.top() * sprite_height;

                                    grid.set(x, y, rgb);
                                }
                            }
                        }
                    }
                }

                grid.render(&area, buf);

                // add ellipses if the window is to short or narrow to show the whole level
                let grid_rect = grid.rendered_rect(&area);

                if grid_rect.width > area.width {
                    let x = area.right() - 1;
                    for y in grid_rect.top()..cmp::min(grid_rect.bottom(), area.bottom()) {
                        buf.get_mut(x, y)
                            .set_bg(Color::Black)
                            .set_fg(Color::White)
                            .set_char(':');
                    }
                }
                if grid_rect.height > area.height {
                    let y = area.bottom() - 1;
                    let right = cmp::min(grid_rect.right(), area.right());
                    for x in grid_rect.left()..right {
                        buf.get_mut(x, y)
                            .set_bg(Color::Black)
                            .set_fg(Color::White)
                            .set_char('.');
                    }
                    // add the "resize" text
                    let resize = "(resize terminal)";
                    let x =
                        grid_rect.left() + (right - grid_rect.left()) / 2 - resize.len() as u16 / 2;
                    buf.set_string(x, y, resize, Style::default().fg(Color::LightYellow))
                }
            }
        }
    }
}

pub struct MessageWindow {
    message: String,
}

impl MessageWindow {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl Widget for MessageWindow {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![String::from("")];
        let mut index = 0;
        for word in self.message.split_whitespace() {
            if index > 33 {
                lines.push(String::from(""));
                index = 0;
            }
            let line = lines.last_mut().unwrap();
            line.push_str(" ");
            line.push_str(word);
            index += word.len();
        }

        let (width, height) = termion::terminal_size().unwrap_or((80, 25));
        let mut y = (height - lines.len() as u16) / 2;
        for line in lines {
            let x = (width - line.len() as u16) / 2;

            buf.set_string(
                x as u16 + area.left(),
                y as u16 + area.top(),
                line,
                Style::default().fg(Color::White),
            );

            y += 1;
        }
    }
}

pub const DOTS: [char; 12] = [
    ' ', '⠁', '⠉', '⠙', '⠹', '⠽', '⠿', '⠽', '⠹', '⠙', '⠉', '⠁',
];

// show this so that the cursor _always_ ends up at the bottom of the screen
pub struct Spinner {
    state: usize,
    last_tick: time::Instant,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            state: 0,
            last_tick: time::Instant::now(),
        }
    }
}

impl Widget for Spinner {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        debug_assert!(area.height > 0, "Needs at least 1 line to show the spinner");

        let s = format!(
            "{:.1}FPS",
            1000 as f32 / self.last_tick.elapsed().as_millis() as f32
        );
        buf.set_string(
            area.right() - s.len() as u16 - 2,
            area.bottom() - 1,
            s,
            Style::default(),
        );
        self.last_tick = time::Instant::now();

        self.state = (self.state + 1) % DOTS.len();
        buf.get_mut(area.right() - 1, area.bottom() - 1)
            .set_char(DOTS[self.state]);
    }
}

pub struct Help {
    pub expanded: bool,
}

impl Help {
    pub fn new() -> Self {
        Self { expanded: false }
    }
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }
}

impl Widget for Help {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        if self.expanded {
            buf.set_string(area.x, area.y, "Move: Arrows/WSAD | Action: X/Space | Undo: Z/U | Restart: R | Quit: Q/Esc | Pause: P | Debugger: ` or ~ or \\ | Fast/Slow: - or +", Style::default())
        } else {
            buf.set_string(
                area.x,
                area.y,
                "[?] for help",
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}

pub struct Attribution {
    title: String,
    author: Option<String>,
    homepage: Option<String>,
}

impl Attribution {
    pub fn new(title: String, author: Option<String>, homepage: Option<String>) -> Self {
        Self {
            title,
            author,
            homepage,
        }
    }
}

impl Widget for Attribution {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        assert!(area.height > 0, "Needs at least 1 line to add attribution");

        let mut s = self.title.clone();
        if let Some(author) = &self.author {
            s.push_str(&format!(" by {}", author));
        }
        if let Some(homepage) = &self.homepage {
            s.push_str(&format!(" at {}", homepage));
        }

        buf.set_string(area.x, area.y, s, Style::default());
    }
}

// show this so that the cursor _always_ ends up at the bottom of the screen
pub struct PlayPause {
    pub paused: bool,
}

impl PlayPause {
    pub fn new() -> Self {
        Self { paused: false }
    }

    pub fn resume(&mut self) {
        self.paused = false
    }

    pub fn pause(&mut self) {
        self.paused = true
    }
}

impl Widget for PlayPause {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        debug_assert!(
            area.height > 3,
            "Needs at least 3 lines to show the pause dialog"
        );

        if self.paused {
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    let cell = buf.get_mut(x, y);
                    cell.set_bg(to_grayscale(&cell.style.bg));
                    cell.set_fg(to_grayscale(&cell.style.fg));
                }
            }

            let s = "PAUSED";
            let width = 12;
            let height = 5;
            let x = area.left() + (area.width - width) / 2;
            let y = area.top() + area.height / 2 - height / 2;
            let dialog_area = Rect::new(x, y, width, height);
            let dialog_bg = Color::Black;

            // Fill the block with black (and 1char around the block)
            for y in dialog_area.top() - 1..dialog_area.bottom() + 1 {
                for x in dialog_area.left() - 1..dialog_area.right() + 1 {
                    buf.get_mut(x, y).set_bg(dialog_bg).set_char(' ');
                }
            }

            Block::default()
                .title_style(Style::default().fg(Color::Red))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::LightYellow))
                .style(Style::default().bg(dialog_bg))
                .draw(dialog_area, buf);

            buf.set_string(
                x + 3,
                y + 2,
                s,
                Style::default()
                    .fg(Color::LightRed)
                    .modifier(Modifier::BOLD)
                    .modifier(Modifier::SLOW_BLINK),
            );
        }
    }
}

fn to_grayscale(color: &Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let gray = ((*r as u16 + *g as u16 + *b as u16) / 3) as u8;
            Color::Rgb(gray, gray, gray)
        }
        Color::Indexed(i) => {
            let i = i - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = (i % 36) % 6;
            let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;

            // Color::Indexed(16 + 36 * r + 6 * g + b)
            Color::Indexed(16 + 36 * gray + 6 * gray + gray)
        }
        _ => color.clone(),
    }
}

#[derive(Default)]
pub struct RecordingInfo {
    checkpoints: u16
}

impl RecordingInfo {
    pub fn increment(&mut self) {
        self.checkpoints += 1;
    }
}

impl Widget for RecordingInfo {
    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let s = format!("Checkpoints Reached: {}", self.checkpoints);
        buf.set_string(area.x, area.y, s, Style::default());
    }
}