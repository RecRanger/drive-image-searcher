use crate::parse_hex_string::parse_hex_string;

use serde::{self, Deserialize, Deserializer, Serialize};

use std::fs::File;
use std::io::Read;

use std::str::FromStr;

const DEFAULT_BYTE_COUNT_BEFORE_MATCH: u64 = 1024;
const DEFAULT_BYTE_COUNT_AFTER_MATCH: u64 = 1024;

#[derive(Clone)]
pub struct Needle {
    pub name: String,
    pub val: Vec<u8>,
    pub description_notes: String,
    /// significance level from 0-9, where 9 is "very happy"
    pub happiness_level: u8,
    pub write_to_file: bool,
    pub byte_count_before_match: u64,
    pub byte_count_after_match: u64,
    // TODO: add more options to search both endians, etc.
    // TODO: add option for 'shortest substring to match' to search for chunks within each needle
    // TODO: add "ignore if other one is found" option to ignore substrings of other searches
}

impl Needle {
    pub fn from_needle_val_config(config_needle_val: &ConfigNeedle) -> Self {
        let val = match config_needle_val.val_format {
            ConfigNeedleValFormat::Hex => {
                // The string is like "48656c6c6f", or "72 65 6c 6c 6f", or "0x72 0x65 0x6c 0x6c 0x6f".
                // We must parse it from these values.

                match parse_hex_string(config_needle_val.val.as_str()) {
                    Ok(val) => val,
                    Err(_) => {
                        panic!("Failed to parse hex string: {}", config_needle_val.val);
                    }
                }
            }
            ConfigNeedleValFormat::Ascii => {
                // convert the string to bytes as you'd do normally
                config_needle_val.val.as_bytes().to_vec()
            }
        };
        Self {
            name: config_needle_val.name.clone(),
            val,
            description_notes: config_needle_val.description_notes.clone(),
            happiness_level: config_needle_val.happiness_level,
            write_to_file: config_needle_val.write_to_file,
            byte_count_before_match: DEFAULT_BYTE_COUNT_BEFORE_MATCH,
            byte_count_after_match: DEFAULT_BYTE_COUNT_AFTER_MATCH,
        }
    }

    pub fn is_val_printable(&self) -> bool {
        self.val.iter().all(|b| b.is_ascii_graphic())
    }

    pub fn val_as_string(&self) -> String {
        match self.is_val_printable() {
            true => {
                format!("{:?} ('{}')", self.val, String::from_utf8_lossy(&self.val))
            }
            false => format!("{:?}", self.val),
        }
    }

    pub fn happiness_level_as_string(&self) -> String {
        let emojis = "😶😐🙂🙃😊😁😄😃😆😂";
        let emoji = emojis.chars().nth(self.happiness_level as usize).unwrap();
        format!("{}{} ({})", emoji, emoji, self.happiness_level)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConfigNeedle {
    pub name: String,
    pub val: String,
    pub val_format: ConfigNeedleValFormat,
    pub description_notes: String,
    pub happiness_level: u8,

    #[serde(default = "default_write_to_file")]
    pub write_to_file: bool,
}

fn default_write_to_file() -> bool {
    true
}

#[derive(Serialize, Debug)]
pub enum ConfigNeedleValFormat {
    Hex,
    Ascii,
}

impl FromStr for ConfigNeedleValFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "hex" => Ok(ConfigNeedleValFormat::Hex),
            "ascii" => Ok(ConfigNeedleValFormat::Ascii),
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for ConfigNeedleValFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match Self::from_str(&s) {
            Ok(config_need_val_format) => Ok(config_need_val_format),
            Err(_) => Err(serde::de::Error::custom("unknown format")),
        }
    }
}

fn load_config_needles_from_file(file_path: &str) -> Result<Vec<ConfigNeedle>, serde_yaml::Error> {
    let mut file = File::open(file_path).expect("unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("unable to read file");
    serde_yaml::from_str(&contents)
}

pub fn load_needles_from_file(file_path: &str) -> Result<Vec<Needle>, serde_yaml::Error> {
    let config_needle_vals = load_config_needles_from_file(file_path)?;
    let needle_vals = config_needle_vals
        .iter()
        .map(Needle::from_needle_val_config)
        .collect();
    Ok(needle_vals)
}

// test: load needles from file in <repo root>/needle_config.sample.yaml
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_needles_from_file() {
        let needles = load_needles_from_file("needle_config.sample.yaml").unwrap();
        assert!(needles.len() > 2);
        assert!(needles[0].name == "Example Needle 1");
        assert!(needles[1].name == "Example Needle 2");
        assert!(needles[2].name == "Example Needle 3");
    }
}
