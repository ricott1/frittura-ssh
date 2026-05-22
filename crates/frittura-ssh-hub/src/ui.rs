use crate::config::{AnimatedFrame, AnimatedPreview, GameMetadata, Preview};
use crate::utils::{big_text, FRITTURA_YELLOW, TITLE, TITLE_HEIGHT};
use frittura_ssh_core::idle_warning_text;
use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
    Frame,
};
use std::time::Duration;

pub fn render_lobby_menu(
    frame: &mut Frame,
    username: &str,
    games: &[GameMetadata],
    selected_idx: usize,
    kick_warning_secs: Option<u32>,
    flash: Option<&str>,
    selection_elapsed: Duration,
) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(FRITTURA_YELLOW)),
        area,
    );

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(TITLE_HEIGHT), // big title
        Constraint::Length(1),
        Constraint::Length(1), // username / flash row
        Constraint::Length(1),
        Constraint::Fill(1), // body: games | preview
        Constraint::Length(1),
        Constraint::Length(1), // kick warning slot
        Constraint::Length(1),
        Constraint::Length(1), // controls hint
    ])
    .split(area.inner(Margin::new(2, 1)));

    frame.render_widget(
        big_text(&TITLE, FRITTURA_YELLOW, Color::DarkGray),
        chunks[1],
    );

    let greeting = if let Some(msg) = flash {
        Line::styled(
            msg.to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        Line::from(vec![
            Span::raw("Hello "),
            Span::styled(
                username.to_string(),
                Style::default()
                    .fg(FRITTURA_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("!"),
        ])
    };
    frame.render_widget(greeting.centered(), chunks[3]);

    let body = chunks[5];
    let body_split =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).split(body);
    render_games_pane(frame, body_split[0], games, selected_idx);
    render_preview_pane(
        frame,
        body_split[1],
        games.get(selected_idx),
        selection_elapsed,
    );

    if let Some(secs) = kick_warning_secs {
        let banner_w: u16 = 50;
        let banner_h: u16 = 3;
        let banner = Rect {
            x: area.x + area.width.saturating_sub(banner_w) / 2,
            y: chunks[7].y.saturating_sub(1),
            width: banner_w.min(area.width),
            height: banner_h.min(area.height),
        };
        frame.render_widget(Clear, banner);
        frame.render_widget(
            Paragraph::new(idle_warning_text(secs))
                .centered()
                .style(Style::new().red().bold())
                .block(Block::bordered()),
            banner,
        );
    }

    frame.render_widget(
        Line::styled(
            "↑/↓ or j/k: move    Enter: connect    Esc: leave",
            Style::default().fg(Color::DarkGray),
        )
        .centered(),
        chunks[9],
    );
}

fn render_games_pane(frame: &mut Frame, area: Rect, games: &[GameMetadata], selected_idx: usize) {
    let mut lines: Vec<Line<'static>> = vec![];
    if games.is_empty() {
        lines.push(Line::styled(
            "(no games configured)",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        for (i, g) in games.iter().enumerate() {
            let is_selected = i == selected_idx;
            let (marker, name_style, desc_style) = if is_selected {
                (
                    "▶ ",
                    Style::default()
                        .fg(FRITTURA_YELLOW)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Gray),
                )
            } else {
                (
                    "  ",
                    Style::default().fg(Color::White),
                    Style::default().fg(Color::DarkGray),
                )
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}. ", marker, i + 1), name_style),
                Span::styled(g.name.clone(), name_style),
            ]));
            lines.push(Line::styled(format!("     {}", g.description), desc_style));
            lines.push(Line::raw(""));
        }
    }
    frame.render_widget(Paragraph::new(lines), area.inner(Margin::new(1, 1)));
}

fn render_preview_pane(
    frame: &mut Frame,
    area: Rect,
    game: Option<&GameMetadata>,
    elapsed: Duration,
) {
    let content_lines: Vec<Line<'static>> = match game.and_then(|g| g.preview.as_ref()) {
        Some(Preview::Static(lines)) => lines.clone(),
        Some(Preview::Animated(anim)) => pick_frame(anim, elapsed).lines.clone(),
        None => {
            let body = game.map(|g| g.description.clone()).unwrap_or_default();
            vec![
                Line::raw(""),
                Line::styled(body, Style::default().fg(Color::DarkGray)).centered(),
            ]
        }
    };

    // Bias the preview toward the top of the pane so it sits below the title
    // rather than dead-centered with empty space above.
    let v_offset = area.height.saturating_sub(content_lines.len() as u16) / 4;
    frame.render_widget(
        Paragraph::new(content_lines).centered(),
        area.inner(Margin::new(1, 1 + v_offset)),
    );
}

fn pick_frame(anim: &AnimatedPreview, elapsed: Duration) -> &AnimatedFrame {
    if anim.total.is_zero() {
        return &anim.frames[0];
    }
    let t = Duration::from_nanos((elapsed.as_nanos() % anim.total.as_nanos()) as u64);
    let mut acc = Duration::ZERO;
    for f in &anim.frames {
        acc += f.delay;
        if t < acc {
            return f;
        }
    }
    anim.frames.last().expect("frames non-empty")
}
