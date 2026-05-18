use crate::core::client::AuthedSession;
use crate::core::trait_def::{SshGame, SshSession};
use crate::core::writer::SSHWriterProxy;
use anyhow::anyhow;
use russh::server::Handle;
use russh::ChannelId;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct AppChannel<G: SshGame> {
    state: AppChannelState,
    authed: AuthedSession<G>,
    game: Arc<G>,
}

enum AppChannelState {
    AwaitingPty,
    Ready {
        data_tx: mpsc::Sender<Vec<u8>>,
        resize_tx: mpsc::Sender<(u32, u32)>,
    },
}

impl<G: SshGame> AppChannel<G> {
    pub(crate) fn new(authed: AuthedSession<G>, game: Arc<G>) -> Self {
        Self {
            state: AppChannelState::AwaitingPty,
            authed,
            game,
        }
    }

    pub async fn data(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let AppChannelState::Ready { data_tx, .. } = &self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };
        data_tx
            .send(data.to_vec())
            .await
            .map_err(|_| anyhow!("session task gone"))?;
        Ok(())
    }

    pub async fn pty_request(
        &mut self,
        id: ChannelId,
        term: String,
        width: u32,
        height: u32,
        handle: Handle,
    ) -> anyhow::Result<()> {
        if !matches!(self.state, AppChannelState::AwaitingPty) {
            return Err(anyhow!("pty has been already allocated"));
        }
        let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(64);
        let (resize_tx, resize_rx) = mpsc::channel::<(u32, u32)>(8);

        let session = SshSession {
            username: self.authed.username.clone(),
            auth: self.authed.auth.clone(),
            term,
            writer: SSHWriterProxy::new(id, handle.clone()),
            channel_id: id,
            handle,
            initial_size: (width, height),
            data_rx,
            resize_rx,
        };

        let game = self.game.clone();
        tokio::spawn(async move {
            game.on_session(session).await;
        });

        self.state = AppChannelState::Ready { data_tx, resize_tx };
        Ok(())
    }

    pub async fn window_change_request(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        let AppChannelState::Ready { resize_tx, .. } = &self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };
        resize_tx
            .send((width, height))
            .await
            .map_err(|_| anyhow!("session task gone"))?;
        Ok(())
    }
}
