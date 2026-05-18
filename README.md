# sshhub

One SSH connection, all my games. Connect to the hub, pick a game from the lobby, and your session gets bridged to the upstream game server transparently.

## Just try it out!

`ssh frittura.org`

Use the arrow keys (or `j`/`k`) to move, Enter to connect, Esc to leave.

## Run it yourself

You need the [rust toolchain](https://www.rust-lang.org/tools/install). Then:

```
cargo build --release
./target/release/sshhub --port 2222
```

The lobby reads from `games.toml` next to the binary. Each entry becomes a row in the menu; on selection sshhub opens an outbound SSH connection to `host:port` and bridges the bytes back and forth.

```toml
[[games]]
key = "sshattrick"
name = "ssHattrick"
description = "Hockey in your terminal."
host = "127.0.0.1"
port = 3020
```

Binding to port 22 in production needs either `setcap 'cap_net_bind_service=+ep'` on the binary, systemd socket activation, or an iptables redirect.

## For game devs

The crate also exposes the SSH/ratatui scaffolding I use across my games as a library, behind the `core` feature. Add to your `Cargo.toml`:

```toml
sshhub = { git = "https://github.com/ricott1/sshhub", default-features = false }
```

and implement the `SshGame` trait:

```rust
use sshhub::core::{run_server, Credential, SshGame, SshSession};

struct MyGame { /* ... */ }

impl SshGame for MyGame {
    type Auth = ();
    const SCREEN_SIZE: (u16, u16) = (160, 50);
    const TITLE: &'static str = "My Game";
    const SERVER_INACTIVITY: Duration = Duration::from_secs(3600);

    async fn authenticate(&self, _: &str, _: Credential) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_session(self: Arc<Self>, session: SshSession<()>) {
        // drive a ratatui Tui on `session.writer`, consume `session.data_rx`...
    }
}
```

See [stonks](https://github.com/ricott1/stonks) for a real example with credential-based save lookup.

## License

GPLv3.
