use crate::config::GameMetadata;
use crate::ui;
use crate::AppResult;
use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, SetTitle};
use frittura_ssh_core::SSHWriterProxy;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

/// Hub lobby is fixed-size so the same TUI works regardless of the user's
/// real terminal dimensions. Chosen to fit comfortably in an 80x24 window.
const HUB_SCREEN_SIZE: (u16, u16) = (78, 22);

pub struct Tui {
    username: String,
    terminal: Terminal<CrosstermBackend<SSHWriterProxy>>,
    /// When true, `Drop` flushes the alt-screen-cleanup bytes AND closes the
    /// SSH channel atomically. When false (the default), Drop only flushes -
    /// leaving the channel open so the hub can re-create a fresh Tui after a
    /// recoverable bridge auth failure.
    close_on_drop: bool,
}

impl Tui {
    pub fn new(username: String, writer: SSHWriterProxy) -> AppResult<Self> {
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
        let mut tui = Self {
            username,
            terminal,
            close_on_drop: false,
        };
        tui.init()?;
        Ok(tui)
    }

    fn init(&mut self) -> AppResult<()> {
        crossterm::execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            SetTitle("sshhub"),
            Clear(ClearType::All),
            Hide
        )?;
        Ok(())
    }

    /// Mark this Tui so `Drop` closes the SSH channel after the final flush.
    /// Use when the session is ending (user quit or got idle-kicked); leave
    /// false when the Tui drop is just a swap (e.g. re-entering the lobby
    /// after a recoverable bridge auth failure).
    pub fn close_channel_on_drop(&mut self) {
        self.close_on_drop = true;
    }

    pub fn draw_lobby(
        &mut self,
        games: &[GameMetadata],
        selected_idx: usize,
        kick_warning_secs: Option<u32>,
        flash: Option<&str>,
    ) -> AppResult<()> {
        let username = &self.username;
        self.terminal.draw(|frame| {
            ui::render_lobby_menu(frame, username, games, selected_idx, kick_warning_secs, flash)
        })?;
        Ok(())
    }

    pub async fn push_data(&mut self) -> AppResult<()> {
        self.terminal.backend_mut().writer_mut().send().await?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let close = self.close_on_drop;
        let backend = self.terminal.backend_mut();
        let _ = crossterm::execute!(
            backend,
            LeaveAlternateScreen,
            Clear(ClearType::All),
            Show
        );
        let writer = backend.writer_mut();
        if close {
            writer.send_and_close_in_background();
        } else {
            writer.send_in_background();
        }
    }
}
