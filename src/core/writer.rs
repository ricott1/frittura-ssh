use russh::server::Handle;
use russh::ChannelId;

/// Buffer of bytes destined for the SSH client. Flushed via `Handle::data`
/// when ratatui calls `flush()` at the end of a draw. Implements
/// `std::io::Write` so a ratatui crossterm backend can write into it.
#[derive(Clone)]
pub struct SSHWriterProxy {
    flushing: bool,
    channel_id: ChannelId,
    handle: Handle,
    sink: Vec<u8>,
}

impl std::fmt::Debug for SSHWriterProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSHWriterProxy")
            .field("flushing", &self.flushing)
            .field("channel_id", &self.channel_id)
            .field("sink_len", &self.sink.len())
            .finish()
    }
}

impl SSHWriterProxy {
    pub fn new(channel_id: ChannelId, handle: Handle) -> Self {
        Self {
            flushing: false,
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

    /// Hand the current sink off to a background task. Lets `Drop` impls
    /// (which can't await) still get the final alt-screen-cleanup bytes out
    /// before the channel closes.
    pub fn send_in_background(&mut self) {
        if self.sink.is_empty() {
            return;
        }
        let handle = self.handle.clone();
        let channel_id = self.channel_id;
        let data = std::mem::take(&mut self.sink);
        self.flushing = false;
        tokio::spawn(async move {
            let _ = handle.data(channel_id, data).await;
        });
    }
}

impl std::io::Write for SSHWriterProxy {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushing = true;
        Ok(())
    }
}
