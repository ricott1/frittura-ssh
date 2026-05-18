use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Clone, Copy, Debug)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Quit,
}
