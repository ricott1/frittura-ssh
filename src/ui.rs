use crate::config::GameMetadata;
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

pub fn render_lobby_menu(
    frame: &mut Frame,
    username: &str,
    games: &[GameMetadata],
    selected_idx: usize,
    kick_warning_secs: Option<u32>,
    flash: Option<&str>,
) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let chunks = Layout::vertical([
        Constraint::Length(1), // top pad
        Constraint::Length(1), // title
        Constraint::Length(1), // pad
        Constraint::Length(1), // username
        Constraint::Length(1), // pad
        Constraint::Length(1), // flash banner (empty unless set)
        Constraint::Length(1), // pad
        Constraint::Length(1), // hint header
        Constraint::Length(1), // pad
        Constraint::Min(3),    // game list
        Constraint::Length(1), // pad
        Constraint::Length(1), // kick warning (empty unless within window)
        Constraint::Length(1), // pad
        Constraint::Length(1), // controls hint
    ])
    .split(area);

    let centered = |line: Line<'static>| Paragraph::new(line).alignment(Alignment::Center);

    frame.render_widget(
        centered(Line::styled(
            "sshhub",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        chunks[1],
    );
    frame.render_widget(centered(Line::from(username.to_string())), chunks[3]);

    if let Some(msg) = flash {
        frame.render_widget(
            centered(Line::styled(
                msg.to_string(),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )),
            chunks[5],
        );
    }

    frame.render_widget(
        centered(Line::styled(
            "Pick a game:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        chunks[7],
    );

    let list_lines: Vec<Line<'static>> = if games.is_empty() {
        vec![Line::styled(
            "(no games configured)",
            Style::default().fg(Color::DarkGray),
        )]
    } else {
        games
            .iter()
            .enumerate()
            .map(|(i, g)| render_game_line(i, g, i == selected_idx))
            .collect()
    };
    frame.render_widget(
        Paragraph::new(list_lines).alignment(Alignment::Center),
        chunks[9],
    );

    if let Some(secs) = kick_warning_secs {
        frame.render_widget(
            centered(Line::styled(
                format!("idle - kicking in {secs}s, press any key"),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )),
            chunks[11],
        );
    }

    frame.render_widget(
        centered(Line::styled(
            "↑/↓ or j/k: move    Enter: connect    Esc: leave",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[13],
    );
}

fn render_game_line(idx: usize, game: &GameMetadata, selected: bool) -> Line<'static> {
    let prefix = format!("{}. ", idx + 1);
    let name = game.name.clone();
    let desc = format!("  -  {}", game.description);
    let (style_prefix, style_name, style_desc) = if selected {
        (
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Gray),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default(),
            Style::default().fg(Color::DarkGray),
        )
    };
    Line::from(vec![
        Span::styled(prefix, style_prefix),
        Span::styled(name, style_name),
        Span::styled(desc, style_desc),
    ])
}
