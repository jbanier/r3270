use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransferError {
    #[error("host file name is required")]
    MissingHostFile,
    #[error("local file name is required")]
    MissingLocalFile,
    #[error("unsupported host kind for command builder")]
    UnsupportedHostKind,
    #[error("DFT frame is shorter than header")]
    ShortDftFrame,
    #[error("DFT frame length mismatch")]
    InvalidDftLength,
    #[error("host transfer error: {0}")]
    HostError(String),
    #[error("local file IO failed: {0}")]
    LocalIo(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    SendToHost,
    ReceiveFromHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferMode {
    Text,
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostKind {
    Tso,
    Cics,
    Vm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordFormat {
    Fixed,
    Variable,
    Undefined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferOptions {
    pub append: bool,
    pub crlf: bool,
    pub record_format: Option<RecordFormat>,
    pub logical_record_length: Option<u16>,
    pub block_size: Option<u16>,
    pub primary_space: Option<u16>,
    pub secondary_space: Option<u16>,
    pub dft_buffer_size: u16,
    pub extra_options: Vec<String>,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            append: false,
            crlf: true,
            record_format: None,
            logical_record_length: None,
            block_size: None,
            primary_space: None,
            secondary_space: None,
            dft_buffer_size: 4096,
            extra_options: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndFileRequest {
    pub local_file: String,
    pub host_file: String,
    pub direction: TransferDirection,
    pub mode: TransferMode,
    pub host_kind: HostKind,
    pub options: TransferOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DftDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DftStatus {
    Started,
    Running,
    Finished,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DftProgress {
    pub direction: DftDirection,
    pub status: DftStatus,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DftFrame {
    pub request_id: u16,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Default for IndFileRequest {
    fn default() -> Self {
        Self {
            local_file: String::new(),
            host_file: String::new(),
            direction: TransferDirection::ReceiveFromHost,
            mode: TransferMode::Text,
            host_kind: HostKind::Tso,
            options: TransferOptions::default(),
        }
    }
}

impl IndFileRequest {
    pub fn tso_command(&self) -> Result<String, TransferError> {
        if self.host_kind != HostKind::Tso {
            return Err(TransferError::UnsupportedHostKind);
        }
        if self.host_file.trim().is_empty() {
            return Err(TransferError::MissingHostFile);
        }
        if self.local_file.trim().is_empty() {
            return Err(TransferError::MissingLocalFile);
        }

        let direction = match self.direction {
            TransferDirection::SendToHost => "PUT",
            TransferDirection::ReceiveFromHost => "GET",
        };
        let mut parts = vec![
            "IND$FILE".to_owned(),
            direction.to_owned(),
            quote_host_file(&self.host_file),
        ];

        if self.mode == TransferMode::Text {
            parts.push("ASCII".to_owned());
            if self.options.crlf {
                parts.push("CRLF".to_owned());
            }
        }

        if self.options.append {
            parts.push("APPEND".to_owned());
        }
        if let Some(format) = self.options.record_format {
            let value = match format {
                RecordFormat::Fixed => "F",
                RecordFormat::Variable => "V",
                RecordFormat::Undefined => "U",
            };
            parts.push(format!("RECFM({value})"));
        }
        if let Some(lrecl) = self.options.logical_record_length {
            parts.push(format!("LRECL({lrecl})"));
        }
        if let Some(block_size) = self.options.block_size {
            parts.push(format!("BLKSIZE({block_size})"));
        }
        if let Some(primary) = self.options.primary_space {
            if let Some(secondary) = self.options.secondary_space {
                parts.push(format!("SPACE({primary},{secondary})"));
            } else {
                parts.push(format!("SPACE({primary})"));
            }
        }
        parts.extend(self.options.extra_options.iter().cloned());
        Ok(parts.join(" "))
    }

    pub fn dft_direction(&self) -> DftDirection {
        match self.direction {
            TransferDirection::SendToHost => DftDirection::Upload,
            TransferDirection::ReceiveFromHost => DftDirection::Download,
        }
    }
}

impl DftFrame {
    pub fn new(request_id: u16, flags: u8, payload: Vec<u8>) -> Self {
        Self {
            request_id,
            flags,
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let length = self.payload.len() + 5;
        let mut out = Vec::with_capacity(length);
        out.extend_from_slice(&(length as u16).to_be_bytes());
        out.extend_from_slice(&self.request_id.to_be_bytes());
        out.push(self.flags);
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn decode(input: &[u8]) -> Result<Self, TransferError> {
        if input.len() < 5 {
            return Err(TransferError::ShortDftFrame);
        }
        let length = u16::from_be_bytes([input[0], input[1]]) as usize;
        if length != input.len() {
            return Err(TransferError::InvalidDftLength);
        }
        Ok(Self {
            request_id: u16::from_be_bytes([input[2], input[3]]),
            flags: input[4],
            payload: input[5..].to_vec(),
        })
    }
}

pub fn prepare_upload_payload(bytes: &[u8], mode: TransferMode, crlf: bool) -> Vec<u8> {
    if mode == TransferMode::Binary {
        return bytes.to_vec();
    }
    let text = String::from_utf8_lossy(bytes);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if crlf {
        normalized.replace('\n', "\r\n").into_bytes()
    } else {
        normalized.into_bytes()
    }
}

pub fn parse_host_message(
    message: &str,
    direction: DftDirection,
) -> Result<DftProgress, TransferError> {
    let upper = message.to_ascii_uppercase();
    if upper.contains("ERROR") || upper.contains("FAILED") {
        return Err(TransferError::HostError(message.trim().to_owned()));
    }
    let status = if upper.contains("COMPLETE") || upper.contains("SUCCESS") {
        DftStatus::Finished
    } else if upper.contains("CANCEL") {
        DftStatus::Cancelled
    } else if upper.contains("START") {
        DftStatus::Started
    } else {
        DftStatus::Running
    };
    let bytes_done = upper
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .next()
        .and_then(|part| part.parse().ok())
        .unwrap_or(0);
    Ok(DftProgress {
        direction,
        status,
        bytes_done,
        bytes_total: None,
        message: message.trim().to_owned(),
    })
}

fn quote_host_file(host_file: &str) -> String {
    let trimmed = host_file.trim();
    if trimmed.starts_with('\'') || trimmed.contains('(') || trimmed.contains(' ') {
        trimmed.to_owned()
    } else {
        format!("'{trimmed}'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_tso_receive_text_command() {
        let request = IndFileRequest {
            local_file: "pc.txt".to_owned(),
            host_file: "HLQ.DATA".to_owned(),
            direction: TransferDirection::ReceiveFromHost,
            mode: TransferMode::Text,
            host_kind: HostKind::Tso,
            options: TransferOptions::default(),
        };
        assert_eq!(
            request.tso_command().unwrap(),
            "IND$FILE GET 'HLQ.DATA' ASCII CRLF"
        );
    }

    #[test]
    fn builds_tso_send_binary_with_allocation() {
        let request = IndFileRequest {
            local_file: "data.bin".to_owned(),
            host_file: "HLQ.BIN".to_owned(),
            direction: TransferDirection::SendToHost,
            mode: TransferMode::Binary,
            host_kind: HostKind::Tso,
            options: TransferOptions {
                record_format: Some(RecordFormat::Fixed),
                logical_record_length: Some(80),
                block_size: Some(3120),
                primary_space: Some(5),
                secondary_space: Some(1),
                ..TransferOptions::default()
            },
        };
        assert_eq!(
            request.tso_command().unwrap(),
            "IND$FILE PUT 'HLQ.BIN' RECFM(F) LRECL(80) BLKSIZE(3120) SPACE(5,1)"
        );
    }

    #[test]
    fn dft_frame_round_trips() {
        let frame = DftFrame::new(7, 0x80, b"payload".to_vec());
        assert_eq!(DftFrame::decode(&frame.encode()).unwrap(), frame);
    }

    #[test]
    fn text_payload_normalizes_crlf() {
        assert_eq!(
            prepare_upload_payload(b"a\nb\rc\r\nd", TransferMode::Text, true),
            b"a\r\nb\r\nc\r\nd"
        );
    }

    #[test]
    fn host_message_reports_errors() {
        assert!(parse_host_message("ERROR 12 failed", DftDirection::Download).is_err());
        let progress = parse_host_message("4096 bytes complete", DftDirection::Download).unwrap();
        assert_eq!(progress.status, DftStatus::Finished);
        assert_eq!(progress.bytes_done, 4096);
    }
}
