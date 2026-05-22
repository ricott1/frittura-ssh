use crate::config::GameMetadata;
use crate::ui;
use crate::AppResult;
use crossterm::cursor::Hide;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, SetTitle};
use frittura_ssh_core::SshWriterProxy;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::time::Duration;

/// Hub lobby is fixed-size so the same TUI works regardless of the user's
/// real terminal dimensions.
const HUB_SCREEN_SIZE: (u16, u16) = (132, 48);

pub struct Tui {
    username: String,
    terminal: Terminal<CrosstermBackend<SshWriterProxy>>,
}

impl Tui {
    pub fn new(username: String, writer: SshWriterProxy) -> AppResult<Self> {
        let backend = CrosstermBackend::new(writer);
        let opts = TerminalOptions {
            viewport: Viewport::Fixed(Rect {
                x: 0,
                y: 0,
                width: HUB_SCREEN_SIZE.0,
                height: HUB_SCREEN_SIZE.1,
            }),
        };
        let terminal = Terminal::with_options(backend, opts)?;
        let mut tui = Self { username, terminal };
        tui.init()?;
        Ok(tui)
    }

    fn init(&mut self) -> AppResult<()> {
        crossterm::execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            SetTitle("ssHub"),
            Clear(ClearType::All),
            Hide
        )?;
        Ok(())
    }

    /// Restore the terminal and close the SSH channel, awaited end-to-end.
    pub async fn close(mut self) {
        self.terminal
            .backend_mut()
            .writer_mut()
            .send_and_close()
            .await;
    }

    pub fn draw_lobby(
        &mut self,
        games: &[GameMetadata],
        selected_idx: usize,
        kick_warning_secs: Option<u32>,
        flash: Option<&str>,
        selection_elapsed: Duration,
    ) -> AppResult<()> {
        let username = &self.username;
        self.terminal.draw(|frame| {
            ui::render_lobby_menu(
                frame,
                username,
                games,
                selected_idx,
                kick_warning_secs,
                flash,
                selection_elapsed,
            )
        })?;
        Ok(())
    }

    pub async fn push_data(&mut self) -> AppResult<()> {
        self.terminal.backend_mut().writer_mut().send().await?;
        Ok(())
    }
}
