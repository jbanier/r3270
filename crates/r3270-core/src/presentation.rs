use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::codepage::{CodePage, CodePageKind, codepage};
use crate::config::TerminalModel;

pub const CMD_WRITE: u8 = 0xf1;
pub const CMD_WRITE_ALT: u8 = 0x01;
pub const CMD_READ_BUFFER: u8 = 0xf2;
pub const CMD_READ_BUFFER_ALT: u8 = 0x02;
pub const CMD_WRITE_STRUCTURED_FIELD: u8 = 0xf3;
pub const CMD_WRITE_STRUCTURED_FIELD_ALT: u8 = 0x03;
pub const CMD_ERASE_WRITE: u8 = 0xf5;
pub const CMD_ERASE_WRITE_ALT: u8 = 0x05;
pub const CMD_READ_MODIFIED: u8 = 0xf6;
pub const CMD_READ_MODIFIED_ALT: u8 = 0x06;
pub const CMD_ERASE_WRITE_ALTERNATE: u8 = 0x7e;
pub const CMD_READ_MODIFIED_ALL: u8 = 0x6e;
pub const CMD_ERASE_ALL_UNPROTECTED: u8 = 0x6f;

pub const ORDER_SBA: u8 = 0x11;
pub const ORDER_EUA: u8 = 0x12;
pub const ORDER_IC: u8 = 0x13;
pub const ORDER_SF: u8 = 0x1d;
pub const ORDER_SA: u8 = 0x28;
pub const ORDER_SFE: u8 = 0x29;
pub const ORDER_MF: u8 = 0x2c;
pub const ORDER_RA: u8 = 0x3c;
pub const ORDER_PT: u8 = 0x05;

pub const EXT_ALL: u8 = 0x00;
pub const EXT_FIELD_ATTRIBUTE: u8 = 0xc0;
pub const EXT_HIGHLIGHTING: u8 = 0x41;
pub const EXT_FOREGROUND: u8 = 0x42;
pub const EXT_BACKGROUND: u8 = 0x45;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PresentationError {
    #[error("empty 3270 data stream")]
    EmptyStream,
    #[error("truncated 3270 order {order:#04x}")]
    TruncatedOrder { order: u8 },
    #[error("unsupported 3270 command {0:#04x}")]
    UnsupportedCommand(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
}

impl Default for Cursor {
    fn default() -> Self {
        Self { row: 0, col: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Default,
    Black,
    Blue,
    Red,
    Pink,
    Green,
    Turquoise,
    Yellow,
    White,
}

impl Color {
    pub fn from_3270(value: u8) -> Self {
        match value {
            0xf0 => Self::Default,
            0xf1 => Self::Blue,
            0xf2 => Self::Red,
            0xf3 => Self::Pink,
            0xf4 => Self::Green,
            0xf5 => Self::Turquoise,
            0xf6 => Self::Yellow,
            0xf7 => Self::White,
            0xf8 => Self::Black,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Highlight {
    Default,
    Normal,
    Blink,
    Reverse,
    Underscore,
    Intensify,
}

impl Highlight {
    pub fn from_3270(value: u8) -> Self {
        match value {
            0xf0 => Self::Default,
            0xf1 => Self::Normal,
            0xf2 => Self::Blink,
            0xf4 => Self::Reverse,
            0xf8 => Self::Underscore,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellAttrs {
    pub protected: bool,
    pub numeric: bool,
    pub intensified: bool,
    pub nondisplay: bool,
    pub modified: bool,
    pub foreground: Color,
    pub background: Color,
    pub highlight: Highlight,
}

impl Default for CellAttrs {
    fn default() -> Self {
        Self {
            protected: false,
            numeric: false,
            intensified: false,
            nondisplay: false,
            modified: false,
            foreground: Color::Default,
            background: Color::Default,
            highlight: Highlight::Default,
        }
    }
}

impl CellAttrs {
    pub fn from_field_attribute(attribute: u8) -> Self {
        let display = (attribute >> 2) & 0x03;
        Self {
            protected: attribute & 0x20 != 0,
            numeric: attribute & 0x10 != 0,
            intensified: display == 0x02,
            nondisplay: display == 0x03,
            modified: attribute & 0x01 != 0,
            ..Self::default()
        }
    }

    fn apply_extended(&mut self, attr_type: u8, value: u8) {
        match attr_type {
            EXT_FIELD_ATTRIBUTE => *self = Self::from_field_attribute(value),
            EXT_FOREGROUND => self.foreground = Color::from_3270(value),
            EXT_BACKGROUND => self.background = Color::from_3270(value),
            EXT_HIGHLIGHTING => self.highlight = Highlight::from_3270(value),
            EXT_ALL => *self = Self::default(),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub ch: char,
    pub ebcdic: u8,
    pub attrs: CellAttrs,
    pub field_attribute: Option<u8>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            ebcdic: 0x40,
            attrs: CellAttrs::default(),
            field_attribute: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub start: usize,
    pub end: usize,
    pub attrs: CellAttrs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AidKey {
    Enter,
    Clear,
    Attn,
    SysReq,
    Pa(u8),
    Pf(u8),
}

impl AidKey {
    pub fn code(self) -> u8 {
        match self {
            Self::Enter => 0x7d,
            Self::Clear => 0x6d,
            Self::Attn => 0x7d,
            Self::SysReq => 0xf0,
            Self::Pa(1) => 0x6c,
            Self::Pa(2) => 0x6e,
            Self::Pa(3) => 0x6b,
            Self::Pa(_) => 0x6c,
            Self::Pf(n @ 1..=9) => 0xf0 + n,
            Self::Pf(10) => 0x7a,
            Self::Pf(11) => 0x7b,
            Self::Pf(12) => 0x7c,
            Self::Pf(n @ 13..=21) => 0xc0 + (n - 12),
            Self::Pf(22) => 0x4a,
            Self::Pf(23) => 0x4b,
            Self::Pf(24) => 0x4c,
            Self::Pf(_) => 0x7d,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationSpace {
    rows: u16,
    cols: u16,
    cells: Vec<Cell>,
    cursor_addr: usize,
    write_addr: usize,
    codepage: CodePageKind,
    keyboard_locked: bool,
    current_attrs: CellAttrs,
    alarm: bool,
}

impl PresentationSpace {
    pub fn new(model: TerminalModel, codepage: CodePageKind) -> Self {
        let (rows, cols) = model.dimensions();
        Self::with_dimensions(rows, cols, codepage)
    }

    pub fn with_dimensions(rows: u16, cols: u16, codepage: CodePageKind) -> Self {
        Self {
            rows,
            cols,
            cells: vec![Cell::default(); rows as usize * cols as usize],
            cursor_addr: 0,
            write_addr: 0,
            codepage,
            keyboard_locked: false,
            current_attrs: CellAttrs::default(),
            alarm: false,
        }
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cursor(&self) -> Cursor {
        self.addr_to_cursor(self.cursor_addr)
    }

    pub fn keyboard_locked(&self) -> bool {
        self.keyboard_locked
    }

    pub fn alarm(&self) -> bool {
        self.alarm
    }

    pub fn clear_alarm(&mut self) {
        self.alarm = false;
    }

    pub fn set_cursor(&mut self, row: u16, col: u16) {
        self.cursor_addr = self.cursor_to_addr(row, col);
        self.write_addr = self.cursor_addr;
    }

    pub fn text_rows(&self) -> Vec<String> {
        self.cells
            .chunks(self.cols as usize)
            .map(|row| {
                row.iter()
                    .map(|cell| if cell.attrs.nondisplay { ' ' } else { cell.ch })
                    .collect()
            })
            .collect()
    }

    pub fn fields(&self) -> Vec<Field> {
        let mut starts: Vec<usize> = self
            .cells
            .iter()
            .enumerate()
            .filter_map(|(idx, cell)| cell.field_attribute.map(|_| idx))
            .collect();
        if starts.is_empty() {
            return vec![Field {
                start: 0,
                end: self.cells.len() - 1,
                attrs: CellAttrs::default(),
            }];
        }
        starts.sort_unstable();
        let mut fields = Vec::with_capacity(starts.len());
        for (idx, start) in starts.iter().enumerate() {
            let next = starts[(idx + 1) % starts.len()];
            let end = if next == 0 {
                self.cells.len() - 1
            } else if next > *start {
                next - 1
            } else {
                self.cells.len() - 1
            };
            fields.push(Field {
                start: *start,
                end,
                attrs: self.cells[*start].attrs,
            });
        }
        fields
    }

    pub fn apply_host_data(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, PresentationError> {
        let Some((&command, body)) = data.split_first() else {
            return Err(PresentationError::EmptyStream);
        };
        match command {
            CMD_WRITE | CMD_WRITE_ALT => {
                self.write(body, false)?;
                Ok(None)
            }
            CMD_ERASE_WRITE | CMD_ERASE_WRITE_ALT => {
                self.erase(false);
                self.write(body, false)?;
                Ok(None)
            }
            CMD_ERASE_WRITE_ALTERNATE => {
                self.erase(false);
                self.write(body, false)?;
                Ok(None)
            }
            CMD_ERASE_ALL_UNPROTECTED => {
                self.erase_unprotected_all();
                Ok(None)
            }
            CMD_READ_BUFFER | CMD_READ_BUFFER_ALT => Ok(Some(self.read_buffer())),
            CMD_READ_MODIFIED | CMD_READ_MODIFIED_ALT => {
                Ok(Some(self.read_modified(AidKey::Enter, false)))
            }
            CMD_READ_MODIFIED_ALL => Ok(Some(self.read_modified(AidKey::Enter, true))),
            CMD_WRITE_STRUCTURED_FIELD | CMD_WRITE_STRUCTURED_FIELD_ALT => {
                Ok(Some(self.query_reply()))
            }
            other => Err(PresentationError::UnsupportedCommand(other)),
        }
    }

    pub fn input_char(&mut self, ch: char) -> bool {
        if self.keyboard_locked {
            return false;
        }
        let Some(byte) = codepage(self.codepage).encode_char(ch) else {
            return false;
        };
        if self.protected_at(self.cursor_addr) {
            return false;
        }
        self.write_cell(self.cursor_addr, byte, ch, self.attrs_at(self.cursor_addr));
        self.mark_field_modified(self.cursor_addr);
        self.cursor_addr = self.next_addr(self.cursor_addr);
        self.write_addr = self.cursor_addr;
        true
    }

    pub fn backspace(&mut self) -> bool {
        let prev = if self.cursor_addr == 0 {
            self.cells.len() - 1
        } else {
            self.cursor_addr - 1
        };
        if self.protected_at(prev) {
            return false;
        }
        self.cursor_addr = prev;
        self.write_cell(prev, 0x40, ' ', self.attrs_at(prev));
        self.mark_field_modified(prev);
        true
    }

    pub fn aid_response(&mut self, aid: AidKey) -> Vec<u8> {
        self.keyboard_locked = true;
        match aid {
            AidKey::Clear => vec![aid.code()],
            AidKey::Attn | AidKey::SysReq => vec![aid.code()],
            _ => self.read_modified(aid, false),
        }
    }

    pub fn read_buffer(&self) -> Vec<u8> {
        let mut out = vec![AidKey::Enter.code()];
        out.extend_from_slice(&encode_address(self.cursor_addr));
        for (idx, cell) in self.cells.iter().enumerate() {
            out.push(ORDER_SBA);
            out.extend_from_slice(&encode_address(idx));
            if let Some(attribute) = cell.field_attribute {
                out.push(ORDER_SF);
                out.push(attribute);
            } else {
                out.push(cell.ebcdic);
            }
        }
        out
    }

    pub fn read_modified(&self, aid: AidKey, all_unprotected: bool) -> Vec<u8> {
        let mut out = vec![aid.code()];
        out.extend_from_slice(&encode_address(self.cursor_addr));
        for field in self.fields() {
            if field.attrs.protected {
                continue;
            }
            let modified = self.cells[field.start].attrs.modified;
            if !modified && !all_unprotected {
                continue;
            }
            out.push(ORDER_SBA);
            out.extend_from_slice(&encode_address(self.next_addr(field.start)));
            let mut addr = self.next_addr(field.start);
            while addr != self.next_addr(field.end) {
                if self.cells[addr].field_attribute.is_some() {
                    break;
                }
                out.push(self.cells[addr].ebcdic);
                addr = self.next_addr(addr);
            }
        }
        out
    }

    fn write(&mut self, body: &[u8], reset_mdt: bool) -> Result<(), PresentationError> {
        if body.is_empty() {
            return Ok(());
        }
        let wcc = body[0];
        self.apply_wcc(wcc, reset_mdt);
        self.write_orders(&body[1..])
    }

    fn apply_wcc(&mut self, wcc: u8, reset_mdt: bool) {
        if wcc & 0x04 != 0 {
            self.keyboard_locked = false;
        }
        if wcc & 0x02 != 0 {
            self.alarm = true;
        }
        if reset_mdt || wcc & 0x01 != 0 {
            for cell in &mut self.cells {
                cell.attrs.modified = false;
            }
        }
    }

    fn write_orders(&mut self, data: &[u8]) -> Result<(), PresentationError> {
        let mut idx = 0;
        let cp = codepage(self.codepage);
        while idx < data.len() {
            match data[idx] {
                ORDER_SBA => {
                    let (addr, consumed) = decode_address(&data[idx + 1..])
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_SBA })?;
                    self.write_addr = addr % self.cells.len();
                    idx += 1 + consumed;
                }
                ORDER_SF => {
                    let attr = *data
                        .get(idx + 1)
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_SF })?;
                    self.start_field(attr);
                    idx += 2;
                }
                ORDER_SFE => {
                    let pair_count = *data
                        .get(idx + 1)
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_SFE })?
                        as usize;
                    let needed = idx + 2 + pair_count * 2;
                    if needed > data.len() {
                        return Err(PresentationError::TruncatedOrder { order: ORDER_SFE });
                    }
                    let mut attrs = self.current_attrs;
                    for pair in data[idx + 2..needed].chunks_exact(2) {
                        attrs.apply_extended(pair[0], pair[1]);
                    }
                    self.start_field_with_attrs(0x00, attrs);
                    idx = needed;
                }
                ORDER_SA => {
                    let attr_type = *data
                        .get(idx + 1)
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_SA })?;
                    let value = *data
                        .get(idx + 2)
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_SA })?;
                    self.current_attrs.apply_extended(attr_type, value);
                    idx += 3;
                }
                ORDER_MF => {
                    let pair_count = *data
                        .get(idx + 1)
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_MF })?
                        as usize;
                    let needed = idx + 2 + pair_count * 2;
                    if needed > data.len() {
                        return Err(PresentationError::TruncatedOrder { order: ORDER_MF });
                    }
                    let mut attrs = self.attrs_at(self.write_addr);
                    for pair in data[idx + 2..needed].chunks_exact(2) {
                        attrs.apply_extended(pair[0], pair[1]);
                    }
                    if let Some(cell) = self.cells.get_mut(self.write_addr) {
                        cell.attrs = attrs;
                    }
                    idx = needed;
                }
                ORDER_IC => {
                    self.cursor_addr = self.write_addr;
                    idx += 1;
                }
                ORDER_PT => {
                    self.write_addr = self.next_unprotected_field(self.write_addr);
                    idx += 1;
                }
                ORDER_RA => {
                    let (addr, consumed) = decode_address(&data[idx + 1..])
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_RA })?;
                    let byte = *data
                        .get(idx + 1 + consumed)
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_RA })?;
                    let ch = cp.decode_byte(byte);
                    while self.write_addr != addr % self.cells.len() {
                        self.write_cell(self.write_addr, byte, ch, self.current_attrs);
                        self.write_addr = self.next_addr(self.write_addr);
                    }
                    idx += 2 + consumed;
                }
                ORDER_EUA => {
                    let (addr, consumed) = decode_address(&data[idx + 1..])
                        .ok_or(PresentationError::TruncatedOrder { order: ORDER_EUA })?;
                    self.erase_unprotected_to(addr % self.cells.len());
                    idx += 1 + consumed;
                }
                byte if byte < 0x40 => {
                    idx += 1;
                }
                byte => {
                    let ch = cp.decode_byte(byte);
                    self.write_cell(self.write_addr, byte, ch, self.current_attrs);
                    self.write_addr = self.next_addr(self.write_addr);
                    idx += 1;
                }
            }
        }
        Ok(())
    }

    fn start_field(&mut self, field_attribute: u8) {
        let attrs = CellAttrs::from_field_attribute(field_attribute);
        self.start_field_with_attrs(field_attribute, attrs);
    }

    fn start_field_with_attrs(&mut self, field_attribute: u8, mut attrs: CellAttrs) {
        attrs.modified = attrs.modified || field_attribute & 0x01 != 0;
        if let Some(cell) = self.cells.get_mut(self.write_addr) {
            *cell = Cell {
                ch: ' ',
                ebcdic: 0x40,
                attrs,
                field_attribute: Some(field_attribute),
            };
        }
        self.current_attrs = attrs;
        self.write_addr = self.next_addr(self.write_addr);
    }

    fn write_cell(&mut self, addr: usize, ebcdic: u8, ch: char, attrs: CellAttrs) {
        if let Some(cell) = self.cells.get_mut(addr) {
            *cell = Cell {
                ch,
                ebcdic,
                attrs,
                field_attribute: None,
            };
        }
    }

    fn erase(&mut self, preserve_size: bool) {
        let len = self.cells.len();
        self.cells.clear();
        self.cells.resize(len, Cell::default());
        self.write_addr = 0;
        self.cursor_addr = 0;
        self.current_attrs = CellAttrs::default();
        if !preserve_size {
            self.keyboard_locked = true;
        }
    }

    fn erase_unprotected_all(&mut self) {
        for addr in 0..self.cells.len() {
            if !self.protected_at(addr) && self.cells[addr].field_attribute.is_none() {
                let attrs = self.attrs_at(addr);
                self.write_cell(addr, 0x40, ' ', attrs);
            }
        }
    }

    fn erase_unprotected_to(&mut self, end: usize) {
        while self.write_addr != end {
            if !self.protected_at(self.write_addr)
                && self.cells[self.write_addr].field_attribute.is_none()
            {
                let attrs = self.attrs_at(self.write_addr);
                self.write_cell(self.write_addr, 0x40, ' ', attrs);
            }
            self.write_addr = self.next_addr(self.write_addr);
        }
    }

    fn query_reply(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(0x88);
        out.push(0x00);
        out.push(0x13);
        out.push(0x81);
        out.push(0x80);
        out.extend_from_slice(&self.cols.to_be_bytes());
        out.extend_from_slice(&self.rows.to_be_bytes());
        out.push(0x86);
        out.push(0x00);
        out.push(0x08);
        out.push(0x87);
        out.push(0x00);
        out.push(0x08);
        out
    }

    fn attrs_at(&self, addr: usize) -> CellAttrs {
        if self.cells[addr].field_attribute.is_some() {
            return self.cells[addr].attrs;
        }
        self.field_for_addr(addr)
            .map(|field| field.attrs)
            .unwrap_or_default()
    }

    fn protected_at(&self, addr: usize) -> bool {
        self.attrs_at(addr).protected
    }

    fn field_for_addr(&self, addr: usize) -> Option<Field> {
        self.fields().into_iter().find(|field| {
            if field.start <= field.end {
                addr >= field.start && addr <= field.end
            } else {
                addr >= field.start || addr <= field.end
            }
        })
    }

    fn mark_field_modified(&mut self, addr: usize) {
        if let Some(field) = self.field_for_addr(addr) {
            if let Some(cell) = self.cells.get_mut(field.start) {
                cell.attrs.modified = true;
            }
        }
    }

    fn next_unprotected_field(&self, from: usize) -> usize {
        let mut addr = self.next_addr(from);
        for _ in 0..self.cells.len() {
            if self.cells[addr].field_attribute.is_some() && !self.cells[addr].attrs.protected {
                return self.next_addr(addr);
            }
            addr = self.next_addr(addr);
        }
        from
    }

    fn next_addr(&self, addr: usize) -> usize {
        (addr + 1) % self.cells.len()
    }

    fn addr_to_cursor(&self, addr: usize) -> Cursor {
        Cursor {
            row: (addr / self.cols as usize) as u16,
            col: (addr % self.cols as usize) as u16,
        }
    }

    fn cursor_to_addr(&self, row: u16, col: u16) -> usize {
        let row = row.min(self.rows.saturating_sub(1));
        let col = col.min(self.cols.saturating_sub(1));
        row as usize * self.cols as usize + col as usize
    }
}

pub fn decode_address(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.len() < 2 {
        return None;
    }
    let first = bytes[0];
    let second = bytes[1];
    if first & 0xc0 == 0 {
        Some(((((first & 0x3f) as usize) << 8) | second as usize, 2))
    } else {
        Some((((first & 0x3f) as usize) << 6 | (second & 0x3f) as usize, 2))
    }
}

pub fn encode_address(addr: usize) -> [u8; 2] {
    [((addr >> 8) & 0x3f) as u8, (addr & 0xff) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> PresentationSpace {
        PresentationSpace::new(TerminalModel::Model2, CodePageKind::Cp037)
    }

    #[test]
    fn erase_write_writes_text_and_unlocks_keyboard() {
        let mut ps = screen();
        ps.apply_host_data(&[CMD_ERASE_WRITE, 0x04, 0xc8, 0xc5, 0xd3, 0xd3, 0xd6])
            .unwrap();
        assert_eq!(&ps.text_rows()[0][0..5], "HELLO");
        assert!(!ps.keyboard_locked());
    }

    #[test]
    fn sba_places_text_at_address() {
        let mut ps = screen();
        let mut data = vec![CMD_ERASE_WRITE, 0x04, ORDER_SBA];
        data.extend_from_slice(&encode_address(82));
        data.extend_from_slice(&[0xc1, 0xc2, 0xc3]);
        ps.apply_host_data(&data).unwrap();
        assert_eq!(&ps.text_rows()[1][2..5], "ABC");
    }

    #[test]
    fn fields_protect_input_and_modified_fields_are_returned() {
        let mut ps = screen();
        ps.apply_host_data(&[
            CMD_ERASE_WRITE,
            0x04,
            ORDER_SF,
            0x20,
            0xc8,
            0xc9,
            ORDER_SF,
            0x00,
            0x40,
            0x40,
        ])
        .unwrap();
        ps.set_cursor(0, 4);
        assert!(ps.input_char('A'));
        let response = ps.read_modified(AidKey::Enter, false);
        assert_eq!(response[0], AidKey::Enter.code());
        assert!(response.contains(&ORDER_SBA));
        assert!(response.contains(&0xc1));
    }

    #[test]
    fn extended_color_is_applied() {
        let mut ps = screen();
        ps.apply_host_data(&[CMD_ERASE_WRITE, 0x04, ORDER_SA, EXT_FOREGROUND, 0xf2, 0xc1])
            .unwrap();
        assert_eq!(ps.cells()[0].attrs.foreground, Color::Red);
    }

    #[test]
    fn address_round_trip_14_bit() {
        let encoded = encode_address(1919);
        assert_eq!(decode_address(&encoded), Some((1919, 2)));
    }
}
