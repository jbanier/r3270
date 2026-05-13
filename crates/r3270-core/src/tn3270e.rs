use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Tn3270eError {
    #[error("TN3270E message shorter than five-byte header")]
    ShortHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Data3270,
    ScsData,
    Response,
    BindImage,
    Unbind,
    NvtData,
    Request,
    SscpLuData,
    PrintEoj,
    Unknown(u8),
}

impl DataType {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => Self::Data3270,
            0x01 => Self::ScsData,
            0x02 => Self::Response,
            0x03 => Self::BindImage,
            0x04 => Self::Unbind,
            0x05 => Self::NvtData,
            0x06 => Self::Request,
            0x07 => Self::SscpLuData,
            0x08 => Self::PrintEoj,
            other => Self::Unknown(other),
        }
    }

    pub fn as_byte(self) -> u8 {
        match self {
            Self::Data3270 => 0x00,
            Self::ScsData => 0x01,
            Self::Response => 0x02,
            Self::BindImage => 0x03,
            Self::Unbind => 0x04,
            Self::NvtData => 0x05,
            Self::Request => 0x06,
            Self::SscpLuData => 0x07,
            Self::PrintEoj => 0x08,
            Self::Unknown(byte) => byte,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tn3270eHeader {
    pub data_type: DataType,
    pub request_flag: u8,
    pub response_flag: u8,
    pub sequence_number: u16,
}

impl Tn3270eHeader {
    pub fn new(data_type: DataType, sequence_number: u16) -> Self {
        Self {
            data_type,
            request_flag: 0,
            response_flag: 0,
            sequence_number,
        }
    }

    pub fn parse(input: &[u8]) -> Result<(Self, &[u8]), Tn3270eError> {
        if input.len() < 5 {
            return Err(Tn3270eError::ShortHeader);
        }
        Ok((
            Self {
                data_type: DataType::from_byte(input[0]),
                request_flag: input[1],
                response_flag: input[2],
                sequence_number: u16::from_be_bytes([input[3], input[4]]),
            },
            &input[5..],
        ))
    }

    pub fn encode(self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 5);
        out.push(self.data_type.as_byte());
        out.push(self.request_flag);
        out.push(self.response_flag);
        out.extend_from_slice(&self.sequence_number.to_be_bytes());
        out.extend_from_slice(data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_and_payload() {
        let bytes = [0x00, 0x00, 0x00, 0x12, 0x34, 0xf5, 0xc1];
        let (header, payload) = Tn3270eHeader::parse(&bytes).unwrap();
        assert_eq!(header.data_type, DataType::Data3270);
        assert_eq!(header.sequence_number, 0x1234);
        assert_eq!(payload, &[0xf5, 0xc1]);
    }

    #[test]
    fn encodes_header() {
        let header = Tn3270eHeader::new(DataType::Data3270, 7);
        assert_eq!(header.encode(&[0xf5]), vec![0, 0, 0, 0, 7, 0xf5]);
    }
}
