use crate::config::{AuthMethod, GameMetadata};
use crate::identity::HubIdentity;
use crate::AppResult;
use anyhow::{anyhow, Context};
use frittura_ssh_core::Credential;
use russh::client::{self, Config, Handler, Msg};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey};
use russh::{Channel, ChannelMsg, Disconnect};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct BridgeArgs<'a> {
    pub channel_id: russh::ChannelId,
    pub handle: russh::server::Handle,
    pub username: String,
    pub credential: Credential,
    pub game: GameMetadata,
    pub identity: Arc<HubIdentity>,
    pub term: String,
    pub width: u32,
    pub height: u32,
    pub data_rx: &'a mut mpsc::Receiver<Vec<u8>>,
    pub resize_rx: &'a mut mpsc::Receiver<(u32, u32)>,
}

/// Why the bridge ended. `AuthRejected` is recoverable: the hub re-enters
/// the lobby with a flash message. Everything else is a hard error - we
/// log and close the user session.
pub enum BridgeError {
    AuthRejected,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for BridgeError {
    fn from(e: anyhow::Error) -> Self {
        BridgeError::Other(e)
    }
}

/// Trust-on-first-use server key handler. The hub connects to a fixed list
/// of game servers configured locally, so TOFU is acceptable. A future
/// iteration could pin per-game host keys in games.toml.
struct BridgeClientHandler;

impl Handler for BridgeClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub async fn run(args: BridgeArgs<'_>) -> Result<(), BridgeError> {
    let config = Arc::new(Config {
        inactivity_timeout: Some(Duration::from_secs(3600)),
        ..Default::default()
    });

    let mut session = client::connect(
        config,
        (args.game.host.as_str(), args.game.port),
        BridgeClientHandler,
    )
    .await
    .with_context(|| format!("connecting to {}:{}", args.game.host, args.game.port))?;

    let outbound_method = args
        .game
        .outbound_method(AuthMethod::for_credential(&args.credential));
    let auth = match (outbound_method, &args.credential) {
        (AuthMethod::Password, cred) => {
            let pw = match cred {
                Credential::Password(p) => p.clone(),
                Credential::PublicKey(pk) => pk.to_string(),
            };
            session
                .authenticate_password(args.username.as_str(), pw.as_str())
                .await
                .context("outbound authenticate_password failed")?
        }
        (AuthMethod::Publickey, Credential::PublicKey(pk)) => {
            let fingerprint = pk.fingerprint(HashAlg::Sha256).to_string();
            let derived = args
                .identity
                .derive_for(&args.game.key, &fingerprint)
                .context("deriving hub identity key")?;
            let key = PrivateKeyWithHashAlg::new(Arc::new(derived), None);
            session
                .authenticate_publickey(args.username.as_str(), key)
                .await
                .context("outbound authenticate_publickey failed")?
        }
        (AuthMethod::Publickey, Credential::Password(_)) => {
            return Err(BridgeError::AuthRejected);
        }
    };
    if !auth.success() {
        return Err(BridgeError::AuthRejected);
    }

    let mut outbound = session
        .channel_open_session()
        .await
        .context("outbound channel_open_session failed")?;

    outbound
        .request_pty(true, &args.term, args.width, args.height, 0, 0, &[])
        .await
        .context("outbound request_pty failed")?;
    await_request_reply(&mut outbound, "pty-req").await?;

    outbound
        .request_shell(true)
        .await
        .context("outbound request_shell failed")?;
    await_request_reply(&mut outbound, "shell").await?;

    loop {
        tokio::select! {
            data = args.data_rx.recv() => {
                let Some(bytes) = data else { break; };
                if let Err(e) = outbound.data(&bytes[..]).await {
                    log::warn!("outbound data write failed: {e}");
                    break;
                }
            }
            change = args.resize_rx.recv() => {
                let Some((w, h)) = change else { break; };
                if let Err(e) = outbound.window_change(w, h, 0, 0).await {
                    log::warn!("outbound window_change failed: {e}");
                    break;
                }
            }
            msg = outbound.wait() => {
                let Some(msg) = msg else { break; };
                match msg {
                    ChannelMsg::Data { data } => {
                        if let Err(e) = args.handle.data(args.channel_id, data).await {
                            log::warn!("inbound data write failed: {e:?}");
                            break;
                        }
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        if let Err(e) = args.handle.data(args.channel_id, data).await {
                            log::warn!("inbound extended data write failed: {e:?}");
                            break;
                        }
                    }
                    ChannelMsg::Eof | ChannelMsg::Close | ChannelMsg::ExitStatus { .. } => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = outbound.close().await;
    let _ = session
        .disconnect(Disconnect::ByApplication, "", "en")
        .await;

    Ok(())
}

impl From<russh::Error> for BridgeError {
    fn from(e: russh::Error) -> Self {
        BridgeError::Other(e.into())
    }
}

const REQUEST_REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Block on the Success/Failure reply for a `want_reply: true` request.
async fn await_request_reply(chan: &mut Channel<Msg>, req: &'static str) -> AppResult<()> {
    let wait = async {
        loop {
            match chan.wait().await {
                Some(ChannelMsg::Success) => return Ok(()),
                Some(ChannelMsg::Failure) => {
                    return Err(anyhow!("outbound {req} refused by server"))
                }
                Some(_) => continue,
                None => return Err(anyhow!("outbound channel closed before {req} reply")),
            }
        }
    };
    tokio::time::timeout(REQUEST_REPLY_TIMEOUT, wait)
        .await
        .map_err(|_| anyhow!("outbound {req} timed out"))?
}
