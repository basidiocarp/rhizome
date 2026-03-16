/// A sample struct for testing
#[derive(Debug)]
pub struct Config {
    pub name: String,
    pub value: i32,
}

impl Config {
    /// Creates a new Config
    pub fn new(name: String, value: i32) -> Self {
        Config { name, value }
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}

/// A standalone function
pub fn process(config: &Config) -> String {
    format!("{}: {}", config.name, config.value)
}

const MAX_SIZE: usize = 1024;

use std::collections::HashMap;
use std::path::PathBuf;

pub enum Status {
    Active,
    Inactive,
}

pub trait Processor {
    fn process(&self) -> Result<(), String>;
}
