use serde::{Serialize, Deserialize};

use std::error::Error;
use std::io::BufReader;
use std::fs::File;

use log::debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct SaveState {
    pub version: u8,
    pub inputs: Vec<String>,
    pub level: u8,
    pub checkpoint: Option<Vec<Vec<String>>>,
}

impl SaveState {
    pub fn read_from_file(path: &str) -> Result<Self, Box<Error>> {
        debug!("Reading {:?}", path);

        // Open the file in read-only mode with buffer.
        let f = File::open(path)?;
        let r = BufReader::new(f);
        Ok(serde_json::from_reader(r).unwrap())
    }

    pub fn write_to_file(&self, path: &str) -> Result<(), Box<Error>> {
        debug!("Writing {:?}", path);
        let f = File::create(path)?;
        Ok(serde_json::to_writer(f, &self).unwrap())
    }
}