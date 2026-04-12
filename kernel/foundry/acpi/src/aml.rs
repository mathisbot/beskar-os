//! AML parser.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Parsed AML data.
pub struct Aml {
    s5_sleep_type_a: u8,
    s5_sleep_type_b: u8,
}

impl Aml {
    #[must_use]
    #[inline]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let (s5_sleep_type_a, s5_sleep_type_b) = parse_s5_sleep_types(bytes)?;
        Some(Self {
            s5_sleep_type_a,
            s5_sleep_type_b,
        })
    }

    #[must_use]
    #[inline]
    pub const fn s5_sleep_type_a(self) -> u8 {
        self.s5_sleep_type_a
    }

    #[must_use]
    #[inline]
    pub const fn s5_sleep_type_b(self) -> u8 {
        self.s5_sleep_type_b
    }
}

fn parse_s5_sleep_types(aml_data: &[u8]) -> Option<(u8, u8)> {
    let mut cursor = 0usize;
    while cursor < aml_data.len() {
        if aml_data[cursor] == 0x08
            && let Some(s5) = parse_s5_at(&aml_data[cursor..])
        {
            return Some(s5);
        }
        cursor += 1;
    }

    None
}

fn parse_s5_at(aml_data: &[u8]) -> Option<(u8, u8)> {
    let mut cursor = 1;

    // Skip root prefix characters
    while matches!(aml_data.get(cursor), Some(0x5C | 0x5E)) {
        cursor += 1;
    }

    // Validate name
    // The name field is 4 chars long. It is unclear to me if the last character
    // is always an underscore or if it is undefined.
    if aml_data.get(cursor..cursor + 3)? != b"_S5" {
        return None;
    }
    cursor += 4;

    // Validate package opcode
    if *aml_data.get(cursor)? != 0x12 {
        return None;
    }
    cursor += 1;

    let (package_length, package_length_size) = parse_pkg_length(aml_data, cursor)?;
    cursor += package_length_size;
    let package_end = cursor + package_length;

    let package_data = aml_data.get(cursor..package_end)?;
    let package_elements = usize::from(*package_data.first()?);
    if package_elements == 0 {
        return None;
    }
    let mut package_cursor = 1usize;

    let (sleep_type_a, consumed_a) = parse_integer(package_data, package_cursor)?;
    package_cursor += consumed_a;

    let sleep_type_b = if package_elements >= 2 {
        let (sleep_type_b, _) = parse_integer(package_data, package_cursor)?;
        sleep_type_b
    } else {
        sleep_type_a
    };

    let sleep_type_a = u8::try_from(sleep_type_a).ok()?;
    let sleep_type_b = u8::try_from(sleep_type_b).ok()?;

    if sleep_type_a > 0b111 || sleep_type_b > 0b111 {
        return None;
    }

    Some((sleep_type_a, sleep_type_b))
}

fn parse_pkg_length(aml_data: &[u8], start: usize) -> Option<(usize, usize)> {
    let lead = *aml_data.get(start)?;
    let follow_count = usize::from(lead >> 6);

    let mut length = if follow_count == 0 {
        usize::from(lead & 0x3F)
    } else {
        usize::from(lead & 0x0F)
    };

    for i in 0..follow_count {
        let byte = *aml_data.get(start + 1 + i)?;
        length |= usize::from(byte) << (4 + i * 8);
    }

    Some((length, 1 + follow_count))
}

fn parse_integer(aml_data: &[u8], start: usize) -> Option<(u64, usize)> {
    let opcode = *aml_data.get(start)?;

    match opcode {
        0x00 => Some((0, 1)),
        0x01 => Some((1, 1)),
        0xFF => Some((u64::MAX, 1)),
        0x0A => Some((u64::from(*aml_data.get(start + 1)?), 2)),
        0x0B => {
            let b0 = *aml_data.get(start + 1)?;
            let b1 = *aml_data.get(start + 2)?;
            Some((u64::from(u16::from_le_bytes([b0, b1])), 3))
        }
        0x0C => {
            let b0 = *aml_data.get(start + 1)?;
            let b1 = *aml_data.get(start + 2)?;
            let b2 = *aml_data.get(start + 3)?;
            let b3 = *aml_data.get(start + 4)?;
            Some((u64::from(u32::from_le_bytes([b0, b1, b2, b3])), 5))
        }
        0x0E => {
            let b0 = *aml_data.get(start + 1)?;
            let b1 = *aml_data.get(start + 2)?;
            let b2 = *aml_data.get(start + 3)?;
            let b3 = *aml_data.get(start + 4)?;
            let b4 = *aml_data.get(start + 5)?;
            let b5 = *aml_data.get(start + 6)?;
            let b6 = *aml_data.get(start + 7)?;
            let b7 = *aml_data.get(start + 8)?;
            Some((u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]), 9))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_aml;

    #[test]
    fn parses_s5_with_root_prefix() {
        let aml = [
            0x08, 0x5C, b'_', b'S', b'5', b'_', 0x12, 0x05, 0x02, 0x0A, 0x05, 0x0A, 0x05,
        ];
        let parsed = parse_aml(&aml).expect("failed to parse _S5 package");

        assert_eq!(parsed.s5_sleep_type_a(), 5);
        assert_eq!(parsed.s5_sleep_type_b(), 5);
    }

    #[test]
    fn parses_s5_with_prefixless_ones() {
        let aml = [0x08, b'_', b'S', b'5', b'_', 0x12, 0x03, 0x02, 0x01, 0x01];
        let parsed = parse_aml(&aml).expect("failed to parse _S5 package");

        assert_eq!(parsed.s5_sleep_type_a(), 1);
        assert_eq!(parsed.s5_sleep_type_b(), 1);
    }

    #[test]
    fn returns_none_when_no_s5_package_is_present() {
        let aml = [0x08, b'_', b'T', b'E', b'S', 0x0A, 0x42];
        assert!(parse_aml(&aml).is_none());
    }
}
