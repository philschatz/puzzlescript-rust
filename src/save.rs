use serde::{Deserialize, Serialize};

use std::error::Error;
use std::fs::File;
use std::io::BufReader;

use log::debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct SaveState {
    pub version: u8,
    pub inputs: Vec<String>,
    pub level: u8,
    pub checkpoint: Option<Vec<Vec<String>>>,
}

impl SaveState {
    pub fn read_from_file(path: &str) -> Result<Self, Box<dyn Error>> {
        debug!("Reading {:?}", path);

        // Open the file in read-only mode with buffer.
        let f = File::open(path)?;
        let r = BufReader::new(f);
        Ok(serde_json::from_reader(r).unwrap())
    }

    pub fn write_to_file(&self, path: &str) -> Result<(), Box<dyn Error>> {
        debug!("Writing {:?}", path);
        let f = File::create(path)?;
        Ok(serde_json::to_writer(f, &self).unwrap())
    }
}
