use crate::display_hex::display_hex_offset;
use crate::found_needle::NeedleValFound;
use crate::needle::Needle;

use num_format::{Locale, ToFormattedString as _};

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use log::{debug, error, info};

pub struct SearchAssignment {
    pub input_file_path: PathBuf,
    pub output_dir_path: PathBuf,
    pub jsonl_output_log_file_path: PathBuf,
    pub needles: Vec<Needle>,
}

pub struct ProcessDataState {
    // variables to keep track of progress, etc.
    pub haystack_chunk_buffer: Vec<u8>,
    pub total_haystack_bytes_read: u64,
    pub last_progress_log_time: Instant,
    pub start_time: Instant,

    pub needle_vals_found: Vec<NeedleValFound>,
    pub chunk_count: u64,
    pub partial_chunk_read_count: u32,
}

impl ProcessDataState {
    pub fn new(haystack_chunk_buffer_size_bytes: usize) -> Self {
        Self {
            haystack_chunk_buffer: vec![0; haystack_chunk_buffer_size_bytes],
            total_haystack_bytes_read: 0,
            last_progress_log_time: Instant::now(),
            start_time: Instant::now(),
            needle_vals_found: Vec::new(),
            chunk_count: 0,
            partial_chunk_read_count: 0,
        }
    }

    pub fn sec_since_last_progress_log(&self) -> f32 {
        self.last_progress_log_time.elapsed().as_secs_f32()
    }

    pub fn do_search(&mut self, search_assignment: &SearchAssignment) {
        let haystack_chunk_start_global_offset = self.total_haystack_bytes_read;
        let _haystack_chunk_end_global_offset =
            self.total_haystack_bytes_read + (self.haystack_chunk_buffer.len() as u64);

        for needle in search_assignment.needles.as_slice() {
            let needle_val_sequence = &needle.val;
            if let Some(pos_in_chunk) = self
                .haystack_chunk_buffer
                .windows(needle_val_sequence.len())
                .position(|window| window == needle_val_sequence)
            {
                // Found a match!
                // Window = Match now
                let match_start_global_offset: u64 =
                    haystack_chunk_start_global_offset + pos_in_chunk as u64;
                let needle_val_as_string = needle.val_as_string();

                // just a debug, not the main log
                debug!(
                    "{} Found '{}' {} at position {} in the chunk",
                    needle.happiness_level_as_string(),
                    needle.name,
                    needle_val_as_string,
                    pos_in_chunk
                );

                // Create the NeedleValFound object
                let needle_val_found = NeedleValFound::from_needle_val(
                    needle,
                    match_start_global_offset + pos_in_chunk as u64,
                    &search_assignment.input_file_path,
                );

                // Write the haystack chunk to disk
                let needle_dir_path = search_assignment.output_dir_path.clone().join(format!(
                    "{}_{}",
                    needle.happiness_level,
                    needle.name.clone()
                ));
                if !needle_dir_path.exists() {
                    fs::create_dir(&needle_dir_path)
                        .expect("Could not create per-needle output directory");
                    info!(
                        "{}. First time for '{}' needle. Created new needle directory: {}",
                        needle.happiness_level_as_string(),
                        needle.name,
                        needle_dir_path.display()
                    );
                }

                if needle.write_to_file {
                    let write_start_pos_in_chunk = (pos_in_chunk as i64
                        - needle.byte_count_before_match as i64)
                        .max(0) as usize;
                    let write_end_pos_in_chunk = (pos_in_chunk
                        + needle_val_sequence.len()
                        + needle.byte_count_after_match as usize)
                        .min(self.haystack_chunk_buffer.len());

                    // `chunk_file_name` format: <this match's global offset>_<file_start_offset>_<file_end_offset>
                    let chunk_file_name = format!(
                        "found_g_0x{}_startat_0x{}.bin",
                        display_hex_offset(match_start_global_offset, 20),
                        // offset_within_file:
                        display_hex_offset(
                            match_start_global_offset + write_start_pos_in_chunk as u64,
                            1 // minimum width is good
                        ),
                    );

                    let chunk_output_file_path =
                        PathBuf::from(&needle_dir_path).join(chunk_file_name);
                    let mut output_file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(chunk_output_file_path.clone())
                        .expect("Could not open chunk output file");

                    match output_file.write_all(
                        &self.haystack_chunk_buffer
                            [write_start_pos_in_chunk..write_end_pos_in_chunk],
                    ) {
                        Ok(_) => {}
                        Err(e) => error!("Could not write haystack chunk to disk: {}", e),
                    }

                    info!(
                        "Offset 0x{}. Needle '{}'. {}. Wrote to disk ({} bytes).",
                        display_hex_offset(match_start_global_offset, 20),
                        needle.name,
                        needle.happiness_level_as_string(),
                        (write_end_pos_in_chunk - write_start_pos_in_chunk)
                            .to_formatted_string(&Locale::en),
                    );
                } else {
                    info!(
                        "Offset 0x{}. Needle '{}'. Happiness level {}. Skipping writing to disk.",
                        display_hex_offset(match_start_global_offset, 20),
                        needle.name,
                        needle.happiness_level,
                    );
                }

                // Write the needle val to disk as JSONL (in both the general file, and the needle-specific file)
                needle_val_found
                    .append_to_jsonl_file(&search_assignment.jsonl_output_log_file_path)
                    .expect("Could not write needle val to overall JSONL file");
                needle_val_found
                    .append_to_jsonl_file(
                        &PathBuf::from(&needle_dir_path).join(format!("001_{}.jsonl", needle.name)),
                    )
                    .expect("Could not write needle val to per-needle JSONL file");

                self.needle_vals_found.push(needle_val_found);
            }
        }

        // update stats
        self.total_haystack_bytes_read += self.haystack_chunk_buffer.len() as u64;
        self.chunk_count += 1;
    }
}
