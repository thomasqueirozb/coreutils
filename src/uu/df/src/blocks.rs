//  * This file is part of the uutils coreutils package.
//  *
//  * For the full copyright and license information, please view the LICENSE
//  * file that was distributed with this source code.
//! Types for representing and displaying block sizes.
use crate::OPT_BLOCKSIZE;
use clap::ArgMatches;
use std::{env, fmt};

use uucore::{
    display::Quotable,
    parse_size::{parse_size, ParseSizeError},
};

/// The first ten powers of 1024.
const IEC_BASES: [u128; 10] = [
    1,
    1_024,
    1_048_576,
    1_073_741_824,
    1_099_511_627_776,
    1_125_899_906_842_624,
    1_152_921_504_606_846_976,
    1_180_591_620_717_411_303_424,
    1_208_925_819_614_629_174_706_176,
    1_237_940_039_285_380_274_899_124_224,
];

/// Suffixes for the first nine multi-byte unit suffixes.
const SUFFIXES: [char; 9] = ['B', 'K', 'M', 'G', 'T', 'P', 'E', 'Z', 'Y'];

const SI_BASES: [u128; 10] = [
    1,
    1_000,
    1_000_000,
    1_000_000_000,
    1_000_000_000_000,
    1_000_000_000_000_000,
    1_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000,
];

// we use "kB" instead of "KB" because of GNU df
const SI_SUFFIXES: [&str; 9] = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

/// Convert a multiple of 1024 into a string like "12K" or "34M".
///
/// # Examples
///
/// Powers of 1024 become "1K", "1M", "1G", etc.
///
/// ```rust,ignore
/// assert_eq!(to_magnitude_and_suffix_1024(1024).unwrap(), "1K");
/// assert_eq!(to_magnitude_and_suffix_1024(1024 * 1024).unwrap(), "1M");
/// assert_eq!(to_magnitude_and_suffix_1024(1024 * 1024 * 1024).unwrap(), "1G");
/// ```
///
/// Multiples of those powers affect the magnitude part of the
/// returned string:
///
/// ```rust,ignore
/// assert_eq!(to_magnitude_and_suffix_1024(123 * 1024).unwrap(), "123K");
/// assert_eq!(to_magnitude_and_suffix_1024(456 * 1024 * 1024).unwrap(), "456M");
/// assert_eq!(to_magnitude_and_suffix_1024(789 * 1024 * 1024 * 1024).unwrap(), "789G");
/// ```
fn to_magnitude_and_suffix_1024(n: u128) -> Result<String, ()> {
    // Find the smallest power of 1024 that is larger than `n`. That
    // number indicates which units and suffix to use.
    for i in 0..IEC_BASES.len() - 1 {
        if n < IEC_BASES[i + 1] {
            return Ok(format!("{}{}", n / IEC_BASES[i], SUFFIXES[i]));
        }
    }
    Err(())
}

/// Convert a number into a string like "12kB" or "34MB".
///
/// Powers of 1000 become "1kB", "1MB", "1GB", etc.
///
/// The returned string has a maximum length of 5 chars, for example: "1.1kB", "999kB", "1MB".
fn to_magnitude_and_suffix_not_powers_of_1024(n: u128) -> Result<String, ()> {
    let mut i = 0;

    while SI_BASES[i + 1] - SI_BASES[i] < n && i < SI_SUFFIXES.len() {
        i += 1;
    }

    let quot = n / SI_BASES[i];
    let rem = n % SI_BASES[i];
    let suffix = SI_SUFFIXES[i];

    if rem == 0 {
        Ok(format!("{}{}", quot, suffix))
    } else {
        let tenths_place = rem / (SI_BASES[i] / 10);

        if rem % (SI_BASES[i] / 10) == 0 {
            Ok(format!("{}.{}{}", quot, tenths_place, suffix))
        } else if tenths_place + 1 == 10 || quot >= 10 {
            Ok(format!("{}{}", quot + 1, suffix))
        } else {
            Ok(format!("{}.{}{}", quot, tenths_place + 1, suffix))
        }
    }
}

/// Convert a number into a magnitude and a multi-byte unit suffix.
///
/// # Errors
///
/// If the number is too large to represent.
fn to_magnitude_and_suffix(n: u128) -> Result<String, ()> {
    if n % 1024 == 0 && n % 1000 != 0 {
        to_magnitude_and_suffix_1024(n)
    } else {
        to_magnitude_and_suffix_not_powers_of_1024(n)
    }
}

/// A mode to use in condensing the display of a large number of bytes.
pub(crate) enum SizeFormat {
    HumanReadable(HumanReadable),
    StaticBlockSize,
}

impl Default for SizeFormat {
    fn default() -> Self {
        Self::StaticBlockSize
    }
}

/// A mode to use in condensing the human readable display of a large number
/// of bytes.
///
/// The [`HumanReadable::Decimal`] and[`HumanReadable::Binary`] variants
/// represent dynamic block sizes: as the number of bytes increases, the
/// divisor increases as well (for example, from 1 to 1,000 to 1,000,000
/// and so on in the case of [`HumanReadable::Decimal`]).
#[derive(Clone, Copy)]
pub(crate) enum HumanReadable {
    /// Use the largest divisor corresponding to a unit, like B, K, M, G, etc.
    ///
    /// This variant represents powers of 1,000. Contrast with
    /// [`HumanReadable::Binary`], which represents powers of
    /// 1,024.
    Decimal,

    /// Use the largest divisor corresponding to a unit, like B, K, M, G, etc.
    ///
    /// This variant represents powers of 1,024. Contrast with
    /// [`HumanReadable::Decimal`], which represents powers
    /// of 1,000.
    Binary,
}

/// A block size to use in condensing the display of a large number of bytes.
///
/// The [`BlockSize::Bytes`] variant represents a static block
/// size.
///
/// The default variant is `Bytes(1024)`.
#[derive(Debug, PartialEq)]
pub(crate) enum BlockSize {
    /// A fixed number of bytes.
    ///
    /// The number must be positive.
    Bytes(u64),
}

impl BlockSize {
    /// Returns the associated value
    pub(crate) fn as_u64(&self) -> u64 {
        match *self {
            Self::Bytes(n) => n,
        }
    }
}

impl Default for BlockSize {
    fn default() -> Self {
        if env::var("POSIXLY_CORRECT").is_ok() {
            Self::Bytes(512)
        } else {
            Self::Bytes(1024)
        }
    }
}

pub(crate) fn block_size_from_matches(matches: &ArgMatches) -> Result<BlockSize, ParseSizeError> {
    if matches.is_present(OPT_BLOCKSIZE) {
        let s = matches.value_of(OPT_BLOCKSIZE).unwrap();
        let bytes = parse_size(s)?;

        if bytes > 0 {
            Ok(BlockSize::Bytes(bytes))
        } else {
            Err(ParseSizeError::ParseFailure(format!("{}", s.quote())))
        }
    } else {
        Ok(Default::default())
    }
}

impl fmt::Display for BlockSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Bytes(n) => match to_magnitude_and_suffix(*n as u128) {
                Ok(s) => write!(f, "{}", s),
                Err(_) => Err(fmt::Error),
            },
        }
    }
}

#[cfg(test)]
mod tests {

    use std::env;

    use crate::blocks::{to_magnitude_and_suffix, BlockSize};

    #[test]
    fn test_to_magnitude_and_suffix_powers_of_1024() {
        assert_eq!(to_magnitude_and_suffix(1024).unwrap(), "1K");
        assert_eq!(to_magnitude_and_suffix(2048).unwrap(), "2K");
        assert_eq!(to_magnitude_and_suffix(4096).unwrap(), "4K");
        assert_eq!(to_magnitude_and_suffix(1024 * 1024).unwrap(), "1M");
        assert_eq!(to_magnitude_and_suffix(2 * 1024 * 1024).unwrap(), "2M");
        assert_eq!(to_magnitude_and_suffix(1024 * 1024 * 1024).unwrap(), "1G");
        assert_eq!(
            to_magnitude_and_suffix(34 * 1024 * 1024 * 1024).unwrap(),
            "34G"
        );
    }

    #[test]
    fn test_to_magnitude_and_suffix_not_powers_of_1024() {
        assert_eq!(to_magnitude_and_suffix(1).unwrap(), "1B");
        assert_eq!(to_magnitude_and_suffix(999).unwrap(), "999B");

        assert_eq!(to_magnitude_and_suffix(1000).unwrap(), "1kB");
        assert_eq!(to_magnitude_and_suffix(1001).unwrap(), "1.1kB");
        assert_eq!(to_magnitude_and_suffix(1023).unwrap(), "1.1kB");
        assert_eq!(to_magnitude_and_suffix(1025).unwrap(), "1.1kB");
        assert_eq!(to_magnitude_and_suffix(10_001).unwrap(), "11kB");
        assert_eq!(to_magnitude_and_suffix(999_000).unwrap(), "999kB");

        assert_eq!(to_magnitude_and_suffix(999_001).unwrap(), "1MB");
        assert_eq!(to_magnitude_and_suffix(999_999).unwrap(), "1MB");
        assert_eq!(to_magnitude_and_suffix(1_000_000).unwrap(), "1MB");
        assert_eq!(to_magnitude_and_suffix(1_000_001).unwrap(), "1.1MB");
        assert_eq!(to_magnitude_and_suffix(1_100_000).unwrap(), "1.1MB");
        assert_eq!(to_magnitude_and_suffix(1_100_001).unwrap(), "1.2MB");
        assert_eq!(to_magnitude_and_suffix(1_900_000).unwrap(), "1.9MB");
        assert_eq!(to_magnitude_and_suffix(1_900_001).unwrap(), "2MB");
        assert_eq!(to_magnitude_and_suffix(9_900_000).unwrap(), "9.9MB");
        assert_eq!(to_magnitude_and_suffix(9_900_001).unwrap(), "10MB");
        assert_eq!(to_magnitude_and_suffix(999_000_000).unwrap(), "999MB");

        assert_eq!(to_magnitude_and_suffix(999_000_001).unwrap(), "1GB");
        assert_eq!(to_magnitude_and_suffix(1_000_000_000).unwrap(), "1GB");
        assert_eq!(to_magnitude_and_suffix(1_000_000_001).unwrap(), "1.1GB");
    }

    #[test]
    fn test_to_magnitude_and_suffix_multiples_of_1000_and_1024() {
        assert_eq!(to_magnitude_and_suffix(128_000).unwrap(), "128kB");
        assert_eq!(to_magnitude_and_suffix(1000 * 1024).unwrap(), "1.1MB");
        assert_eq!(to_magnitude_and_suffix(1_000_000_000_000).unwrap(), "1TB");
    }

    #[test]
    fn test_block_size_display() {
        assert_eq!(format!("{}", BlockSize::Bytes(1024)), "1K");
        assert_eq!(format!("{}", BlockSize::Bytes(2 * 1024)), "2K");
        assert_eq!(format!("{}", BlockSize::Bytes(3 * 1024 * 1024)), "3M");
    }

    #[test]
    fn test_default_block_size() {
        assert_eq!(BlockSize::Bytes(1024), BlockSize::default());
        env::set_var("POSIXLY_CORRECT", "1");
        assert_eq!(BlockSize::Bytes(512), BlockSize::default());
        env::remove_var("POSIXLY_CORRECT");
    }
}
