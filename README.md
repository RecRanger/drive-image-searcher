# drive-image-searcher
A Rust CLI tool to stream a drive image, and search for one or more byte patterns

```bash
cargo install drive-image-searcher
drive-image-searcher -h
```

## Features
* Supports custom "needle" definition configuration file.
* Writes out chunks of data where the needle was found.
* Fast.

## Usage

1. Download the [`needle_config.sample.yaml` file](https://github.com/RecRanger/drive-image-searcher/blob/main/needle_config.sample.yaml), and fill it with search patterns you want to locate. For example:

```yaml
- name: "Example Needle 1"
  val: "48 65 6c 6c 6f ff ff ff ff ff ff ff"  # This is "Hello" in hexadecimal
  val_format: hex
  description_notes: "A simple hex value of the word 'Hello'"
  happiness_level: 1

- name: "Example Needle 2"
  val: "word plus a bunch of other random text"
  val_format: ascii
  description_notes: "A plain ASCII value"
  happiness_level: 2
  write_to_file: false
```

2. Run `cargo install drive-image-searcher`.
3. Run `drive-image-searcher -i /path/to/dd_file.img -n /path/to/needle_config.yaml -o ./output_dir/`

When complete, matching instances within the files will be in `./output_dir/`, alongside logs.

## Bugs

* Total file size for block devices shows as 0, so ETA doesn't work.
