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
    // TODO: Add validation for empty names
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

// FIXME: This function has a known bug with negative values
fn internal_helper(x: i32) -> i32 {
    x + 1
}

/// Complex function for testing cyclomatic complexity
pub fn complex_logic(x: i32, y: i32) -> String {
    if x > 0 {
        if y > 0 {
            "both positive".to_string()
        } else if y == 0 {
            "x positive, y zero".to_string()
        } else {
            "x positive, y negative".to_string()
        }
    } else if x == 0 {
        match y {
            0 => "both zero".to_string(),
            _ => "x zero".to_string(),
        }
    } else {
        for i in 0..x.abs() {
            if i > 10 || y < -100 {
                return "early exit".to_string();
            }
        }
        "x negative".to_string()
    }
}

/// A type alias for convenience
pub type ConfigMap = HashMap<String, Config>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let cfg = Config::new("test".to_string(), 42);
        assert_eq!(cfg.name, "test");
    }

    #[test]
    fn test_process() {
        let cfg = Config::new("key".to_string(), 10);
        assert_eq!(process(&cfg), "key: 10");
    }
}
