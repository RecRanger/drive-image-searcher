use std::fmt::{LowerHex, UpperHex};

use thousands::{digits::ASCII_HEXADECIMAL, Separable, SeparatorPolicy};

pub fn display_hex_offset<T: LowerHex + UpperHex + num_traits::PrimInt + std::fmt::Display>(
    offset: T,
    width: usize,
) -> String {
    let hex_display_policy = SeparatorPolicy {
        separator: "_",
        groups: &[4],
        digits: ASCII_HEXADECIMAL,
    };

    let offset_str = format!("{:X}", offset);
    // zfill to width
    let offset_str = format!("{:0>1$}", offset_str, width);
    offset_str.separate_by_policy(hex_display_policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_hex_offset() {
        assert_eq!(display_hex_offset(0, 1), "0");
        assert_eq!(display_hex_offset(1, 1), "1");
        assert_eq!(display_hex_offset(10, 1), "A");
        assert_eq!(display_hex_offset(15, 1), "F");
        assert_eq!(display_hex_offset(15, 4), "000F"); // check zfill
        assert_eq!(display_hex_offset(15, 8), "0000_000F"); // check zfill with separator

        assert_eq!(display_hex_offset(16, 2), "10");
        assert_eq!(display_hex_offset(16, 1), "10"); // check requested width < actual width
        assert_eq!(display_hex_offset(4096, 4), "1000");
        assert_eq!(display_hex_offset(268435455, 7), "FFF_FFFF");
        assert_eq!(display_hex_offset(0xFFFF_FFFF_u64, 8), "FFFF_FFFF");
    }
}
