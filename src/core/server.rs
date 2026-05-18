use crate::core::client::AppClient;
use crate::core::trait_def::SshGame;
use russh::server::{Config, Server};
use std::sync::Arc;
use std::time::Duration;

const HOST_KEY_PATH: &str = "./keys";

/// Stand up the shared SSH server for a game. Blocks until the server stops.
pub async fn run_server<G: SshGame>(game: Arc<G>, port: u16) -> anyhow::Result<()> {
    let private_key = crate::core::keys::load_or_generate(HOST_KEY_PATH)?;
    let config = Config {
        inactivity_timeout: Some(G::SERVER_INACTIVITY),
        auth_rejection_time: Duration::from_secs(3),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        // Drop dead NAT'd peers in well under `inactivity_timeout` so the
        // server doesn't hold orphaned channels and connections for an hour.
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        keys: vec![private_key],
        ..Default::default()
    };
    log::info!("Starting {} SSH server on port {port}.", G::TITLE);
    let mut server = GameServer { game };
    server
        .run_on_address(Arc::new(config), ("0.0.0.0", port))
        .await?;
    Ok(())
}

struct GameServer<G: SshGame> {
    game: Arc<G>,
}

impl<G: SshGame> Server for GameServer<G> {
    type Handler = AppClient<G>;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> AppClient<G> {
        AppClient::new(self.game.clone())
    }
}
