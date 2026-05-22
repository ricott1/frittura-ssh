use crate::config::GameMetadata;
use crate::identity::HubIdentity;
use frittura_ssh_core::{Credential, SshGame, SshSession};
use crate::ssh::session::run_hub_session;
use std::sync::Arc;
use std::time::Duration;

pub struct HubGame {
    games: Arc<Vec<GameMetadata>>,
    identity: Arc<HubIdentity>,
}

impl HubGame {
    pub fn new(games: Vec<GameMetadata>, identity: Arc<HubIdentity>) -> Self {
        Self {
            games: Arc::new(games),
            identity,
        }
    }
}

impl SshGame for HubGame {
    /// Sized to fit comfortably in an 80x24 terminal.
    const SCREEN_SIZE: (u16, u16) = (78, 22);
    const TITLE: &'static str = "sshhub";
    const SERVER_INACTIVITY: Duration = Duration::from_secs(3600);

    /// Hub keeps the credential around so the bridge can forward it
    /// outbound to the chosen game.
    type Auth = Credential;

    async fn authenticate(&self, _username: &str, credential: Credential) -> anyhow::Result<Credential> {
        Ok(credential)
    }

    async fn on_session(self: Arc<Self>, session: SshSession<Credential>) {
        run_hub_session(self.games.clone(), self.identity.clone(), session).await;
    }
}
