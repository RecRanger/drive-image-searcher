# drive-image-searcher
A Rust CLI tool to stream a drive image, and search for one or more byte patterns

## Features
* Supports custom "needle" definition configuration file.
* Supports reading from compressed disk images (lz4 and xz compression).
* Writes out chunks of data where the needle was found.
* Fast.

## Usage

1. Copy the `needle_config.sample.yaml` file, and fill it with search patterns you want to locate. <!-- TODO: add link, fill example inline here >
2. Run `cargo install drive_image_searcher`.
3. Run `drive_image_searcher -c none -i /path/to/dd_file.img -n /path/to/needle_config.yaml -o ./output_dir/`

When complete, matching instances within the files will be in `./output_dir/`, alongside logs.
