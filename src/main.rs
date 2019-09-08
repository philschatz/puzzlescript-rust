extern crate env_logger;
extern crate hex;
extern crate nom;
extern crate rand;
extern crate rand_xorshift;
extern crate serde;
extern crate serde_json;
extern crate termion;
#[macro_use]
extern crate clap;

mod bitset;
mod cli;
mod color;
mod debugger;
mod engine;
mod json;
mod model;
mod parser;
mod save;
mod terminal;

use std::error::Error;
use std::str::from_utf8;
use std::str::FromStr;
// use std::num::ParseIntError;
// use std::num::IntErrorKind;
use crate::json::Dimension;

use nom::{
    alt,
    bytes::complete::{tag, take_while_m_n},
    character::is_alphabetic,
    character::is_digit,
    character::complete::alpha1,
    character::complete::digit1,
    character::complete::newline,
    character::complete::not_line_ending,
    // character::complete::space,
    character::complete::space0,
    character::complete::space1,
    take_while,
    do_parse,
    map_res,
    many0,
    map,
    many1,
    opt,
    named,
    parse_to,
    recognize,
    //   combinator::map_res,
    sequence::tuple,
    tag_no_case,
    take_while1,
    tuple,
    ws,
    IResult,
};

fn main() /*-> Result<(), Box<dyn Error>>*/
{
    println!("{:?}", hex_color("#2F14DF"));
    // cli::main()
}

#[derive(Debug, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(input, 16)
}

fn is_hex_digit(c: char) -> bool {
    c.is_digit(16)
}

fn hex_primary(input: &str) -> IResult<&str, u8> {
    nom::combinator::map_res(take_while_m_n(2, 2, is_hex_digit), from_hex)(input)
}

fn hex_color(input: &str) -> IResult<&str, Color> {
    let (input, _) = tag("#")(input)?;
    let (input, (red, green, blue)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;

    Ok((input, Color { red, green, blue }))
}

// named!(my_u64(&str) -> u64,
//     map_res!(recognize!(nom::digit), u64::from_str)
// );

fn parser(input: &str) -> IResult<&str, &str> {
    digit1(input)
}

named!(integer_bytes, take_while1!(is_digit));
named!(integer_str, take_while1!(is_digit));

named!(to_u64(&[u8]) -> u64, parse_to!(u64));

fn parse_u16(digits: &str) -> u16 {
    match digits.parse() {
        Ok(v) => v,
        Err(_) => panic!("BUG. Could not parse '{}'", digits),
    }
}

// named!(parse_to_u16(&str) -> u16,
//     do_parse!(
//         digits: digit1 >>
//         (parse_u16(digits))
//     )
// );


named!(parse_to_u16(&str) -> u16,
    // map!(digit1, |s| s.parse().unwrap())
    map_res!(digit1, |s: &str| s.parse())
);

named!(x<&str, MetadataKey>, parse_to!(MetadataKey));

named!(parse_metadata_key<&str, MetadataKey>,
    map_res!(alpha1, |s: &str| s.parse())
);

// named!(parse_metadata_key<&str, MetadataKey>,
//   parse_to!(MetadataKey)
// );

named!(parse_words,
//   do_parse!(
//     words: take_while1!(not_line_ending) >>
//     ("words")
//   )
  recognize!(many1!(not_line_ending))
);

// named!(parse_decimal<&str, f32>, // nom::InputLength
//     parse_to!(f32)
// );

named!(parse_dimension<&str, Dimension>, // nom::InputLength
    do_parse!(
        width: parse_to_u16 >>
        tag_no_case!("x") >>
        height: parse_to_u16 >>
        (Dimension { width, height })
    )
);

#[derive(PartialEq, Debug)]
enum MetadataKey {
    author,
    homepage,
    youtube,
    zoomscreen,
    flickscreen,
    color_palette,
    background_color,
    text_color,
    realtime_interval,
    key_repeat_interval,
    again_interval,
    no_action,
    no_undo,
    run_rules_on_level_start,
    no_repeat_action,
    throttle_movement,
    no_restart,
    require_player_movement,
    verbose_logging,
}

#[derive(Debug)]
struct UnknownMetadataKey {}

impl FromStr for MetadataKey {
    type Err = UnknownMetadataKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = s.to_lowercase();
        return if key == "author" {
            Ok(Self::author)
        } else if key == "homepage" {
            Ok(Self::homepage)
        } else if key == "youtube" {
            Ok(Self::youtube)
        } else if key == "zoomscreen" {
            Ok(Self::zoomscreen)
        } else if key == "flickscreen" {
            Ok(Self::flickscreen)
        } else if key == "color_palette" {
            Ok(Self::color_palette)
        } else if key == "background_color" {
            Ok(Self::background_color)
        } else if key == "text_color" {
            Ok(Self::text_color)
        } else if key == "realtime_interval" {
            Ok(Self::realtime_interval)
        } else if key == "key_repeat_interval" {
            Ok(Self::key_repeat_interval)
        } else if key == "again_interval" {
            Ok(Self::again_interval)
        } else if key == "no_action" {
            Ok(Self::no_action)
        } else if key == "no_undo" {
            Ok(Self::no_undo)
        } else if key == "run_rules_on_level_start" {
            Ok(Self::run_rules_on_level_start)
        } else if key == "no_repeat_action" {
            Ok(Self::no_repeat_action)
        } else if key == "throttle_movement" {
            Ok(Self::throttle_movement)
        } else if key == "no_restart" {
            Ok(Self::no_restart)
        } else if key == "require_player_movement" {
            Ok(Self::require_player_movement)
        } else if key == "verbose_logging" {
            Ok(Self::verbose_logging)
        } else {
            Result::Err(UnknownMetadataKey {})
        };
    }
}

#[derive(PartialEq, Debug)]
enum MetadataValue {
    Word(String),
    Words(String),
    ColorNameOrHex(String),
    Decimal(f32),
    False,
    True,
    Dim(Dimension),
}


named!(parse_metadata_dimension<&str, MetadataValue>,
  do_parse!(
    dim: parse_dimension >>
    (MetadataValue::Dim(dim))
  )
);

named!(parse_metadata_true<&str, MetadataValue>,
  do_parse!(
    tag_no_case!("true") >>
    (MetadataValue::True)
  )
);

named!(parse_metadata_off<&str, MetadataValue>,
  do_parse!(
    tag_no_case!("off") >>
    (MetadataValue::False)
  )
);

named!(parse_metadata_words<&str, MetadataValue>,
    map!(alpha1, |s: &str| MetadataValue::Words(String::from(s)))
);

// named!(parse_metadata_decimal<&str, MetadataValue>,
//   do_parse!(
//     decimal: parse_to!(f32) >>
//     (MetadataValue::Decimal(decimal))
//   )
// );

named!(parse_metadata_value<&str, MetadataValue>,
  alt!(parse_metadata_off | parse_metadata_true /* | parse_metadata_decimal */ | parse_metadata_words)
);

named!(parse_metadata_item_value<&str, (MetadataKey, Option<MetadataValue>)>,
  do_parse!(
       key: parse_metadata_key 
    >>      space1 
    >> val: opt!(parse_metadata_value)
    >>      newline
    >>      (key, val)
  )
);

named!(parse_metadata_item_novalue<&str, (MetadataKey, Option<MetadataValue>)>,
  do_parse!(
       key: parse_metadata_key
    >>      space0
    >>      newline 
    >>      (key, None)
  )
);

named!(parse_metadata_item<&str, (MetadataKey, Option<MetadataValue>)>,
  alt!(parse_metadata_item_value | parse_metadata_item_novalue)
);

named!(parse_metadata<&str, Vec<(MetadataKey, Option<MetadataValue>)>>,
    ws!(many0!(parse_metadata_item))
);


#[test]
fn parse_color() {
    assert_eq!(
        hex_color("#2F14DF"),
        Ok((
            "",
            Color {
                red: 47,
                green: 20,
                blue: 223,
            }
        ))
    );
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn test_dimension() {
        assert_eq!(
            parse_dimension("12x23"),
            Ok((
                "",
                Dimension {
                    width: 12,
                    height: 23
                }
            ))
        );
    }

    #[test]
    fn test_empty_metadata() {
        let (rest, m) = parse_metadata("").unwrap();
        assert_eq!(rest, "");
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn test_author_key() {
        let src = "author";
        let (rest, m) = parse_metadata_key(src).unwrap();
        assert_eq!(rest, "");
        assert_eq!(m, MetadataKey::author);
    }

    #[test]
    fn test_author_key_extratext() {
        let src = "author \n";
        let (rest, m) = parse_metadata_key(src).unwrap();
        assert_eq!(rest, " \n");
        assert_eq!(m, MetadataKey::author);
    }

    #[test]
    fn test_author_item() {
        let src = "author\n";
        let i = parse_metadata_item(src);
        println!("{:?}", i);
        let (rest, m) = i.unwrap();
        assert_eq!(rest, "");
        println!("{:?}", m);
        assert_eq!(m, (MetadataKey::author, None));
    }

    #[test]
    fn test_author_item_trailing_whitespace() {
        let src = "author \n";
        let i = parse_metadata_item(src);
        println!("{:?}", i);
        let (rest, m) = i.unwrap();
        assert_eq!(rest, "");
        println!("{:?}", m);
        assert_eq!(m, (MetadataKey::author, None));
    }

    #[test]
    fn test_author_one_word() {
        let src = "author jim\n";
        let (rest, m) = parse_metadata(src).unwrap();
        assert_eq!(rest, "");
        println!("{:?}", m);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0], (MetadataKey::author, Some(MetadataValue::Words(String::from("jim")))));
    }

}