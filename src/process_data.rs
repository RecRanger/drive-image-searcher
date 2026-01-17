use crate::found_needle::NeedleValFound;
use crate::needle::Needle;
use crate::{display_hex::display_hex_offset, found_needle::log_polars_summary};

use aho_corasick::AhoCorasick;
use indicatif::{ProgressBar, ProgressStyle};
use memmap2::Mmap;
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

pub fn do_search(search_assignment: &SearchAssignment, haystack: &Mmap) -> Vec<NeedleValFound> {
    let mut last_summary_log = Instant::now();
    let haystack_len = haystack.len();

    // Setup progress bar.
    let progress_bar = ProgressBar::new(haystack.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({binary_bytes_per_sec}) - ETA {eta}")
            .unwrap()
            .progress_chars("#>-")
    );

    info!("Searching for {} needles", search_assignment.needles.len());

    let mut needle_vals_found: Vec<NeedleValFound> = Vec::new();

    // Create needle directories upfront.
    for needle in &search_assignment.needles {
        let needle_dir_path = search_assignment
            .output_dir_path
            .join(format!("{}_{}", needle.happiness_level, needle.name));

        if !needle_dir_path.exists() {
            fs::create_dir(&needle_dir_path).expect("Could not create per-needle output directory");
            info!(
                "Created needle directory for '{}': {}",
                needle.name,
                needle_dir_path.display()
            );
        }
    }

    // Build Aho-Corasick automaton.
    info!("Building Aho-Corasick automaton...");
    let patterns: Vec<&[u8]> = search_assignment
        .needles
        .iter()
        .map(|n| n.val.as_slice())
        .collect();

    let ac = AhoCorasick::new(patterns).expect("Failed to build Aho-Corasick automaton");
    info!("Automaton built, starting search...");

    let mut last_position = 0;

    // Process matches as they're found.
    for mat in ac.find_iter(haystack) {
        let search_pos = mat.start();
        let pattern_idx = mat.pattern().as_usize();
        let needle = &search_assignment.needles[pattern_idx];
        let pattern_len = needle.val.len();

        // Update progress bar periodically.
        if search_pos - last_position > 10 * 1024 * 1024 {
            progress_bar.set_position(search_pos as u64);
            last_position = search_pos;

            // Log summary every 30 seconds.
            if last_summary_log.elapsed().as_secs() >= 30 {
                info!(
                    "Progress: {:.2}% complete, {} matches found so far",
                    (search_pos as f64 / haystack_len as f64) * 100.0,
                    needle_vals_found.len()
                );

                match log_polars_summary(&search_assignment.jsonl_output_log_file_path) {
                    Ok(()) => (),
                    Err(e) => error!("Failed to log polars summary: {}", e),
                }

                last_summary_log = Instant::now();
            }
        }

        // Found a match!
        debug!(
            "{} Found '{}' {} at offset 0x{}",
            needle.happiness_level_as_string(),
            needle.name,
            needle.val_as_string(),
            display_hex_offset(search_pos, 20)
        );

        // Create the NeedleValFound object.
        let needle_val_found = NeedleValFound::from_needle_val(
            needle,
            search_pos as u64,
            &search_assignment.input_file_path,
        );

        let needle_dir_path = search_assignment
            .output_dir_path
            .join(format!("{}_{}", needle.happiness_level, needle.name));

        // Write match context to file if requested.
        if needle.write_to_file {
            let write_start_offset =
                search_pos.saturating_sub(needle.byte_count_before_match as usize);
            let write_end_offset =
                (search_pos + pattern_len + needle.byte_count_after_match as usize)
                    .min(haystack_len);

            let chunk_file_name = format!(
                "found_g_0x{}_startat_0x{}.bin",
                display_hex_offset(search_pos as u64, 20),
                display_hex_offset(search_pos as u64 - (write_start_offset as u64), 1),
            );

            let chunk_output_file_path = needle_dir_path.join(chunk_file_name);
            let mut output_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&chunk_output_file_path)
                .expect("Could not open chunk output file");

            match output_file.write_all(&haystack[write_start_offset..write_end_offset]) {
                Ok(_) => {}
                Err(e) => error!("Could not write haystack chunk to disk: {}", e),
            }

            info!(
                "Offset 0x{}. Needle '{}'. {}. Wrote to disk ({} bytes).",
                display_hex_offset(search_pos, 20),
                needle.name,
                needle.happiness_level_as_string(),
                (write_end_offset - write_start_offset).to_formatted_string(&Locale::en),
            );
        } else {
            info!(
                "Offset 0x{}. Needle '{}'. Happiness level {}. Skipping writing to disk.",
                display_hex_offset(search_pos, 20),
                needle.name,
                needle.happiness_level,
            );
        }

        // Write to JSONL files.
        needle_val_found
            .append_to_jsonl_file(&search_assignment.jsonl_output_log_file_path)
            .expect("Could not write needle val to overall JSONL file");
        needle_val_found
            .append_to_jsonl_file(&needle_dir_path.join(format!("001_{}.jsonl", needle.name)))
            .expect("Could not write needle val to per-needle JSONL file");

        needle_vals_found.push(needle_val_found);
    }

    progress_bar.finish_with_message("Search complete");

    // Final summary per needle.
    info!("Search complete. Summary by needle:");
    for needle in &search_assignment.needles {
        let count = needle_vals_found
            .iter()
            .filter(|n| n.name == needle.name)
            .count();
        info!("  - '{}': {} matches", needle.name, count);
    }

    needle_vals_found
}
