//! IFC GlobalId: a 128-bit identifier in the 22-character IFC base64 form.

const ALPHABET: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// FNV-1a, 128-bit variant: stable across platforms and compiler versions.
fn fnv1a_128(parts: &[&[u8]]) -> u128 {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;
    let mut h = OFFSET;
    for part in parts {
        for &b in *part {
            h ^= b as u128;
            h = h.wrapping_mul(PRIME);
        }
        // Separator so ("ab", "c") and ("a", "bc") differ.
        h ^= 0xff;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Encode a 128-bit value: 2 bits in the first character, 6 in each of the other 21.
pub fn encode(n: u128) -> String {
    let mut s = String::with_capacity(22);
    s.push(ALPHABET[(n >> 126) as usize] as char);
    for i in 0..21 {
        s.push(ALPHABET[((n >> (120 - 6 * i)) & 63) as usize] as char);
    }
    s
}

/// Deterministic GlobalId for the object named `tag` within the project
/// identified by `seed`, so repeated exports keep the same ids.
pub fn guid(seed: &str, tag: &str) -> String {
    encode(fnv1a_128(&[seed.as_bytes(), tag.as_bytes()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_values() {
        assert_eq!(encode(0), "0000000000000000000000");
        assert_eq!(encode(u128::MAX), "3$$$$$$$$$$$$$$$$$$$$$");
        assert_eq!(encode(64), "0000000000000000000010");
    }

    #[test]
    fn guids_are_stable_and_distinct() {
        let a = guid("p", "element-E1");
        assert_eq!(a.len(), 22);
        assert_eq!(a, guid("p", "element-E1"));
        assert_ne!(a, guid("p", "element-E2"));
        assert_ne!(a, guid("q", "element-E1"));
        assert_ne!(guid("ab", "c"), guid("a", "bc"));
    }
}
