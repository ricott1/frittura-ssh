use clap::{ArgAction, Parser};
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use std::sync::Arc;
use sshhub::core::run_server;
use sshhub::ssh::HubGame;
use sshhub::{config, store_path, AppResult};

const DEFAULT_PORT: u16 = 2222;
const DEFAULT_GAMES_PATH: &str = "games.toml";

#[derive(Parser, Debug)]
#[clap(name="sshhub", about = "SSH lobby that proxies to ricott1's terminal games", author, version, long_about = None)]
struct Args {
    #[clap(long, short = 'p', action = ArgAction::Set, help = "Port to listen on")]
    port: Option<u16>,
    #[clap(long, short = 'g', action = ArgAction::Set, help = "Path to games.toml")]
    games: Option<String>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let logfile_path = store_path("sshhub.log")?;
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

    let hub = Arc::new(HubGame::new(games));
    run_server(hub, port).await?;
    Ok(())
}
