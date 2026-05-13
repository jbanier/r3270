use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use r3270_core::events::{ConnectionState, HostEvent, OiaState};
use r3270_core::presentation::{AidKey, PresentationSpace};
use r3270_core::telnet::{TelnetEvent, TelnetNegotiator, TelnetParser, frame_record};
use r3270_core::tn3270e::{DataType, Tn3270eHeader};
use r3270_core::{SessionConfig, TerminalModel};
use rustls::RootCertStore;
use rustls_pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use tracing::{debug, warn};

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> AsyncReadWrite for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

pub type SessionStream = Pin<Box<dyn AsyncReadWrite>>;

#[derive(Debug, Clone)]
pub enum SessionCommand {
    InputText(String),
    SendAid(AidKey),
    MoveCursor { row: u16, col: u16 },
    Raw3270(Vec<u8>),
    Disconnect,
}

pub async fn run_session(
    config: SessionConfig,
    mut commands: mpsc::UnboundedReceiver<SessionCommand>,
    events: mpsc::UnboundedSender<HostEvent>,
) -> Result<()> {
    let profile = config.profile.clone();
    send_event(&events, HostEvent::Connection(ConnectionState::Connecting));
    let mut stream = connect_stream(&config)
        .await
        .with_context(|| format!("connect to {}:{}", profile.host, profile.port))?;

    let mut parser = TelnetParser::new();
    let mut negotiator = TelnetNegotiator::new(
        profile.terminal_model.device_type(),
        profile.lu_name.clone(),
    );
    let mut screen = PresentationSpace::new(profile.terminal_model, profile.codepage);
    let mut record = Vec::new();
    let mut sequence = 0u16;
    let mut mode = ConnectionState::Nvt;
    send_oia(&events, mode, &screen, "Connected");

    let mut read_buf = [0_u8; 8192];
    loop {
        tokio::select! {
            read = stream.read(&mut read_buf) => {
                let read = read?;
                if read == 0 {
                    send_event(&events, HostEvent::Disconnected);
                    return Ok(());
                }
                for event in parser.feed(&read_buf[..read]) {
                    for response in negotiator.handle(&event) {
                        stream.write_all(&response).await?;
                    }
                    let new_mode = match negotiator.mode {
                        r3270_core::telnet::NegotiatedMode::Nvt => ConnectionState::Nvt,
                        r3270_core::telnet::NegotiatedMode::Tn3270 => ConnectionState::Tn3270,
                        r3270_core::telnet::NegotiatedMode::Tn3270e => ConnectionState::Tn3270e,
                    };
                    if new_mode != mode {
                        mode = new_mode;
                        send_oia(&events, mode, &screen, "Ready");
                    }
                    match event {
                        TelnetEvent::Data(data) => record.extend_from_slice(&data),
                        TelnetEvent::EndOfRecord => {
                            handle_record(&mut screen, &record, mode, &events, &mut stream, &mut sequence).await?;
                            record.clear();
                            send_oia(&events, mode, &screen, "Ready");
                        }
                        TelnetEvent::Command { .. } => {}
                    }
                }
            }
            command = commands.recv() => {
                let Some(command) = command else {
                    return Ok(());
                };
                match command {
                    SessionCommand::Disconnect => return Ok(()),
                    SessionCommand::MoveCursor { row, col } => {
                        screen.set_cursor(row, col);
                        send_event(&events, HostEvent::Screen(screen.clone()));
                    }
                    SessionCommand::InputText(text) => {
                        for ch in text.chars() {
                            screen.input_char(ch);
                        }
                        send_event(&events, HostEvent::Screen(screen.clone()));
                    }
                    SessionCommand::SendAid(aid) => {
                        let payload = screen.aid_response(aid);
                        write_3270_record(&mut stream, mode, &payload, &mut sequence).await?;
                        send_event(&events, HostEvent::Screen(screen.clone()));
                        send_oia(&events, mode, &screen, "Waiting for host");
                    }
                    SessionCommand::Raw3270(payload) => {
                        write_3270_record(&mut stream, mode, &payload, &mut sequence).await?;
                    }
                }
            }
        }
    }
}

async fn connect_stream(config: &SessionConfig) -> Result<SessionStream> {
    if config.profile.insecure_skip_verify {
        return Err(anyhow!(
            "insecure TLS mode is explicit in config but is not implemented yet"
        ));
    }
    let addr = format!("{}:{}", config.profile.host, config.profile.port);
    let tcp = timeout(
        Duration::from_secs(config.connect_timeout_secs),
        TcpStream::connect(&addr),
    )
    .await
    .context("connection timed out")??;
    tcp.set_nodelay(true)?;
    if !config.profile.tls {
        return Ok(Box::pin(tcp));
    }

    let root_store = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(tls_config));
    let server_name = ServerName::try_from(config.profile.host.clone())?;
    let tls_stream = connector.connect(server_name, tcp).await?;
    Ok(Box::pin(tls_stream))
}

async fn handle_record(
    screen: &mut PresentationSpace,
    record: &[u8],
    mode: ConnectionState,
    events: &mpsc::UnboundedSender<HostEvent>,
    stream: &mut SessionStream,
    sequence: &mut u16,
) -> Result<()> {
    if record.is_empty() {
        return Ok(());
    }
    let payload = if mode == ConnectionState::Tn3270e {
        let (header, payload) =
            Tn3270eHeader::parse(record).map_err(|err| anyhow!("invalid TN3270E header: {err}"))?;
        match header.data_type {
            DataType::Data3270 => payload,
            DataType::NvtData | DataType::SscpLuData => {
                debug!(
                    "received non-display TN3270E data type {:?}",
                    header.data_type
                );
                return Ok(());
            }
            other => {
                debug!("ignored TN3270E data type {:?}", other);
                return Ok(());
            }
        }
    } else {
        record
    };

    match screen.apply_host_data(payload) {
        Ok(Some(response)) => {
            write_3270_record(stream, mode, &response, sequence).await?;
            send_event(events, HostEvent::Screen(screen.clone()));
        }
        Ok(None) => send_event(events, HostEvent::Screen(screen.clone())),
        Err(err) => {
            warn!("3270 parser error: {err}");
            send_event(events, HostEvent::ProtocolError(err.to_string()));
        }
    }
    Ok(())
}

async fn write_3270_record(
    stream: &mut SessionStream,
    mode: ConnectionState,
    payload: &[u8],
    sequence: &mut u16,
) -> Result<()> {
    let record = if mode == ConnectionState::Tn3270e {
        let header = Tn3270eHeader::new(DataType::Data3270, *sequence);
        *sequence = sequence.wrapping_add(1);
        header.encode(payload)
    } else {
        payload.to_vec()
    };
    stream.write_all(&frame_record(&record)).await?;
    stream.flush().await?;
    Ok(())
}

fn send_event(events: &mpsc::UnboundedSender<HostEvent>, event: HostEvent) {
    let _ = events.send(event);
}

fn send_oia(
    events: &mpsc::UnboundedSender<HostEvent>,
    connection: ConnectionState,
    screen: &PresentationSpace,
    message: impl Into<String>,
) {
    let _ = events.send(HostEvent::Oia(OiaState {
        connection,
        keyboard_locked: screen.keyboard_locked(),
        insert_mode: false,
        message: message.into(),
    }));
}

pub fn default_session_config(host: impl Into<String>, port: u16) -> SessionConfig {
    let mut config = SessionConfig::default();
    config.profile.host = host.into();
    config.profile.port = port;
    config.profile.terminal_model = TerminalModel::Model2;
    config
}
