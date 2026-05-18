//! Shared SSH-game runtime: trait + helpers consumed by sshhub itself and by
//! sibling game crates (sshattrick, rebels-in-the-sky, stonks, asterion).
//!
//! Downstream game crates pull this in via:
//! ```toml
//! sshhub = { git = "https://github.com/ricott1/sshhub", default-features = false }
//! ```
//! which selects only the `core` feature and excludes the hub binary deps.

pub mod channel;
pub mod client;
pub mod event;
pub mod idle;
pub mod input;
pub mod keys;
pub mod server;
pub mod trait_def;
pub mod writer;

pub use event::TerminalEvent;
pub use idle::kick_warning_secs;
pub use input::{convert_data_to_terminal_event, CMD_RESIZE};
pub use keys::load_or_generate;
pub use server::run_server;
pub use trait_def::{spawn_event_converter, Credential, SshGame, SshSession};
pub use writer::SSHWriterProxy;
