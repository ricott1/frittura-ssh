use crate::config::{AuthMethod, GameMetadata, Preview};
use frittura_ssh_core::{
    convert_data_to_terminal_event, kick_warning_secs, Credential, SshWriterProxy, SshSession,
    TerminalEvent,
};
use crate::identity::HubIdentity;
use crate::ssh::bridge::{self, BridgeError};
use crate::tui::Tui;
use crossterm::event::KeyCode;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{self, MissedTickBehavior};

const DRAW_TIME_STEP: Duration = Duration::from_millis(1000 / 30);
const HUB_LOBBY_IDLE: Duration = Duration::from_secs(60);
const KICK_WARNING_REMAINING: Duration = Duration::from_secs(10);

fn unsupported_method_flash(game: &GameMetadata) -> String {
    let methods = game
        .auth
        .as_deref()
        .expect("flash only generated when game.auth restricts inbound");
    let joined = methods
        .iter()
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join(" or ");
    format!("{} requires {joined} authentication", game.name)
}

pub async fn run_hub_session(
    games: Arc<Vec<GameMetadata>>,
    identity: Arc<HubIdentity>,
    session: SshSession<Credential>,
) {
    let SshSession {
        username,
        auth: credential,
        term,
        writer,
        channel_id,
        handle,
        initial_size,
        exec_command,
        mut data_rx,
        mut resize_rx,
    } = session;

    log::info!("Session opened: user={username} channel={channel_id}");

    let mut writer = Some(writer);
    let mut flash: Option<String> = None;

    if let Some(name) = exec_command.as_deref() {
        match games.iter().find(|g| g.key == name) {
            Some(game) => {
                if !game.accepts_inbound(AuthMethod::for_credential(&credential)) {
                    log::info!(
                        "Direct exec to {} refused: inbound auth method not accepted",
                        game.key
                    );
                    let _ = handle.close(channel_id).await;
                    return;
                }
                log::info!("User {username} requested direct exec -> {}", game.key);
                let result = bridge_to(
                    game,
                    &identity,
                    channel_id,
                    &handle,
                    &username,
                    &credential,
                    &term,
                    initial_size.0,
                    initial_size.1,
                    &mut data_rx,
                    &mut resize_rx,
                )
                .await;
                match result {
                    Ok(()) => {}
                    Err(BridgeError::AuthRejected) => {
                        log::info!("Direct exec to {} auth-rejected", game.key);
                    }
                    Err(BridgeError::Other(e)) => {
                        log::warn!("Direct exec to {} ended with error: {e}", game.key);
                    }
                }
                let _ = handle.close(channel_id).await;
                return;
            }
            None => {
                log::info!("User {username} requested unknown game '{name}'");
                flash = Some(format!("unknown game '{name}'"));
            }
        }
    }

    loop {
        // The writer is consumed by the Tui inside run_lobby and dropped on
        // exit. For re-entry after a recoverable bridge failure, we make a
        // fresh writer from the still-live handle + channel_id.
        let lobby_writer = writer
            .take()
            .unwrap_or_else(|| SshWriterProxy::new(channel_id, handle.clone()));

        let LobbyOutcome {
            selected,
            data_rx: rx1,
            resize_rx: rx2,
            current_width,
            current_height,
        } = run_lobby(LobbyArgs {
            games: &games,
            username: username.clone(),
            writer: lobby_writer,
            initial_size,
            data_rx,
            resize_rx,
            flash: flash.take(),
        })
        .await;
        data_rx = rx1;
        resize_rx = rx2;

        let Some(game) = selected else {
            log::info!("User {username} left the hub");
            return;
        };

        if !game.accepts_inbound(AuthMethod::for_credential(&credential)) {
            log::info!(
                "User {username} selected '{}' but inbound auth method not accepted",
                game.key
            );
            flash = Some(unsupported_method_flash(&game));
            continue;
        }

        log::info!(
            "User {username} selected '{}' -> bridging to {}:{}",
            game.key,
            game.host,
            game.port
        );

        let bridge_result = bridge_to(
            &game,
            &identity,
            channel_id,
            &handle,
            &username,
            &credential,
            &term,
            current_width,
            current_height,
            &mut data_rx,
            &mut resize_rx,
        )
        .await;

        match bridge_result {
            Ok(()) => break,
            Err(BridgeError::AuthRejected) => {
                log::info!("Outbound auth rejected by {}", game.key);
                flash = Some(format!(
                    "could not connect to {}: authentication rejected",
                    game.key
                ));
                // Loop continues - re-enter the lobby with the flash text.
            }
            Err(BridgeError::Other(e)) => {
                log::warn!(
                    "Bridge to {} ({}:{}) ended with error: {e}",
                    game.key,
                    game.host,
                    game.port
                );
                flash = Some(format!("could not connect to {}", game.key));
            }
        }
    }

    let _ = handle.close(channel_id).await;
}

#[allow(clippy::too_many_arguments)]
async fn bridge_to(
    game: &GameMetadata,
    identity: &Arc<HubIdentity>,
    channel_id: russh::ChannelId,
    handle: &russh::server::Handle,
    username: &str,
    credential: &Credential,
    term: &str,
    width: u32,
    height: u32,
    data_rx: &mut mpsc::Receiver<Vec<u8>>,
    resize_rx: &mut mpsc::Receiver<(u32, u32)>,
) -> Result<(), BridgeError> {
    bridge::run(bridge::BridgeArgs {
        channel_id,
        handle: handle.clone(),
        username: username.to_string(),
        credential: credential.clone(),
        game: game.clone(),
        identity: identity.clone(),
        term: term.to_string(),
        width,
        height,
        data_rx,
        resize_rx,
    })
    .await
}

struct LobbyArgs<'a> {
    games: &'a Arc<Vec<GameMetadata>>,
    username: String,
    writer: SshWriterProxy,
    initial_size: (u32, u32),
    data_rx: mpsc::Receiver<Vec<u8>>,
    resize_rx: mpsc::Receiver<(u32, u32)>,
    flash: Option<String>,
}

struct LobbyOutcome {
    selected: Option<GameMetadata>,
    data_rx: mpsc::Receiver<Vec<u8>>,
    resize_rx: mpsc::Receiver<(u32, u32)>,
    current_width: u32,
    current_height: u32,
}

async fn run_lobby(args: LobbyArgs<'_>) -> LobbyOutcome {
    let LobbyArgs {
        games,
        username,
        writer,
        initial_size,
        mut data_rx,
        mut resize_rx,
        mut flash,
    } = args;

    let mut tui = match Tui::new(username.clone(), writer) {
        Ok(t) => t,
        Err(e) => {
            log::error!("Tui init failed for {username}: {e}");
            return LobbyOutcome {
                selected: None,
                data_rx,
                resize_rx,
                current_width: initial_size.0,
                current_height: initial_size.1,
            };
        }
    };

    let mut selected_idx: usize = 0;
    let mut selected_at = Instant::now();
    let mut dirty = true;
    let mut last_input_at = Instant::now();
    let mut current_width = initial_size.0;
    let mut current_height = initial_size.1;

    let mut draw_ticker = time::interval(DRAW_TIME_STEP);
    draw_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut selected: Option<GameMetadata> = None;
    let mut quit = false;

    while !quit && selected.is_none() {
        tokio::select! {
            biased;
            data = data_rx.recv() => {
                let Some(data) = data else { break; };
                let Some(event) = convert_data_to_terminal_event(&data) else { continue; };
                last_input_at = Instant::now();
                // Any keypress clears the flash banner.
                if flash.is_some() {
                    flash = None;
                    dirty = true;
                }
                match event {
                    TerminalEvent::Key(key) => {
                        let n = games.len().max(1);
                        match key.code {
                            KeyCode::Esc => quit = true,
                            KeyCode::Up | KeyCode::Char('k') => {
                                selected_idx = (selected_idx + n - 1) % n;
                                selected_at = Instant::now();
                                dirty = true;
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                selected_idx = (selected_idx + 1) % n;
                                selected_at = Instant::now();
                                dirty = true;
                            }
                            KeyCode::Enter => {
                                selected = games.get(selected_idx).cloned();
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                let idx = (c as u8 - b'1') as usize;
                                if idx < games.len() {
                                    selected = Some(games[idx].clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    TerminalEvent::Resize(_, _)
                    | TerminalEvent::Mouse(_)
                    | TerminalEvent::Quit
                    | TerminalEvent::IdleWarning(_) => {}
                }
            }
            change = resize_rx.recv() => {
                let Some((w, h)) = change else { break; };
                current_width = w;
                current_height = h;
                dirty = true;
            }
            _ = draw_ticker.tick() => {
                let now = Instant::now();
                let warning = kick_warning_secs(last_input_at, now, HUB_LOBBY_IDLE, KICK_WARNING_REMAINING);
                if warning.is_none()
                    && now.saturating_duration_since(last_input_at) >= HUB_LOBBY_IDLE
                {
                    log::info!("Hub: kicking idle user {username}");
                    quit = true;
                    continue;
                }
                let animated = matches!(
                    games.get(selected_idx).and_then(|g| g.preview.as_ref()),
                    Some(Preview::Animated(_))
                );
                if dirty || warning.is_some() || animated {
                    let _ = tui.draw_lobby(
                        games,
                        selected_idx,
                        warning,
                        flash.as_deref(),
                        selected_at.elapsed(),
                    );
                    let _ = tui.push_data().await;
                    dirty = false;
                }
            }
        }
    }

    if quit && selected.is_none() {
        tui.close().await;
    } else {
        drop(tui);
    }

    LobbyOutcome {
        selected,
        data_rx,
        resize_rx,
        current_width,
        current_height,
    }
}
