use crate::core::channel::AppChannel;
use crate::core::trait_def::{Credential, SshGame};
use anyhow::{anyhow, Context};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, Pty};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AppClient<G: SshGame> {
    /// `None` until one of the `auth_*` callbacks accepts. russh won't open
    /// channels before that, so by the time `channel_open_session` runs the
    /// option is always `Some`.
    auth: Option<AuthedSession<G>>,
    game: Arc<G>,
    channels: HashMap<ChannelId, AppChannel<G>>,
}

pub(crate) struct AuthedSession<G: SshGame> {
    pub username: String,
    pub auth: G::Auth,
}

impl<G: SshGame> Clone for AuthedSession<G> {
    fn clone(&self) -> Self {
        Self {
            username: self.username.clone(),
            auth: self.auth.clone(),
        }
    }
}

impl<G: SshGame> AppClient<G> {
    pub fn new(game: Arc<G>) -> Self {
        Self {
            auth: None,
            game,
            channels: HashMap::new(),
        }
    }

    fn channel_mut(&mut self, id: ChannelId) -> anyhow::Result<&mut AppChannel<G>> {
        self.channels
            .get_mut(&id)
            .with_context(|| format!("unknown channel: {id}"))
    }

    async fn run_authenticate(&self, user: &str, credential: Credential) -> Option<AuthedSession<G>> {
        match self.game.authenticate(user, credential).await {
            Ok(auth) => Some(AuthedSession {
                username: user.to_string(),
                auth,
            }),
            Err(e) => {
                // `{user:?}` debug-formats to escape control chars so a
                // malicious username can't inject newlines into the log.
                log::info!("auth rejected for {user:?}: {e}");
                None
            }
        }
    }
}

impl<G: SshGame> server::Handler for AppClient<G> {
    type Error = anyhow::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> anyhow::Result<Auth> {
        match self
            .run_authenticate(user, Credential::Password(password.to_string()))
            .await
        {
            Some(authed) => {
                self.auth = Some(authed);
                Ok(Auth::Accept)
            }
            None => Ok(Auth::reject()),
        }
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::PublicKey,
    ) -> anyhow::Result<Auth> {
        match self
            .run_authenticate(user, Credential::PublicKey(public_key.clone()))
            .await
        {
            Some(authed) => {
                self.auth = Some(authed);
                Ok(Auth::Accept)
            }
            None => Ok(Auth::reject()),
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> anyhow::Result<bool> {
        let authed = self
            .auth
            .clone()
            .ok_or_else(|| anyhow!("channel opened before authentication"))?;
        let app_channel = AppChannel::new(authed, self.game.clone());
        if self.channels.insert(channel.id(), app_channel).is_some() {
            return Err(anyhow!("channel `{}` has been already opened", channel.id()));
        }
        Ok(true)
    }

    async fn channel_close(&mut self, id: ChannelId, _: &mut Session) -> anyhow::Result<()> {
        self.channels.remove(&id);
        Ok(())
    }

    async fn data(&mut self, id: ChannelId, data: &[u8], _: &mut Session) -> anyhow::Result<()> {
        self.channel_mut(id)?.data(data).await
    }

    async fn pty_request(
        &mut self,
        id: ChannelId,
        term: &str,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> anyhow::Result<()> {
        let handle = session.handle();
        self.channel_mut(id)?
            .pty_request(id, term.to_string(), width, height, handle)
            .await
    }

    async fn window_change_request(
        &mut self,
        id: ChannelId,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> anyhow::Result<()> {
        self.channel_mut(id)?
            .window_change_request(width, height)
            .await
    }
}
