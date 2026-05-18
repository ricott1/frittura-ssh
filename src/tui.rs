use crate::config::GameMetadata;
use crate::core::SSHWriterProxy;
use crate::ui;
use crate::AppResult;
use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, SetTitle};
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

/// Hub lobby is fixed-size so the same TUI works regardless of the user's
/// real terminal dimensions. Chosen to fit comfortably in an 80x24 window.
const HUB_SCREEN_SIZE: (u16, u16) = (78, 22);

pub struct Tui {
    username: String,
    terminal: Terminal<CrosstermBackend<SSHWriterProxy>>,
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
        let mut tui = Self { username, terminal };
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
        let backend = self.terminal.backend_mut();
        let _ = crossterm::execute!(
            backend,
            LeaveAlternateScreen,
            Clear(ClearType::All),
            Show
        );
        backend.writer_mut().send_in_background();
    }
}
