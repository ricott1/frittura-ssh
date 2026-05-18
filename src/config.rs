use crate::AppResult;
use anyhow::Context;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct GameMetadata {
    pub key: String,
    pub name: String,
    pub description: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
struct TopLevel {
    games: Vec<GameMetadata>,
}

pub fn load_games(path: impl AsRef<Path>) -> AppResult<Vec<GameMetadata>> {
    let path = path.as_ref();
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("reading games config at {}", path.display()))?;
    let top: TopLevel = toml::from_str(&contents)
        .with_context(|| format!("parsing games config at {}", path.display()))?;
    Ok(top.games)
}
