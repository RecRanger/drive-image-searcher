use serde::{Deserialize, Serialize};

use std::env;
use std::error;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;

use polars::prelude::*;

use log::info;

use crate::needle::Needle;

#[derive(Serialize, Deserialize, Debug)]
pub struct NeedleValFound {
    pub name: String,
    pub match_start_global_offset: u64,
    pub val: Vec<u8>,
    pub val_as_str: String,
    pub description_notes: String,
    pub happiness_level: u8,
    pub found_timestamp_utc: String,

    pub haystack_written_to_file: bool,
    pub haystack_file_path: Option<String>,
    pub haystack_file_name: Option<String>,
}

impl NeedleValFound {
    pub fn from_needle_val(
        needle_val: &Needle,
        match_start_global_offset: u64,
        input_file_path: &Path,
    ) -> NeedleValFound {
        let input_file_name = input_file_path
            .file_name()
            .expect("Could not get input file name")
            .to_str()
            .expect("Could not convert input file name to str");

        let haystack_file_path = match needle_val.write_to_file {
            true => Some(
                input_file_path
                    .to_str()
                    .expect("Could not convert input file path to str")
                    .to_string(),
            ),
            false => None,
        };
        let haystack_file_name = match needle_val.write_to_file {
            true => Some(input_file_name.to_string()),
            false => None,
        };

        let needle_val_found = NeedleValFound {
            name: needle_val.name.clone(),
            match_start_global_offset,
            val: needle_val.val.clone(),
            val_as_str: needle_val.val_as_string(),
            description_notes: needle_val.description_notes.clone(),
            happiness_level: needle_val.happiness_level,
            found_timestamp_utc: Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            haystack_written_to_file: needle_val.write_to_file,
            haystack_file_path,
            haystack_file_name,
        };
        needle_val_found
    }

    pub fn append_to_jsonl_file(&self, jsonl_file_path: &PathBuf) -> Result<(), std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(jsonl_file_path)?;
        let mut writer = BufWriter::new(file);

        // Serialize the record to a JSON string
        let json = serde_json::to_string(self)?;

        // Write the JSON string followed by a newline to the file
        writeln!(writer, "{}", json)?;
        writer.flush() // Ensure all data is written to the file system
    }
}

pub fn log_polars_summary(
    jsonl_file_path: &PathBuf,
) -> std::result::Result<(), Box<dyn error::Error>> {
    let mut file = std::fs::File::open(jsonl_file_path)?;
    let df = JsonLineReader::new(&mut file).finish()?;

    let df = df
        .lazy()
        .group_by(["name"])
        .agg([
            col("happiness_level").first(), // should all be the same
            len().alias("count"),
            col("match_start_global_offset")
                .max()
                .alias("latest_global_offset"),
            col("description_notes").first(), // should all be the same
        ])
        .sort(
            ["happiness_level", "count", "name"],
            SortMultipleOptions::default().with_order_descending_multi(vec![true, true, false]),
        )
        .collect()?;

    // Print out the result.
    unsafe {
        env::set_var("POLARS_FMT_MAX_ROWS", (df.height() + 5).to_string());
    }
    info!("{}", df);

    Ok(())
}
