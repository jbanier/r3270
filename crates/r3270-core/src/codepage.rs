use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodePageKind {
    Cp037,
    Cp1047,
}

impl Default for CodePageKind {
    fn default() -> Self {
        Self::Cp037
    }
}

pub trait CodePage: Send + Sync {
    fn kind(&self) -> CodePageKind;
    fn decode_byte(&self, byte: u8) -> char;
    fn encode_char(&self, ch: char) -> Option<u8>;

    fn decode(&self, bytes: &[u8]) -> String {
        bytes.iter().map(|byte| self.decode_byte(*byte)).collect()
    }

    fn encode(&self, text: &str) -> Vec<u8> {
        text.chars()
            .map(|ch| self.encode_char(ch).unwrap_or(0x6f))
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltInCodePage {
    kind: CodePageKind,
}

impl BuiltInCodePage {
    pub fn new(kind: CodePageKind) -> Self {
        Self { kind }
    }
}

impl CodePage for BuiltInCodePage {
    fn kind(&self) -> CodePageKind {
        self.kind
    }

    fn decode_byte(&self, byte: u8) -> char {
        decode_sbc(byte, self.kind)
    }

    fn encode_char(&self, ch: char) -> Option<u8> {
        encode_sbc(ch, self.kind)
    }
}

pub fn codepage(kind: CodePageKind) -> BuiltInCodePage {
    BuiltInCodePage::new(kind)
}

fn decode_sbc(byte: u8, kind: CodePageKind) -> char {
    match byte {
        0x00 => '\0',
        0x05 => '\t',
        0x15 | 0x25 => '\n',
        0x40 => ' ',
        0x4a => match kind {
            CodePageKind::Cp037 => '¢',
            CodePageKind::Cp1047 => '[',
        },
        0x4b => '.',
        0x4c => '<',
        0x4d => '(',
        0x4e => '+',
        0x4f => '|',
        0x50 => '&',
        0x5a => '!',
        0x5b => '$',
        0x5c => '*',
        0x5d => ')',
        0x5e => ';',
        0x5f => '¬',
        0x60 => '-',
        0x61 => '/',
        0x6a => match kind {
            CodePageKind::Cp037 => '¦',
            CodePageKind::Cp1047 => ']',
        },
        0x6b => ',',
        0x6c => '%',
        0x6d => '_',
        0x6e => '>',
        0x6f => '?',
        0x79 => '`',
        0x7a => ':',
        0x7b => '#',
        0x7c => '@',
        0x7d => '\'',
        0x7e => '=',
        0x7f => '"',
        0x81..=0x89 => (b'a' + (byte - 0x81)) as char,
        0x91..=0x99 => (b'j' + (byte - 0x91)) as char,
        0xa2..=0xa9 => (b's' + (byte - 0xa2)) as char,
        0xad => match kind {
            CodePageKind::Cp037 => '[',
            CodePageKind::Cp1047 => '¢',
        },
        0xbd => match kind {
            CodePageKind::Cp037 => ']',
            CodePageKind::Cp1047 => '¦',
        },
        0xc0 => '{',
        0xc1..=0xc9 => (b'A' + (byte - 0xc1)) as char,
        0xd0 => '}',
        0xd1..=0xd9 => (b'J' + (byte - 0xd1)) as char,
        0xe0 => '\\',
        0xe2..=0xe9 => (b'S' + (byte - 0xe2)) as char,
        0xf0..=0xf9 => (b'0' + (byte - 0xf0)) as char,
        _ if byte < 0x40 => ' ',
        _ => '·',
    }
}

fn encode_sbc(ch: char, kind: CodePageKind) -> Option<u8> {
    match ch {
        '\0' => Some(0x00),
        '\t' => Some(0x05),
        '\n' => Some(0x25),
        ' ' => Some(0x40),
        'a'..='i' => Some(0x81 + (ch as u8 - b'a')),
        'j'..='r' => Some(0x91 + (ch as u8 - b'j')),
        's'..='z' => Some(0xa2 + (ch as u8 - b's')),
        'A'..='I' => Some(0xc1 + (ch as u8 - b'A')),
        'J'..='R' => Some(0xd1 + (ch as u8 - b'J')),
        'S'..='Z' => Some(0xe2 + (ch as u8 - b'S')),
        '0'..='9' => Some(0xf0 + (ch as u8 - b'0')),
        '.' => Some(0x4b),
        '<' => Some(0x4c),
        '(' => Some(0x4d),
        '+' => Some(0x4e),
        '|' => Some(0x4f),
        '&' => Some(0x50),
        '!' => Some(0x5a),
        '$' => Some(0x5b),
        '*' => Some(0x5c),
        ')' => Some(0x5d),
        ';' => Some(0x5e),
        '¬' => Some(0x5f),
        '-' => Some(0x60),
        '/' => Some(0x61),
        ',' => Some(0x6b),
        '%' => Some(0x6c),
        '_' => Some(0x6d),
        '>' => Some(0x6e),
        '?' => Some(0x6f),
        '`' => Some(0x79),
        ':' => Some(0x7a),
        '#' => Some(0x7b),
        '@' => Some(0x7c),
        '\'' => Some(0x7d),
        '=' => Some(0x7e),
        '"' => Some(0x7f),
        '{' => Some(0xc0),
        '}' => Some(0xd0),
        '\\' => Some(0xe0),
        '[' => Some(match kind {
            CodePageKind::Cp037 => 0xad,
            CodePageKind::Cp1047 => 0x4a,
        }),
        ']' => Some(match kind {
            CodePageKind::Cp037 => 0xbd,
            CodePageKind::Cp1047 => 0x6a,
        }),
        '¢' => Some(match kind {
            CodePageKind::Cp037 => 0x4a,
            CodePageKind::Cp1047 => 0xad,
        }),
        '¦' => Some(match kind {
            CodePageKind::Cp037 => 0x6a,
            CodePageKind::Cp1047 => 0xbd,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp037_round_trips_common_text() {
        let cp = codepage(CodePageKind::Cp037);
        let text = "READY USER01 DATA.SET(ABC)?";
        assert_eq!(cp.decode(&cp.encode(text)), text);
    }

    #[test]
    fn cp037_and_cp1047_brackets_differ() {
        let cp037 = codepage(CodePageKind::Cp037);
        let cp1047 = codepage(CodePageKind::Cp1047);
        assert_eq!(cp037.encode_char('['), Some(0xad));
        assert_eq!(cp1047.encode_char('['), Some(0x4a));
        assert_eq!(cp037.decode_byte(0xad), '[');
        assert_eq!(cp1047.decode_byte(0x4a), '[');
    }
}
