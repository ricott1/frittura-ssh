use crate::core::event::TerminalEvent;
use crate::core::writer::SSHWriterProxy;
use russh::server::Handle;
use russh::ChannelId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// What the user proved at the front door. One unified type instead of
/// `(&str credential, AuthKind kind)` - the variants encode the auth
/// method, the payloads are the typed data the game can act on directly.
#[derive(Clone, Debug)]
pub enum Credential {
    Password(String),
    PublicKey(russh::keys::PublicKey),
}

pub trait SshGame: Send + Sync + 'static {
    /// Fixed ratatui viewport size. Wired into `Viewport::Fixed` by the
    /// runtime if the game uses the bundled `Tui` helper.
    const SCREEN_SIZE: (u16, u16);

    /// Terminal title set on session open.
    const TITLE: &'static str;

    /// Forwarded to `russh::server::Config::inactivity_timeout`. Whole
    /// connections idle for this long get dropped by russh itself.
    const SERVER_INACTIVITY: Duration;

    /// Per-session state the game produces at auth time. Whatever the game
    /// wants downstream (the loaded save, the parsed credential, an
    /// `AgentId`, ...). Use `()` if the game doesn't need any. `Clone` is
    /// required so the runtime can hand a fresh copy to each channel when
    /// an SSH connection opens multiple (rare in practice).
    type Auth: Send + Sync + Clone + 'static;

    /// Validate the user's credential and produce per-session state.
    /// Reject by returning `Err` - the runtime turns that into an
    /// SSH-level auth failure.
    fn authenticate(
        &self,
        username: &str,
        credential: Credential,
    ) -> impl std::future::Future<Output = anyhow::Result<Self::Auth>> + Send;

    /// Called once per successful PTY allocation. The game decides what to
    /// do with the session (spawn a per-channel App, forward to a
    /// matchmaker, bridge to another SSH server, etc).
    fn on_session(
        self: Arc<Self>,
        session: SshSession<Self::Auth>,
    ) -> impl std::future::Future<Output = ()> + Send;
}

/// Everything the runtime hands a game when a new SSH session is ready.
/// Raw `data_rx` is exposed so the hub can byte-bridge to an upstream SSH
/// server without re-serialization; most games will call
/// `core::spawn_event_converter(data_rx, resize_rx)` instead to get a single
/// `Receiver<TerminalEvent>`.
pub struct SshSession<A> {
    pub username: String,
    /// Game-defined per-session state produced by `authenticate`.
    pub auth: A,
    /// `TERM` advertised by the client at `pty_request` time. Most games can
    /// ignore this; the hub forwards it to the upstream game on the bridge.
    pub term: String,
    pub writer: SSHWriterProxy,
    pub channel_id: ChannelId,
    pub handle: Handle,
    pub initial_size: (u32, u32),
    pub data_rx: mpsc::Receiver<Vec<u8>>,
    pub resize_rx: mpsc::Receiver<(u32, u32)>,
}

/// Convenience: drain raw inbound bytes + window-change events into a single
/// parsed `TerminalEvent` stream. Useful for games that don't need raw
/// byte access (i.e., everything except the hub itself).
pub fn spawn_event_converter(
    mut data_rx: mpsc::Receiver<Vec<u8>>,
    mut resize_rx: mpsc::Receiver<(u32, u32)>,
) -> mpsc::Receiver<TerminalEvent> {
    let (tx, rx) = mpsc::channel::<TerminalEvent>(64);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                bytes = data_rx.recv() => {
                    let Some(bytes) = bytes else { break; };
                    if let Some(ev) = crate::core::input::convert_data_to_terminal_event(&bytes) {
                        if tx.send(ev).await.is_err() { break; }
                    }
                }
                resize = resize_rx.recv() => {
                    let Some((w, h)) = resize else { break; };
                    let ev = TerminalEvent::Resize(w.min(u16::MAX as u32) as u16, h.min(u16::MAX as u32) as u16);
                    if tx.send(ev).await.is_err() { break; }
                }
            }
        }
        let _ = tx.send(TerminalEvent::Quit).await;
    });
    rx
}
