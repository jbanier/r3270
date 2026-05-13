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
}
