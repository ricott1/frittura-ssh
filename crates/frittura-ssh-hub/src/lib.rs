pub mod config;
pub mod identity;
pub mod ssh;
mod tui;
mod ui;
mod utils;

pub use identity::HubIdentity;
pub use utils::store_path;

pub type AppResult<T> = Result<T, anyhow::Error>;
