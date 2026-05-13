use serde::{Deserialize, Serialize};

pub const SE: u8 = 240;
pub const NOP: u8 = 241;
pub const BREAK: u8 = 243;
pub const IP: u8 = 244;
pub const AO: u8 = 245;
pub const SB: u8 = 250;
pub const WILL: u8 = 251;
pub const WONT: u8 = 252;
pub const DO: u8 = 253;
pub const DONT: u8 = 254;
pub const IAC: u8 = 255;
pub const EOR: u8 = 239;

pub const OPT_BINARY: u8 = 0;
pub const OPT_ECHO: u8 = 1;
pub const OPT_SUPPRESS_GO_AHEAD: u8 = 3;
pub const OPT_TERMINAL_TYPE: u8 = 24;
pub const OPT_EOR: u8 = 25;
pub const OPT_TN3270E: u8 = 40;

pub const TERMINAL_TYPE_IS: u8 = 0;
pub const TERMINAL_TYPE_SEND: u8 = 1;

pub const TN_ASSOCIATE: u8 = 0;
pub const TN_CONNECT: u8 = 1;
pub const TN_DEVICE_TYPE: u8 = 2;
pub const TN_FUNCTIONS: u8 = 3;
pub const TN_IS: u8 = 4;
pub const TN_REASON: u8 = 5;
pub const TN_REJECT: u8 = 6;
pub const TN_REQUEST: u8 = 7;
pub const TN_SEND: u8 = 8;

pub const TN_FUNC_BIND_IMAGE: u8 = 0;
pub const TN_FUNC_DATA_STREAM_CTL: u8 = 1;
pub const TN_FUNC_RESPONSES: u8 = 2;
pub const TN_FUNC_SCS_CTL_CODES: u8 = 3;
pub const TN_FUNC_SYSREQ: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelnetEvent {
    Data(Vec<u8>),
    EndOfRecord,
    Command {
        verb: u8,
        option: Option<u8>,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Data,
    Iac,
    Verb(u8),
    SbOption,
    Subneg { option: u8, iac_seen: bool },
}

#[derive(Debug, Clone)]
pub struct TelnetParser {
    state: ParserState,
    data_buf: Vec<u8>,
    sb_buf: Vec<u8>,
}

impl Default for TelnetParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TelnetParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Data,
            data_buf: Vec::new(),
            sb_buf: Vec::new(),
        }
    }

    pub fn feed(&mut self, input: &[u8]) -> Vec<TelnetEvent> {
        let mut events = Vec::new();
        for byte in input {
            self.feed_byte(*byte, &mut events);
        }
        self.flush_data(&mut events);
        events
    }

    fn feed_byte(&mut self, byte: u8, events: &mut Vec<TelnetEvent>) {
        match self.state {
            ParserState::Data => {
                if byte == IAC {
                    self.state = ParserState::Iac;
                } else {
                    self.data_buf.push(byte);
                }
            }
            ParserState::Iac => match byte {
                IAC => {
                    self.data_buf.push(IAC);
                    self.state = ParserState::Data;
                }
                EOR => {
                    self.flush_data(events);
                    events.push(TelnetEvent::EndOfRecord);
                    self.state = ParserState::Data;
                }
                SB => {
                    self.flush_data(events);
                    self.state = ParserState::SbOption;
                }
                WILL | WONT | DO | DONT => {
                    self.flush_data(events);
                    self.state = ParserState::Verb(byte);
                }
                verb => {
                    self.flush_data(events);
                    events.push(TelnetEvent::Command {
                        verb,
                        option: None,
                        data: Vec::new(),
                    });
                    self.state = ParserState::Data;
                }
            },
            ParserState::Verb(verb) => {
                events.push(TelnetEvent::Command {
                    verb,
                    option: Some(byte),
                    data: Vec::new(),
                });
                self.state = ParserState::Data;
            }
            ParserState::SbOption => {
                self.sb_buf.clear();
                self.state = ParserState::Subneg {
                    option: byte,
                    iac_seen: false,
                };
            }
            ParserState::Subneg { option, iac_seen } => {
                if iac_seen {
                    match byte {
                        SE => {
                            events.push(TelnetEvent::Command {
                                verb: SB,
                                option: Some(option),
                                data: std::mem::take(&mut self.sb_buf),
                            });
                            self.state = ParserState::Data;
                        }
                        IAC => {
                            self.sb_buf.push(IAC);
                            self.state = ParserState::Subneg {
                                option,
                                iac_seen: false,
                            };
                        }
                        other => {
                            self.sb_buf.push(IAC);
                            self.sb_buf.push(other);
                            self.state = ParserState::Subneg {
                                option,
                                iac_seen: false,
                            };
                        }
                    }
                } else if byte == IAC {
                    self.state = ParserState::Subneg {
                        option,
                        iac_seen: true,
                    };
                } else {
                    self.sb_buf.push(byte);
                }
            }
        }
    }

    fn flush_data(&mut self, events: &mut Vec<TelnetEvent>) {
        if !self.data_buf.is_empty() {
            events.push(TelnetEvent::Data(std::mem::take(&mut self.data_buf)));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiatedMode {
    Nvt,
    Tn3270,
    Tn3270e,
}

#[derive(Debug, Clone)]
pub struct TelnetNegotiator {
    pub terminal_type: String,
    pub lu_name: Option<String>,
    pub mode: NegotiatedMode,
    binary_us: bool,
    binary_them: bool,
    eor_us: bool,
    eor_them: bool,
    tn3270e_us: bool,
    tn3270e_them: bool,
}

impl TelnetNegotiator {
    pub fn new(terminal_type: impl Into<String>, lu_name: Option<String>) -> Self {
        Self {
            terminal_type: terminal_type.into(),
            lu_name,
            mode: NegotiatedMode::Nvt,
            binary_us: false,
            binary_them: false,
            eor_us: false,
            eor_them: false,
            tn3270e_us: false,
            tn3270e_them: false,
        }
    }

    pub fn handle(&mut self, event: &TelnetEvent) -> Vec<Vec<u8>> {
        let TelnetEvent::Command { verb, option, data } = event else {
            return Vec::new();
        };

        match (*verb, *option) {
            (DO, Some(OPT_TN3270E)) => {
                self.tn3270e_us = true;
                self.update_mode();
                vec![command(WILL, OPT_TN3270E)]
            }
            (WILL, Some(OPT_TN3270E)) => {
                self.tn3270e_them = true;
                self.update_mode();
                vec![command(DO, OPT_TN3270E)]
            }
            (DO, Some(OPT_BINARY)) => {
                self.binary_us = true;
                self.update_mode();
                vec![command(WILL, OPT_BINARY)]
            }
            (WILL, Some(OPT_BINARY)) => {
                self.binary_them = true;
                self.update_mode();
                vec![command(DO, OPT_BINARY)]
            }
            (DO, Some(OPT_EOR)) => {
                self.eor_us = true;
                self.update_mode();
                vec![command(WILL, OPT_EOR)]
            }
            (WILL, Some(OPT_EOR)) => {
                self.eor_them = true;
                self.update_mode();
                vec![command(DO, OPT_EOR)]
            }
            (DO, Some(OPT_TERMINAL_TYPE)) => vec![command(WILL, OPT_TERMINAL_TYPE)],
            (DO, Some(OPT_SUPPRESS_GO_AHEAD)) => vec![command(WILL, OPT_SUPPRESS_GO_AHEAD)],
            (WILL, Some(OPT_SUPPRESS_GO_AHEAD)) => vec![command(DO, OPT_SUPPRESS_GO_AHEAD)],
            (DO, Some(option)) => vec![command(WONT, option)],
            (WILL, Some(option)) => vec![command(DONT, option)],
            (SB, Some(OPT_TERMINAL_TYPE)) if data == &[TERMINAL_TYPE_SEND] => {
                vec![subneg(
                    OPT_TERMINAL_TYPE,
                    &[&[TERMINAL_TYPE_IS], self.terminal_type.as_bytes()].concat(),
                )]
            }
            (SB, Some(OPT_TN3270E)) => self.handle_tn3270e_subneg(data),
            _ => Vec::new(),
        }
    }

    fn handle_tn3270e_subneg(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        match data {
            [TN_SEND, TN_DEVICE_TYPE] => {
                let mut payload = vec![TN_DEVICE_TYPE, TN_REQUEST];
                payload.extend_from_slice(self.terminal_type.as_bytes());
                if let Some(lu_name) = &self.lu_name {
                    payload.push(TN_CONNECT);
                    payload.extend_from_slice(lu_name.as_bytes());
                }
                vec![subneg(OPT_TN3270E, &payload)]
            }
            [TN_SEND, TN_FUNCTIONS] => vec![subneg(
                OPT_TN3270E,
                &[
                    TN_FUNCTIONS,
                    TN_REQUEST,
                    TN_FUNC_BIND_IMAGE,
                    TN_FUNC_DATA_STREAM_CTL,
                    TN_FUNC_SYSREQ,
                ],
            )],
            [TN_FUNCTIONS, TN_REQUEST, functions @ ..] => {
                let mut payload = vec![TN_FUNCTIONS, TN_IS];
                payload.extend_from_slice(functions);
                vec![subneg(OPT_TN3270E, &payload)]
            }
            [TN_DEVICE_TYPE, TN_IS, ..] => {
                self.mode = NegotiatedMode::Tn3270e;
                vec![subneg(
                    OPT_TN3270E,
                    &[
                        TN_FUNCTIONS,
                        TN_REQUEST,
                        TN_FUNC_BIND_IMAGE,
                        TN_FUNC_DATA_STREAM_CTL,
                        TN_FUNC_SYSREQ,
                    ],
                )]
            }
            _ => Vec::new(),
        }
    }

    fn update_mode(&mut self) {
        if self.tn3270e_us && self.tn3270e_them {
            self.mode = NegotiatedMode::Tn3270e;
        } else if self.binary_us && self.binary_them && self.eor_us && self.eor_them {
            self.mode = NegotiatedMode::Tn3270;
        }
    }
}

pub fn command(verb: u8, option: u8) -> Vec<u8> {
    vec![IAC, verb, option]
}

pub fn subneg(option: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 5);
    out.extend_from_slice(&[IAC, SB, option]);
    for byte in payload {
        out.push(*byte);
        if *byte == IAC {
            out.push(IAC);
        }
    }
    out.extend_from_slice(&[IAC, SE]);
    out
}

pub fn frame_record(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 2);
    for byte in payload {
        out.push(*byte);
        if *byte == IAC {
            out.push(IAC);
        }
    }
    out.extend_from_slice(&[IAC, EOR]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_handles_iac_escaping_and_eor() {
        let mut parser = TelnetParser::new();
        let events = parser.feed(&[0xc1, IAC, IAC, 0xc2, IAC, EOR]);
        assert_eq!(
            events,
            vec![
                TelnetEvent::Data(vec![0xc1, IAC, 0xc2]),
                TelnetEvent::EndOfRecord
            ]
        );
    }

    #[test]
    fn parser_handles_subnegotiation() {
        let mut parser = TelnetParser::new();
        let events = parser.feed(&[IAC, SB, OPT_TERMINAL_TYPE, TERMINAL_TYPE_SEND, IAC, SE]);
        assert_eq!(
            events,
            vec![TelnetEvent::Command {
                verb: SB,
                option: Some(OPT_TERMINAL_TYPE),
                data: vec![TERMINAL_TYPE_SEND],
            }]
        );
    }

    #[test]
    fn negotiator_sends_terminal_type() {
        let mut negotiator = TelnetNegotiator::new("IBM-3278-2-E", None);
        let response = negotiator.handle(&TelnetEvent::Command {
            verb: SB,
            option: Some(OPT_TERMINAL_TYPE),
            data: vec![TERMINAL_TYPE_SEND],
        });
        assert_eq!(
            response,
            vec![vec![
                IAC,
                SB,
                OPT_TERMINAL_TYPE,
                TERMINAL_TYPE_IS,
                b'I',
                b'B',
                b'M',
                b'-',
                b'3',
                b'2',
                b'7',
                b'8',
                b'-',
                b'2',
                b'-',
                b'E',
                IAC,
                SE
            ]]
        );
    }

    #[test]
    fn negotiator_requests_tn3270e_device_type() {
        let mut negotiator = TelnetNegotiator::new("IBM-3278-2-E", Some("LU123".to_owned()));
        let response = negotiator.handle(&TelnetEvent::Command {
            verb: SB,
            option: Some(OPT_TN3270E),
            data: vec![TN_SEND, TN_DEVICE_TYPE],
        });
        assert_eq!(
            response,
            vec![subneg(OPT_TN3270E, b"\x02\x07IBM-3278-2-E\x01LU123")]
        );
    }
}
