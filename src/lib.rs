pub mod core;

#[cfg(feature = "bin")]
pub mod config;
#[cfg(feature = "bin")]
pub mod ssh;
#[cfg(feature = "bin")]
mod tui;
#[cfg(feature = "bin")]
mod ui;
#[cfg(feature = "bin")]
mod utils;

#[cfg(feature = "bin")]
pub use utils::store_path;

pub type AppResult<T> = Result<T, anyhow::Error>;
