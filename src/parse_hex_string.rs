pub fn parse_hex_string(hex_str: &str) -> Result<Vec<u8>, ()> {
    // first, check if there are spaces in the string
    if hex_str.contains(' ') || hex_str.len() == 1 {
        // Normalize the string by removing '0x' prefixes and spaces
        let hex_bytes: Vec<&str> = hex_str
            .split_whitespace()
            .map(|s| s.trim_start_matches("0x"))
            .filter(|s| !s.is_empty())
            .collect();

        // Collect bytes by parsing each pair of characters as a hex value
        match hex_bytes
            .iter()
            .map(|s| u8::from_str_radix(s, 16))
            .collect()
        {
            Ok(bytes) => Ok(bytes),
            Err(_) => Err(()),
        }
    } else {
        // No spaces, so just parse the entire string in two-byte chunks
        match hex::decode(hex_str.trim_start_matches("0x")) {
            Ok(bytes) => Ok(bytes),
            Err(_) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_input_concatenated() {
        let hex_str = "72656c6c6f";
        assert_eq!(parse_hex_string(hex_str), Ok(vec![114, 101, 108, 108, 111]));
    }

    #[test]
    fn test_valid_input_space_separated() {
        let hex_str = "72 65 6c 6c 6f"; // "Hello"
        assert_eq!(parse_hex_string(hex_str), Ok(vec![114, 101, 108, 108, 111]));
    }

    #[test]
    fn test_valid_input_space_separated_multiple_spaces() {
        let hex_str = "72    65 6c  6c 6f"; // "Hello" (duplicate spaces between hex bytes)
        assert_eq!(parse_hex_string(hex_str), Ok(vec![114, 101, 108, 108, 111]));
    }

    #[test]
    fn test_valid_input_0x_prefixed() {
        let hex_str = "0x72 0x65 0x6c 0x6c 0x6f"; // "Hello"
        assert_eq!(parse_hex_string(hex_str), Ok(vec![114, 101, 108, 108, 111]));
    }

    #[test]
    fn test_valid_input_one_character() {
        let hex_str = "f"; // "f"
        assert_eq!(parse_hex_string(hex_str), Ok(vec![0x0F]));
    }

    #[test]
    fn test_invalid_input_odd_characters() {
        let hex_str = "123"; // Odd number of characters
        assert!(parse_hex_string(hex_str).is_err());
    }

    #[test]
    fn test_invalid_input_odd_characters_multiple() {
        let hex_str = "12 123"; // Odd number of characters in the second pair
        assert!(parse_hex_string(hex_str).is_err());
    }

    #[test]
    fn test_invalid_input_non_hex_characters() {
        let hex_str = "72 65 6g 6c 6f"; // 'g' is not a valid hex character
        assert!(parse_hex_string(hex_str).is_err());
    }

    #[test]
    fn test_empty_string() {
        let hex_str = ""; // Empty string
        assert_eq!(parse_hex_string(hex_str), Ok(vec![]));
    }

    #[test]
    fn test_invalid_input_all_non_hex() {
        let hex_str = "xyz"; // Completely invalid hex characters
        assert!(parse_hex_string(hex_str).is_err());
    }

    #[test]
    fn test_mixed_valid_invalid() {
        let hex_str = "72 65 6z 6c 6f"; // Contains an invalid 'z'
        assert!(parse_hex_string(hex_str).is_err());
    }
}
