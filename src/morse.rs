pub const TABLE: &[(char, &str)] = &[
    ('A', ".-"),
    ('B', "-..."),
    ('C', "-.-."),
    ('D', "-.."),
    ('E', "."),
    ('F', "..-."),
    ('G', "--."),
    ('H', "...."),
    ('I', ".."),
    ('J', ".---"),
    ('K', "-.-"),
    ('L', ".-.."),
    ('M', "--"),
    ('N', "-."),
    ('O', "---"),
    ('P', ".--."),
    ('Q', "--.-"),
    ('R', ".-."),
    ('S', "..."),
    ('T', "-"),
    ('U', "..-"),
    ('V', "...-"),
    ('W', ".--"),
    ('X', "-..-"),
    ('Y', "-.--"),
    ('Z', "--.."),
    ('0', "-----"),
    ('1', ".----"),
    ('2', "..---"),
    ('3', "...--"),
    ('4', "....-"),
    ('5', "....."),
    ('6', "-...."),
    ('7', "--..."),
    ('8', "---.."),
    ('9', "----."),
];

#[allow(dead_code)]
pub fn encode(c: char) -> Option<&'static str> {
    let c = c.to_ascii_uppercase();
    TABLE.iter().find(|(ch, _)| *ch == c).map(|(_, m)| *m)
}

#[allow(dead_code)]
pub fn decode(s: &str) -> Option<char> {
    let s = normalise(s);
    TABLE.iter().find(|(_, m)| *m == s).map(|(ch, _)| *ch)
}

/// Normalise user input: trim, strip all whitespace, replace typographic dashes/dots.
pub fn normalise(s: &str) -> String {
    s.trim()
        .replace(&['–', '—', '−'][..], "-")
        .replace(&['·', '•'][..], ".")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("")
}

/// Duration of one dit in milliseconds at the given WPM (minimum 1 WPM to avoid divide-by-zero).
pub fn dit_ms(wpm: u32) -> u64 {
    1200 / wpm.max(1) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_chars() {
        for (ch, _) in TABLE {
            let code = encode(*ch).unwrap();
            assert_eq!(decode(code), Some(*ch));
        }
    }

    #[test]
    fn normalise_strips_typographic() {
        assert_eq!(
            normalise("–..–"),
            "-..−".replace('−', "-").replace('–', "-")
        );
        assert_eq!(normalise(" .- "), ".-");
    }

    #[test]
    fn dit_ms_standard() {
        assert_eq!(dit_ms(5), 240);
        assert_eq!(dit_ms(13), 92);
        assert_eq!(dit_ms(20), 60);
    }
}
