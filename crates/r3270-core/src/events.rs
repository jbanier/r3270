use serde::{Deserialize, Serialize};

use crate::presentation::PresentationSpace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Nvt,
    Tn3270,
    Tn3270e,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OiaState {
    pub connection: ConnectionState,
    pub keyboard_locked: bool,
    pub insert_mode: bool,
    pub message: String,
}

impl Default for OiaState {
    fn default() -> Self {
        Self {
            connection: ConnectionState::Disconnected,
            keyboard_locked: false,
            insert_mode: false,
            message: "Disconnected".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferProgress {
    pub label: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostEvent {
    Connection(ConnectionState),
    Screen(PresentationSpace),
    Oia(OiaState),
    Transfer(TransferProgress),
    ProtocolError(String),
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserAction {
    Connect,
    Disconnect,
    Reconnect,
    InputText(String),
    SendAid(crate::presentation::AidKey),
    MoveCursor { row: u16, col: u16 },
    StartTransfer(crate::config::TransferRequest),
}
