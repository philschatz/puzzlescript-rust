extern crate env_logger;
extern crate hex;
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

fn main() -> Result<(), Box<dyn Error>> {
    cli::main()
}
