use crossterm::cursor::Show;
use crossterm::event::DisableMouseCapture;
use crossterm::terminal::{Clear, ClearType, LeaveAlternateScreen};
use russh::server::Handle;
use russh::ChannelId;

/// Buffer of bytes destined for the SSH client. Flushed via `Handle::data`
/// when ratatui calls `flush()` at the end of a draw. Implements
/// `std::io::Write` so a ratatui crossterm backend can write into it.
pub struct SshWriterProxy {
    flushing: bool,
    closed: bool,
    channel_id: ChannelId,
    handle: Handle,
    sink: Vec<u8>,
}

impl std::fmt::Debug for SshWriterProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshWriterProxy")
            .field("flushing", &self.flushing)
            .field("channel_id", &self.channel_id)
            .field("sink_len", &self.sink.len())
            .finish()
    }
}

impl SshWriterProxy {
    pub fn new(channel_id: ChannelId, handle: Handle) -> Self {
        Self {
            flushing: false,
            closed: false,
            channel_id,
            handle,
            sink: vec![],
        }
    }

    /// Drain the sink to the SSH client. No-op if `flush()` hasn't been
    /// called since the previous send.
    pub async fn send(&mut self) -> std::io::Result<usize> {
        if !self.flushing {
            return Ok(0);
        }
        let data_length = self.sink.len();
        if let Err(e) = self
            .handle
            .data(self.channel_id, std::mem::take(&mut self.sink))
            .await
        {
            log::error!("Flushing error: {e:?}");
            let _ = self.handle.close(self.channel_id).await;
        }
        self.flushing = false;
        Ok(data_length)
    }

    /// Fire-and-forget flush; channel stays open. No-op once `send_and_close`
    /// has run, or if called outside an active tokio runtime (e.g. during
    /// `#[tokio::main]` shutdown when Drop fires after the runtime exits).
    fn send_in_background(&mut self) {
        if self.closed {
            return;
        }
        let data = std::mem::take(&mut self.sink);
        self.flushing = false;
        if data.is_empty() {
            return;
        }
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let ssh_handle = self.handle.clone();
        let channel_id = self.channel_id;
        rt.spawn(async move {
            let _ = ssh_handle.data(channel_id, data).await;
        });
    }

    /// Write the standard terminal-restore escape sequence into the sink.
    /// `DisableMouseCapture` is a safe no-op on sessions that never enabled
    /// mouse, so this universal cleanup works for all games.
    fn write_terminal_restore(&mut self) {
        let _ = crossterm::execute!(
            self,
            LeaveAlternateScreen,
            DisableMouseCapture,
            Clear(ClearType::All),
            Show
        );
    }

    /// Restore the terminal, flush, EOF, close - all awaited so final bytes
    /// reach the client. Idempotent: subsequent calls (and any later
    /// `send_in_background`) no-op.
    pub async fn send_and_close(&mut self) {
        if self.closed {
            return;
        }
        self.write_terminal_restore();
        let data = std::mem::take(&mut self.sink);
        self.flushing = false;
        self.closed = true;
        if !data.is_empty() {
            let _ = self.handle.data(self.channel_id, data).await;
        }
        let _ = self.handle.eof(self.channel_id).await;
        let _ = self.handle.close(self.channel_id).await;
    }
}

impl std::io::Write for SshWriterProxy {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushing = true;
        Ok(())
    }
}

/// Fallback flush. Awaited shutdown via `send_and_close` is preferred; this
/// fires for unhandled paths (panics, early returns) so the channel doesn't
/// leak past the parent task's lifetime.
impl Drop for SshWriterProxy {
    fn drop(&mut self) {
        if !self.closed {
            self.write_terminal_restore();
        }
        self.send_in_background();
    }
}
