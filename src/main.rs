mod parse_hex_string;

mod needle;
use crate::needle::{load_needles_from_file, Needle};

mod found_needle;

mod process_data;
use crate::process_data::{ProcessDataState, SearchAssignment};

mod display_hex;

use num_format::{Locale, ToFormattedString as _};

use clap::{crate_version, App, Arg};
use found_needle::log_polars_summary;

use std::fs::{self, File};
use std::io::{self, Read, Seek as _};
use std::path::PathBuf;
use std::time::Instant;
use std::time::SystemTime;

use chrono::Utc;

// use: lz4_flex
use xz2::read::XzDecoder;

use fern::Dispatch;
use log::{debug, error, info, warn};
use num_traits::AsPrimitive;

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
                .help("Path to the input image file (can be compressed)")
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
        // TODO: automatically detect the compression format
        .arg(
            Arg::with_name("compression_format")
                .help("Compression format of input file (none, xz, or lz4)")
                .short('c')
                .long("compression-format")
                .possible_values(vec!["none", "xz", "lz4"])
                .default_value("none"),
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
    let compression_format = cli_arg_matches
        .value_of("compression_format")
        .expect("No valid compression format provided");
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
        "Total compressed image size: {} bytes = {} MiB",
        input_file_size_bytes.to_formatted_string(&Locale::en),
        ((input_file_size_bytes as f32 / 1024.0 / 1024.0).round() as u64)
            .to_formatted_string(&Locale::en),
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

    // checked all pre-conditions; probably should not fail anymore based on invalid args, so we can start making dirs

    if !output_dir_path.exists() {
        fs::create_dir_all(&output_dir_path).expect("Failed to create output directory");
        info!("Created output directory: {}", output_dir_path.display());
    } else {
        info!(
            "Using existing output directory: {}",
            output_dir_path.display()
        );
    }

    // bind the logs to the output directory
    let log_file_path = output_dir_path.join("01_general_log.log");
    setup_logger(&log_file_path).expect("Could not set up logger");

    // re-log a few things so they show in the file
    info!("Logging to: {}", log_file_path.display());
    info!(
        "Drive Image Searcher version: {}",
        env!("CARGO_PKG_VERSION")
    );
    info!("Using args: {:?}", cli_arg_matches);
    info!("Using args: input_file_path: {}, compression_format: {}, output_dir: {}, needle_config_yaml_path: {}",
        input_file_path_str, compression_format, cli_output_dir_str, needle_config_yaml_path);

    // copy the needle config file to the output directory
    let needle_config_file_dest_path = output_dir_path.join("02_needle_config.yaml");
    fs::copy(needle_config_yaml_path, &needle_config_file_dest_path)
        .expect("Could not copy needle config file to output directory");
    info!(
        "Copied needle config file to: {}",
        needle_config_file_dest_path.display()
    );

    let jsonl_output_log_file_path = output_dir_path.clone().join("00_all_output_record.jsonl");

    // pack into a struct for easy passage as an arg
    let search_assignment = SearchAssignment {
        input_file_path: input_file_path.clone(),
        output_dir_path: output_dir_path.clone(),
        jsonl_output_log_file_path: jsonl_output_log_file_path.clone(),
        needles: needles.clone(),
    };

    let mut input_reader: InputReader = match compression_format {
        "none" => InputReader::File(input_file),
        "lz4" => InputReader::Lz4(lz4_flex::frame::FrameDecoder::new(input_file)),
        "xz" => InputReader::Xz(XzDecoder::new(input_file)),
        other_compression_format => panic!(
            "Invalid compression format provided: {}",
            other_compression_format
        ),
    };

    // Amount from the end of the previous read to carry forward
    let haystack_carry_forward_len_bytes = 1024;

    // These sizes are important, as they determine how much memory to allocate for the haystack buffer.
    let haystack_chunk_buffer_size_bytes: usize = match input_reader {
        InputReader::File(_) => 8*1024*1024, // 8 MiB
        InputReader::Lz4(_) => 4194304 + haystack_carry_forward_len_bytes,
        InputReader::Xz(_) => unimplemented!("XzReader not implemented yet, because the returned buffer is a variable length. A refactor is required to work like that.") // 4096 + haystack_carry_forward_len_bytes,
    };
    info!(
        "Haystack (uncompressed) chunk buffer size: {} bytes = {} MiB",
        haystack_chunk_buffer_size_bytes,
        ((haystack_chunk_buffer_size_bytes as f32 / 1024.0 / 1024.0).round() as u64)
            .to_formatted_string(&Locale::en)
    );

    let mut process_data_state = ProcessDataState::new(haystack_chunk_buffer_size_bytes);

    // Read chunks of the file
    info!("Starting search...");

    loop {
        if process_data_state.total_haystack_bytes_read > 0 {
            // move the last `haystack_carry_forward_len_bytes` bytes to the beginning of the buffer
            process_data_state.haystack_chunk_buffer.copy_within(
                (haystack_carry_forward_len_bytes)..(haystack_chunk_buffer_size_bytes as usize), // to the end
                0,
            );
        }

        // TODO: refactor this read step to probably be in the process_data_state/process_data places
        // FIXME: fix the bug where the offsets are incorrect as a result of the carry forward not shifting the haystack
        // TODO: before writing out a Needle Find, check that it's not already found (by offset and pattern), because if it's in the 1024 byte carry forward, it gets duplicated right now
        match input_reader
            .read(&mut process_data_state.haystack_chunk_buffer[haystack_carry_forward_len_bytes..])
        {
            Ok(bytes_read_this_chunk) => {
                debug!("Read {} bytes", bytes_read_this_chunk);

                if (process_data_state.sec_since_last_progress_log() >= 30.0)
                    || (bytes_read_this_chunk == 0)
                {
                    info!(
                        "Progress stats: {}",
                        make_progress_stats_message(
                            &input_reader,
                            input_file_size_bytes,
                            &process_data_state
                        )
                    );

                    match log_polars_summary(&jsonl_output_log_file_path) {
                        Ok(()) => (),
                        Err(e) => error!("Failed to log polars summary: {}", e),
                    }

                    process_data_state.last_progress_log_time = Instant::now();
                }

                if bytes_read_this_chunk == 0 {
                    info!(
                        "Finished searching. No more bytes to read. Total haystack bytes read: {}",
                        process_data_state
                            .total_haystack_bytes_read
                            .to_formatted_string(&Locale::en)
                    );
                    break;
                } else if bytes_read_this_chunk
                    < (haystack_chunk_buffer_size_bytes - haystack_carry_forward_len_bytes)
                {
                    // null out the rest of the buffer to the end
                    let end_of_data_idx = haystack_carry_forward_len_bytes + bytes_read_this_chunk;
                    process_data_state.haystack_chunk_buffer[(end_of_data_idx + 1)..].fill(0);
                    info!("Finishing search. This should be the last haystack chunk. Only read {}/{} bytes",
                        end_of_data_idx.to_formatted_string(&Locale::en),
                        (haystack_chunk_buffer_size_bytes - haystack_carry_forward_len_bytes)
                            .to_formatted_string(&Locale::en));

                    if process_data_state.partial_chunk_read_count > 0 {
                        warn!("Partial chunk read count: {} (>0) already. This should only happen once.",
                            process_data_state.partial_chunk_read_count);
                    }
                    process_data_state.partial_chunk_read_count += 1;
                }
            }
            Err(e) => panic!("Could not read: {}", e),
        }

        // If all the bytes in the chunk are the same value, then we can skip searching this chunk.
        // This happens a lot for null/0 bytes in practice.
        let first_val = process_data_state.haystack_chunk_buffer[0];
        if process_data_state
            .haystack_chunk_buffer
            .iter()
            .all(|&val| val == first_val)
        {
            // This log message happens a lot:
            // debug!(
            //     "Skipping search for chunk {} because all bytes are the same: {}",
            //     process_data_state.chunk_count, first_val
            // );
        } else {
            // don't need to skip, so search
            process_data::do_search(&mut process_data_state, &search_assignment);
        }

        // update stats
        process_data_state.total_haystack_bytes_read +=
            process_data_state.haystack_chunk_buffer.len() as u64;
        process_data_state.chunk_count += 1;
    }

    info!(
        "Finished searching. Found {} matches.",
        process_data_state.needle_vals_found.len()
    );

    Ok(())
}

fn format_duration<T: AsPrimitive<u64>>(seconds: T) -> String {
    let secs = seconds.as_();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn make_progress_stats_message(
    input_reader: &InputReader,
    input_source_file_size: u64,
    process_data_state: &ProcessDataState,
) -> String {
    let compression_ratio = input_reader.total_in() as f32 / input_reader.total_out() as f32;
    let total_uncompressed_image_size = (input_source_file_size as f32) / compression_ratio;
    let elapsed_time_sec = process_data_state.start_time.elapsed().as_secs_f32();
    let expected_time_remaining_sec =
        elapsed_time_sec * (input_source_file_size as f32) / input_reader.total_in() as f32;

    let message = format!("{} elapsed, {}MiB / {}MiB decompressed ({}% complete), {} MiB/{} MiB searched ({}% complete), {} remaining, {} MiB/s out, ratio: {}%, {} chunks",
        format_duration(elapsed_time_sec.round()),

        // compressed (input-side) stats
        ((input_reader.total_in() as f32 / 1024.0 / 1024.0).round() as u64).to_formatted_string(&Locale::en),
        ((input_source_file_size as f32 / 1024.0 / 1024.0).round() as u64).to_formatted_string(&Locale::en),
        (input_reader.total_in() as f32 / input_source_file_size as f32 * 100.0).round(),

        // uncompressed (output-side) stats
        ((process_data_state.total_haystack_bytes_read as f32 / 1024.0 / 1024.0).round() as u64).to_formatted_string(&Locale::en),
        ((total_uncompressed_image_size / 1024.0 / 1024.0).round() as u64).to_formatted_string(&Locale::en),
        (process_data_state.total_haystack_bytes_read as f32 / total_uncompressed_image_size * 100.0).round(),

        // other
        format_duration(expected_time_remaining_sec.round()),
        (process_data_state.total_haystack_bytes_read as f32 / elapsed_time_sec / 1024.0 / 1024.0).round(),
        (compression_ratio * 100.0).round(),
        process_data_state.chunk_count
    );

    message
}

enum InputReader {
    File(File),
    Xz(XzDecoder<File>),
    Lz4(lz4_flex::frame::FrameDecoder<File>),
}

impl Read for InputReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            InputReader::File(file) => file.read(buf),
            InputReader::Xz(xz_decoder) => xz_decoder.read(buf),
            InputReader::Lz4(lz4_decoder) => lz4_decoder.read(buf),
        }
    }
}

trait TotalInOut {
    fn total_in(&self) -> u64;
    fn total_out(&self) -> u64;
}

impl TotalInOut for InputReader {
    fn total_in(&self) -> u64 {
        match self {
            InputReader::File(file_reader) => {
                // return the current byte position
                // Note: the clone is used because otherwise it requires a mutable reference (cringe)
                match (*file_reader).try_clone() {
                    Ok(mut cloned_file_reader) => cloned_file_reader
                        .stream_position()
                        .expect("Could not get stream position for file reader in total_out()"),
                    Err(_e) => 1, // arbitrary non-zero value
                }
            }
            InputReader::Xz(xz_reader) => xz_reader.total_in(),
            InputReader::Lz4(_lz4_reader) => {
                // FIXME: use lz4_reader.total_in(), if it's ever added
                1 // hack to return a non-zero value, because lz4 doesn't support total_in()
            }
        }
    }

    fn total_out(&self) -> u64 {
        match self {
            InputReader::File(file_reader) => {
                // return the current byte position
                // Note: the clone is used because otherwise it requires a mutable reference (cringe)
                match (*file_reader).try_clone() {
                    Ok(mut cloned_file_reader) => cloned_file_reader
                        .stream_position()
                        .expect("Could not get stream position for file reader in total_out()"),
                    Err(_e) => 1, // arbitrary non-zero value
                }
            }
            InputReader::Xz(xz_reader) => xz_reader.total_out(),
            InputReader::Lz4(_lz4_reader) => {
                1 // hack to return a non-zero value, because lz4 doesn't support total_out()
            }
        }
    }
}
