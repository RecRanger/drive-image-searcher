mod parse_hex_string;

mod needle;
use crate::needle::{Needle, load_needles_from_file};

mod found_needle;

mod process_data;
use crate::process_data::SearchAssignment;

mod display_hex;

use clap::{App, Arg, crate_version};
use found_needle::log_polars_summary;

use std::fs::{self, File};
use std::io;
use std::path::PathBuf;
use std::time::Instant;
use std::time::SystemTime;

use chrono::Utc;

use fern::Dispatch;
use indicatif::{HumanBytes, HumanDuration};
use log::{error, info};
use memmap2::Mmap;

fn setup_logger(log_file: &PathBuf) -> Result<(), fern::InitError> {
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_file)?)
        .apply()?;
    Ok(())
}

fn main() -> io::Result<()> {
    info!("Starting Drive Image Searcher");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    let cli_arg_matches = App::new("Drive Image Searcher")
        .version(crate_version!())
        .author("RecRanger")
        .about("Search for byte patterns in large disk images, and explore the results.")
        .arg(
            Arg::with_name("input_file_path")
                .help("Path to the input image file")
                .short('i')
                .long("input-file-path")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output_dir")
                .help("Path to output directory")
                .short('o')
                .long("output-dir")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("needle_config_yaml_path")
                .help("Path to needle config file")
                .short('n')
                .long("needle-config-file-path")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let input_file_path_str = cli_arg_matches
        .value_of("input_file_path")
        .expect("No valid input file provided");
    let cli_output_dir_str = cli_arg_matches
        .value_of("output_dir")
        .expect("No valid output directory provided");
    let needle_config_yaml_path = cli_arg_matches
        .value_of("needle_config_yaml_path")
        .expect("No valid needle config file provided");

    let input_file_path = PathBuf::from(input_file_path_str);
    let input_file_name = input_file_path
        .file_name()
        .expect("Could not get input file name")
        .to_str()
        .expect("Could not convert input file name to str");

    let output_dir_path = PathBuf::from(cli_output_dir_str).join(format!(
        "results__{}__{}",
        input_file_name,
        Utc::now().format("%Y-%m-%dT%H_%M_%S")
    ));

    let input_file_size_bytes = fs::metadata(input_file_path_str)
        .expect("Could not get input file size")
        .len();
    info!(
        "Total image size: {} ({} MiB)",
        HumanBytes(input_file_size_bytes),
        input_file_size_bytes / 1024 / 1024,
    );

    let input_file = File::open(input_file_path_str).expect("Could not open input file");

    let needles: Vec<Needle> = match load_needles_from_file(needle_config_yaml_path) {
        Ok(vals) => {
            info!(
                "Loaded {} needle values from {}",
                vals.len(),
                needle_config_yaml_path
            );
            vals
        }
        Err(e) => panic!("Could not load needle values: {:?}", e),
    };

    // Create output directory
    if !output_dir_path.exists() {
        fs::create_dir_all(&output_dir_path).expect("Failed to create output directory");
        info!("Created output directory: {}", output_dir_path.display());
    } else {
        info!(
            "Using existing output directory: {}",
            output_dir_path.display()
        );
    }

    // Setup logging
    let log_file_path = output_dir_path.join("01_general_log.log");
    setup_logger(&log_file_path).expect("Could not set up logger");

    // Re-log important information
    info!("Logging to: {}", log_file_path.display());
    info!(
        "Drive Image Searcher version: {}",
        env!("CARGO_PKG_VERSION")
    );
    info!("Using args: {:?}", cli_arg_matches);
    info!(
        "Using args: input_file_path: {}, output_dir: {}, needle_config_yaml_path: {}",
        input_file_path_str, cli_output_dir_str, needle_config_yaml_path
    );

    // Copy needle config
    let needle_config_file_dest_path = output_dir_path.join("02_needle_config.yaml");
    fs::copy(needle_config_yaml_path, &needle_config_file_dest_path)
        .expect("Could not copy needle config file to output directory");
    info!(
        "Copied needle config file to: {}",
        needle_config_file_dest_path.display()
    );

    let jsonl_output_log_file_path = output_dir_path.clone().join("00_all_output_record.jsonl");

    // Create search assignment
    let search_assignment = SearchAssignment {
        input_file_path: input_file_path.clone(),
        output_dir_path: output_dir_path.clone(),
        jsonl_output_log_file_path: jsonl_output_log_file_path.clone(),
        needles: needles.clone(),
    };

    // Memory map the file
    info!("Memory mapping input file...");
    let mmap = unsafe { Mmap::map(&input_file).expect("Failed to memory map input file") };
    info!("Successfully memory mapped {} bytes", mmap.len());

    // Optimization: Inform the kernel that it's fine to dump old pages after we're past,
    // and that we'll be requesting forward-looking pages continuously.
    mmap.advise(memmap2::Advice::Sequential)?;

    // Start search.
    info!("Starting search...");
    let search_start_time = Instant::now();

    let needle_vals_found = process_data::do_search(&search_assignment, &mmap);

    // Final rate logs.
    let elapsed = search_start_time.elapsed();
    let throughput_mbps = (mmap.len() as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64();
    info!("Search completed in {}", HumanDuration(elapsed));
    info!("Average throughput: {:.2} MiB/s", throughput_mbps);
    info!("Total matches found: {}", needle_vals_found.len());

    // Final summary log.
    match log_polars_summary(&jsonl_output_log_file_path) {
        Ok(()) => (),
        Err(e) => error!("Failed to log final summary: {}", e),
    }

    Ok(())
}
