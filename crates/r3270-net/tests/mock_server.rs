use std::time::Duration;

use r3270_core::events::{ConnectionState, HostEvent};
use r3270_core::presentation::{AidKey, CMD_ERASE_WRITE, ORDER_SF};
use r3270_core::telnet::{
    DO, EOR, IAC, OPT_BINARY, OPT_EOR, OPT_TERMINAL_TYPE, OPT_TN3270E, TERMINAL_TYPE_SEND,
    TN_DEVICE_TYPE, TN_FUNCTIONS, TN_IS, TN_SEND, WILL, frame_record, subneg,
};
use r3270_core::tn3270e::{DataType, Tn3270eHeader};
use r3270_core::{SessionConfig, TerminalModel};
use r3270_net::{SessionCommand, run_session};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

#[tokio::test]
async fn traditional_tn3270_session_reads_screen_and_sends_aid() {
    let (addr_tx, addr_rx) = oneshot::channel();
    let (aid_tx, aid_rx) = oneshot::channel();
    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        addr_tx.send(listener.local_addr().unwrap()).unwrap();
        let (mut stream, _) = listener.accept().await.unwrap();
        negotiate_traditional(&mut stream).await;
        stream
            .write_all(&frame_record(&erase_write_login_screen()))
            .await
            .unwrap();
        let record = read_telnet_record(&mut stream).await;
        aid_tx.send(record[0]).unwrap();
    });

    let addr = addr_rx.await.unwrap();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let config = config_for_port(addr.port());
    tokio::spawn(run_session(config, command_rx, event_tx));

    let screen = wait_for_screen(&mut event_rx).await;
    assert_eq!(&screen.text_rows()[0][0..5], "HELLO");
    command_tx
        .send(SessionCommand::SendAid(AidKey::Enter))
        .unwrap();
    assert_eq!(
        timeout(Duration::from_secs(2), aid_rx)
            .await
            .unwrap()
            .unwrap(),
        AidKey::Enter.code()
    );
}

#[tokio::test]
async fn tn3270e_session_unwraps_header_and_sends_tn3270e_response() {
    let (addr_tx, addr_rx) = oneshot::channel();
    let (aid_tx, aid_rx) = oneshot::channel();
    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        addr_tx.send(listener.local_addr().unwrap()).unwrap();
        let (mut stream, _) = listener.accept().await.unwrap();
        negotiate_tn3270e(&mut stream).await;
        let payload = Tn3270eHeader::new(DataType::Data3270, 1).encode(&erase_write_login_screen());
        stream.write_all(&frame_record(&payload)).await.unwrap();
        let record = read_telnet_record(&mut stream).await;
        let (header, payload) = Tn3270eHeader::parse(&record).unwrap();
        assert_eq!(header.data_type, DataType::Data3270);
        aid_tx.send(payload[0]).unwrap();
    });

    let addr = addr_rx.await.unwrap();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let config = config_for_port(addr.port());
    tokio::spawn(run_session(config, command_rx, event_tx));

    wait_for_connection(&mut event_rx, ConnectionState::Tn3270e).await;
    let screen = wait_for_screen(&mut event_rx).await;
    assert_eq!(&screen.text_rows()[0][0..5], "HELLO");
    command_tx
        .send(SessionCommand::SendAid(AidKey::Enter))
        .unwrap();
    assert_eq!(
        timeout(Duration::from_secs(2), aid_rx)
            .await
            .unwrap()
            .unwrap(),
        AidKey::Enter.code()
    );
}

#[tokio::test]
async fn host_eof_emits_disconnected() {
    let (addr_tx, addr_rx) = oneshot::channel();
    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        addr_tx.send(listener.local_addr().unwrap()).unwrap();
        let (_stream, _) = listener.accept().await.unwrap();
    });

    let addr = addr_rx.await.unwrap();
    let (_command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let config = config_for_port(addr.port());
    tokio::spawn(run_session(config, command_rx, event_tx));

    loop {
        match timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
        {
            Some(HostEvent::Disconnected) => break,
            Some(_) => {}
            None => panic!("session ended without a disconnected event"),
        }
    }
}

fn config_for_port(port: u16) -> SessionConfig {
    let mut config = SessionConfig::default();
    config.profile.name = "mock".to_owned();
    config.profile.host = "127.0.0.1".to_owned();
    config.profile.port = port;
    config.profile.terminal_model = TerminalModel::Model2;
    config.connect_timeout_secs = 2;
    config
}

async fn negotiate_traditional(stream: &mut TcpStream) {
    stream
        .write_all(&[
            IAC,
            DO,
            OPT_BINARY,
            IAC,
            WILL,
            OPT_BINARY,
            IAC,
            DO,
            OPT_EOR,
            IAC,
            WILL,
            OPT_EOR,
            IAC,
            DO,
            OPT_TERMINAL_TYPE,
        ])
        .await
        .unwrap();
    stream
        .write_all(&subneg(OPT_TERMINAL_TYPE, &[TERMINAL_TYPE_SEND]))
        .await
        .unwrap();
    wait_for_client_bytes(stream, b"IBM-3278-2-E").await;
}

async fn negotiate_tn3270e(stream: &mut TcpStream) {
    stream.write_all(&[IAC, DO, OPT_TN3270E]).await.unwrap();
    wait_for_client_bytes(stream, &[IAC, WILL, OPT_TN3270E]).await;
    stream
        .write_all(&subneg(OPT_TN3270E, &[TN_SEND, TN_DEVICE_TYPE]))
        .await
        .unwrap();
    wait_for_client_bytes(stream, b"IBM-3278-2-E").await;
    stream
        .write_all(&subneg(
            OPT_TN3270E,
            &[TN_DEVICE_TYPE, TN_IS, b'I', b'B', b'M'],
        ))
        .await
        .unwrap();
    wait_for_client_bytes(stream, &[TN_FUNCTIONS]).await;
}

fn erase_write_login_screen() -> Vec<u8> {
    vec![
        CMD_ERASE_WRITE,
        0x02,
        0xc8,
        0xc5,
        0xd3,
        0xd3,
        0xd6,
        0x40,
        ORDER_SF,
        0x00,
        0x40,
        0x40,
    ]
}

async fn wait_for_client_bytes(stream: &mut TcpStream, needle: &[u8]) {
    let mut buffer = Vec::new();
    let mut scratch = [0_u8; 256];
    loop {
        let read = timeout(Duration::from_secs(2), stream.read(&mut scratch))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(read, 0, "client closed before negotiation completed");
        buffer.extend_from_slice(&scratch[..read]);
        if buffer.windows(needle.len()).any(|window| window == needle) {
            return;
        }
    }
}

async fn read_telnet_record(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut iac_seen = false;
    loop {
        let byte = stream.read_u8().await.unwrap();
        if iac_seen {
            match byte {
                EOR => return out,
                IAC => out.push(IAC),
                _ => {}
            }
            iac_seen = false;
        } else if byte == IAC {
            iac_seen = true;
        } else {
            out.push(byte);
        }
    }
}

async fn wait_for_connection(
    event_rx: &mut mpsc::UnboundedReceiver<HostEvent>,
    state: ConnectionState,
) {
    loop {
        match timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
        {
            Some(HostEvent::Connection(connection))
            | Some(HostEvent::Oia(r3270_core::OiaState { connection, .. }))
                if connection == state =>
            {
                return;
            }
            Some(_) => {}
            None => panic!("session ended before reaching {state:?}"),
        }
    }
}

async fn wait_for_screen(
    event_rx: &mut mpsc::UnboundedReceiver<HostEvent>,
) -> r3270_core::PresentationSpace {
    loop {
        match timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
        {
            Some(HostEvent::Screen(screen)) => return screen,
            Some(_) => {}
            None => panic!("session ended before screen update"),
        }
    }
}
