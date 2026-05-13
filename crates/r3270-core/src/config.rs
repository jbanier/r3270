use serde::{Deserialize, Serialize};

use crate::codepage::CodePageKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalModel {
    Model2,
    Model3,
    Model4,
    Model5,
    Dynamic { rows: u16, cols: u16 },
}

impl Default for TerminalModel {
    fn default() -> Self {
        Self::Model2
    }
}

impl TerminalModel {
    pub fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Model2 => (24, 80),
            Self::Model3 => (32, 80),
            Self::Model4 => (43, 80),
            Self::Model5 => (27, 132),
            Self::Dynamic { rows, cols } => (rows, cols),
        }
    }

    pub fn device_type(self) -> &'static str {
        match self {
            Self::Model2 => "IBM-3278-2-E",
            Self::Model3 => "IBM-3278-3-E",
            Self::Model4 => "IBM-3278-4-E",
            Self::Model5 => "IBM-3278-5-E",
            Self::Dynamic { .. } => "IBM-DYNAMIC",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub insecure_skip_verify: bool,
    pub lu_name: Option<String>,
    pub terminal_model: TerminalModel,
    pub codepage: CodePageKind,
    pub theme: String,
}

impl Default for HostProfile {
    fn default() -> Self {
        Self {
            name: "New host".to_owned(),
            host: "localhost".to_owned(),
            port: 23,
            tls: false,
            insecure_skip_verify: false,
            lu_name: None,
            terminal_model: TerminalModel::default(),
            codepage: CodePageKind::Cp037,
            theme: "green".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub profile: HostProfile,
    pub connect_timeout_secs: u64,
    pub keepalive_secs: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            profile: HostProfile::default(),
            connect_timeout_secs: 15,
            keepalive_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSpec(pub String);

impl ColorSpec {
    pub fn rgb(&self) -> Option<(u8, u8, u8)> {
        let value = self.0.strip_prefix('#').unwrap_or(&self.0);
        if value.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&value[0..2], 16).ok()?;
        let g = u8::from_str_radix(&value[2..4], 16).ok()?;
        let b = u8::from_str_radix(&value[4..6], 16).ok()?;
        Some((r, g, b))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
    pub background: ColorSpec,
    pub foreground: ColorSpec,
    pub cursor: ColorSpec,
    pub selection: ColorSpec,
    pub black: ColorSpec,
    pub blue: ColorSpec,
    pub red: ColorSpec,
    pub pink: ColorSpec,
    pub green: ColorSpec,
    pub turquoise: ColorSpec,
    pub yellow: ColorSpec,
    pub white: ColorSpec,
}

impl ThemeConfig {
    pub fn built_ins() -> Vec<Self> {
        vec![
            Self {
                name: "green".to_owned(),
                background: ColorSpec("#06140c".to_owned()),
                foreground: ColorSpec("#74ff9b".to_owned()),
                cursor: ColorSpec("#d8ffd8".to_owned()),
                selection: ColorSpec("#274b35".to_owned()),
                black: ColorSpec("#06140c".to_owned()),
                blue: ColorSpec("#60a5fa".to_owned()),
                red: ColorSpec("#fb7185".to_owned()),
                pink: ColorSpec("#f0abfc".to_owned()),
                green: ColorSpec("#74ff9b".to_owned()),
                turquoise: ColorSpec("#5eead4".to_owned()),
                yellow: ColorSpec("#fde68a".to_owned()),
                white: ColorSpec("#ecfdf5".to_owned()),
            },
            Self {
                name: "dark".to_owned(),
                background: ColorSpec("#111318".to_owned()),
                foreground: ColorSpec("#d6d9df".to_owned()),
                cursor: ColorSpec("#f8fafc".to_owned()),
                selection: ColorSpec("#293142".to_owned()),
                black: ColorSpec("#111318".to_owned()),
                blue: ColorSpec("#7aa2f7".to_owned()),
                red: ColorSpec("#f7768e".to_owned()),
                pink: ColorSpec("#bb9af7".to_owned()),
                green: ColorSpec("#9ece6a".to_owned()),
                turquoise: ColorSpec("#7dcfff".to_owned()),
                yellow: ColorSpec("#e0af68".to_owned()),
                white: ColorSpec("#c0caf5".to_owned()),
            },
            Self {
                name: "light".to_owned(),
                background: ColorSpec("#f8fafc".to_owned()),
                foreground: ColorSpec("#1f2937".to_owned()),
                cursor: ColorSpec("#111827".to_owned()),
                selection: ColorSpec("#dbeafe".to_owned()),
                black: ColorSpec("#111827".to_owned()),
                blue: ColorSpec("#2563eb".to_owned()),
                red: ColorSpec("#dc2626".to_owned()),
                pink: ColorSpec("#c026d3".to_owned()),
                green: ColorSpec("#15803d".to_owned()),
                turquoise: ColorSpec("#0891b2".to_owned()),
                yellow: ColorSpec("#a16207".to_owned()),
                white: ColorSpec("#ffffff".to_owned()),
            },
            Self {
                name: "high-contrast".to_owned(),
                background: ColorSpec("#000000".to_owned()),
                foreground: ColorSpec("#ffffff".to_owned()),
                cursor: ColorSpec("#ffff00".to_owned()),
                selection: ColorSpec("#0044aa".to_owned()),
                black: ColorSpec("#000000".to_owned()),
                blue: ColorSpec("#00a2ff".to_owned()),
                red: ColorSpec("#ff3b30".to_owned()),
                pink: ColorSpec("#ff66ff".to_owned()),
                green: ColorSpec("#00ff66".to_owned()),
                turquoise: ColorSpec("#00ffff".to_owned()),
                yellow: ColorSpec("#ffff00".to_owned()),
                white: ColorSpec("#ffffff".to_owned()),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMap {
    pub enter: String,
    pub clear: String,
    pub attn: String,
    pub sysreq: String,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            enter: "Enter".to_owned(),
            clear: "Ctrl+L".to_owned(),
            attn: "Esc".to_owned(),
            sysreq: "Ctrl+S".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub local_path: String,
    pub host_path: String,
    pub direction: String,
    pub mode: String,
}
