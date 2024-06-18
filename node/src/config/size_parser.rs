use crate::config::error::ConfigError;
use std::str::FromStr;

#[derive(Debug)]
enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
}

impl FromStr for SizeUnit {
    type Err = ();

    fn from_str(input: &str) -> Result<SizeUnit, Self::Err> {
        match input.to_lowercase().as_str() {
            "b" => Ok(SizeUnit::Bytes),
            "kb" => Ok(SizeUnit::Kilobytes),
            "mb" => Ok(SizeUnit::Megabytes),
            "gb" => Ok(SizeUnit::Gigabytes),
            _ => Err(()),
        }
    }
}

pub fn parse_size(size_str: &str) -> Result<u64, ConfigError> {
    let size_str = size_str.trim();
    let mut number_part = String::new();
    let mut unit_part = String::new();

    for c in size_str.chars() {
        if c.is_ascii_digit() {
            number_part.push(c);
        } else {
            unit_part.push(c);
        }
    }

    let number: u64 = number_part
        .parse()
        .map_err(|_| ConfigError::InvalidSizeNumber)?;
    let unit: SizeUnit = unit_part
        .parse()
        .map_err(|_| ConfigError::InvalidSizeUnit)?;

    let bytes = match unit {
        SizeUnit::Bytes => number,
        SizeUnit::Kilobytes => number * 1024,
        SizeUnit::Megabytes => number * 1024 * 1024,
        SizeUnit::Gigabytes => number * 1024 * 1024 * 1024,
    };

    Ok(bytes)
}
