use fnv::FnvHashMap;
use std::fmt;

use hex;

#[derive(Copy, Clone, PartialEq, Hash, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

fn abs(a: u8, b: u8) -> u8 {
    if a > b {
        a - b
    } else {
        b - a
    }
}

impl Rgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b, a: 0 }
    }
    pub fn is_dark(&self) -> bool {
        (self.r as u16 + self.g as u16 + self.b as u16) < 128 * 3
    }

    pub fn to_gray(&self) -> Rgb {
        let gray = ((self.r as u16 + self.g as u16 + self.b as u16) / 3) as u8;
        Rgb {
            r: gray,
            g: gray,
            b: gray,
            a: self.a,
        }
    }

    pub fn darken(&self) -> Rgb {
        Rgb {
            r: self.r / 2,
            g: self.g / 2,
            b: self.b / 2,
            a: self.a,
        }
    }

    pub fn parse(hex: &String) -> Rgb {
        let bytes = hex::decode(&hex[1..]).unwrap(); // Skip the "#"
        let r = bytes[0];
        let g = bytes[1];
        let b = bytes[2];
        let a = if bytes.len() > 3 { bytes[3] } else { 0 };
        Rgb { r, g, b, a }
    }

    pub fn distance(&self, other: &Rgb) -> u8 {
        abs(self.r, other.r) + abs(self.g, other.g) + abs(self.b, self.b)
    }

    // Some terminals only support 256 total colors.
    // We need each pixel to be a different color to show texture.
    // So fiddle with the color variants until an unused color
    // variant is found.
    pub fn to_variant(&self, map: &mut FnvHashMap<Rgb256, Rgb>) -> Rgb {
        let flattened = self.to_256();
        match map.get(&flattened) {
            None => {
                map.insert(flattened.clone(), self.clone());
                flattened.from_256()
            }
            Some(entry) => {
                if entry == self {
                    flattened.from_256()
                } else {
                    self.find_a_variant(map)
                }
            }
        }
    }

    pub fn to_closest_256(&self) -> Rgb {
        self.to_256().from_256()
    }

    fn to_256(&self) -> Rgb256 {
        Rgb256::new(self.r / 51, self.g / 51, self.b / 51, self.a)
    }

    fn find_a_variant(&self, map: &mut FnvHashMap<Rgb256, Rgb>) -> Rgb {
        for variant in self.to_256().to_variants() {
            match map.get(&variant) {
                None => {
                    map.insert(variant, self.clone());
                    return variant.from_256();
                }
                Some(orig) => {
                    if orig == self {
                        return variant.from_256();
                    }
                }
            }
        }
        eprintln!(
            "Could not find a variant for {}. Using original color & hoping for the best",
            self
        );
        self.clone()
    }

    pub fn on_top_of(&self, under: &Rgb) -> Rgb {
        assert!(self.a != 0);

        let a = self.a as f32 / 255 as f32;
        let r = (a * self.r as f32 + (1.0 - a) * under.r as f32) as u8;
        let g = (a * self.g as f32 + (1.0 - a) * under.g as f32) as u8;
        let b = (a * self.b as f32 + (1.0 - a) * under.b as f32) as u8;
        Rgb::new(r, g, b)
    }

    pub fn black() -> Self {
        Rgb::new(0, 0, 0)
    }
}

impl Eq for Rgb {}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut v = vec![self.r, self.g, self.b];
        if self.a > 0 {
            v.push(self.a);
        }
        let hex_str = hex::encode(v);
        write!(f, "#{}", hex_str)
    }
}

#[derive(Copy, Clone, PartialEq, Hash, Debug)]
pub struct Rgb256 {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgb256 {
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        assert!(r <= 5);
        assert!(g <= 5);
        assert!(b <= 5);
        Self { r, g, b, a }
    }

    fn from_256(&self) -> Rgb {
        Rgb {
            r: self.r * 51,
            g: self.g * 51,
            b: self.b * 51,
            a: self.a,
        }
    }

    // Tweak the r, g, b slightly until an unused color is found
    fn to_variants(&self) -> Vec<Rgb256> {
        let mut ret = vec![];
        // first try slightly lighter and then darker
        if self.r < 5 && self.g < 5 && self.b < 5 {
            ret.push(Self::new(self.r + 1, self.g + 1, self.b + 1, self.a));
        }
        if self.r > 0 && self.g > 0 && self.b > 0 {
            ret.push(Self::new(self.r - 1, self.g - 1, self.b - 1, self.a));
        }

        for diff in 0..3 {
            if self.r + diff <= 5 {
                ret.push(Self::new(self.r + diff, self.g, self.b, self.a));
            }
            if self.r > diff {
                ret.push(Self::new(self.r - diff, self.g, self.b, self.a));
            }
            if self.g + diff <= 5 {
                ret.push(Self::new(self.r, self.g + diff, self.b, self.a));
            }
            if self.g > diff {
                ret.push(Self::new(self.r, self.g - diff, self.b, self.a));
            }
            if self.b + diff <= 5 {
                ret.push(Self::new(self.r, self.g, self.b + diff, self.a));
            }
            if self.b > diff {
                ret.push(Self::new(self.r, self.g, self.b - diff, self.a));
            }
        }
        ret
    }
}

impl Eq for Rgb256 {}

#[derive(Debug)]
pub enum ColorSpace {
    Unknown,
    TwoFiftySix,
    TrueColor,
}

impl ColorSpace {
    pub fn get_colorspace() -> Self {
        if env_contains("COLORTERM", "truecolor") {
            ColorSpace::TrueColor
        } else if env_contains("TERM", "256color") {
            ColorSpace::TwoFiftySix
        } else {
            ColorSpace::Unknown
        }
    }

    pub fn is_true_color(&self) -> bool {
        match self {
            ColorSpace::TrueColor => true,
            _ => false,
        }
    }

    pub fn print_bg_color(&self, r: u8, g: u8, b: u8) {
        match self {
            ColorSpace::TrueColor => print!("{}", termion::color::Bg(termion::color::Rgb(r, g, b))),
            ColorSpace::TwoFiftySix => print!(
                "{}",
                termion::color::Bg(termion::color::AnsiValue::rgb(r / 51, g / 51, b / 51))
            ),
            ColorSpace::Unknown => print!(
                "{}",
                termion::color::Bg(termion::color::AnsiValue::rgb(r / 51, g / 51, b / 51))
            ),
        };
    }

    pub fn print_fg_color(&self, r: u8, g: u8, b: u8) {
        match self {
            ColorSpace::TrueColor => print!("{}", termion::color::Fg(termion::color::Rgb(r, g, b))),
            ColorSpace::TwoFiftySix => print!(
                "{}",
                termion::color::Fg(termion::color::AnsiValue::rgb(r / 51, g / 51, b / 51))
            ),
            ColorSpace::Unknown => print!(
                "{}",
                termion::color::Fg(termion::color::AnsiValue::rgb(r / 51, g / 51, b / 51))
            ),
        };
    }
}

fn env_contains(key: &str, contains: &str) -> bool {
    match std::env::var_os(key) {
        None => false,
        Some(val) => val.to_str().unwrap_or("").contains(contains),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant() {
        let black = Rgb::new(0, 0, 0);
        let mut map = FnvHashMap::default();
        assert_eq!(black.to_variant(&mut map), black);

        // pick a lighter gray first
        let dark = Rgb::new(1, 1, 1);
        assert_eq!(dark.to_variant(&mut map), Rgb::new(51, 51, 51));

        // then a lighter red
        let gray = Rgb::new(2, 2, 2);
        assert_eq!(gray.to_variant(&mut map), Rgb::new(51, 0, 0));

        // then a lighter green
        let gray = Rgb::new(3, 3, 3);
        assert_eq!(gray.to_variant(&mut map), Rgb::new(0, 51, 0));
    }

    #[test]
    fn alpha() {
        let mut map = FnvHashMap::default();
        let zero = Rgb {
            r: 0,
            g: 0,
            b: 0,
            a: 128,
        };
        let one = Rgb {
            r: 1,
            g: 1,
            b: 1,
            a: 128,
        };
        let two = Rgb {
            r: 2,
            g: 2,
            b: 2,
            a: 128,
        };
        assert_eq!(zero.to_closest_256().a, 128);
        assert_eq!(zero.to_closest_256().to_variant(&mut map).a, 128);

        assert_eq!(one.to_closest_256().a, 128);
        assert_eq!(one.to_closest_256().to_variant(&mut map).a, 128);

        assert_eq!(two.to_closest_256().a, 128);
        assert_eq!(two.to_closest_256().to_variant(&mut map).a, 128);
    }

    #[test]
    fn on_top_of() {
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
            a: 127,
        };
        let gray = Rgb::new(127, 127, 127);

        assert_eq!(white.on_top_of(&Rgb::black()), gray);
    }

}
