use clap::{ArgAction, Parser};
use frittura_ssh_core::run_server;
use frittura_ssh_hub::ssh::HubGame;
use frittura_ssh_hub::{config, store_path, AppResult, HubIdentity};
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use std::sync::Arc;

const DEFAULT_PORT: u16 = 2222;
const DEFAULT_GAMES_PATH: &str = "games.toml";

#[derive(Parser, Debug)]
#[clap(name="frittura-ssh-hub", about = "SSH lobby for terminal games", author, version, long_about = None)]
struct Args {
    #[clap(long, short = 'p', action = ArgAction::Set, help = "Port to listen on")]
    port: Option<u16>,
    #[clap(long, short = 'g', action = ArgAction::Set, help = "Path to games.toml")]
    games: Option<String>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let logfile_path = store_path("frittura-ssh-hub.log")?;
    let logfile = FileAppender::builder()
        .append(false)
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build(logfile_path)?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(config)?;

    let args = Args::parse();
    let port = args.port.unwrap_or(DEFAULT_PORT);
    let games_path = args.games.as_deref().unwrap_or(DEFAULT_GAMES_PATH);
    let games = config::load_games(games_path)?;
    log::info!(
        "Loaded {} games from {games_path}. Starting hub on port {port}.",
        games.len()
    );

    let identity_path = store_path("hub-master.seed")?;
    let identity = Arc::new(HubIdentity::load_or_generate(&identity_path)?);

    let hub = Arc::new(HubGame::new(games, identity));
    run_server(hub, port).await?;
    Ok(())
}
