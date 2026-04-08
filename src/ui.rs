use crate::app::{AppMode, AppState, DateField, Panel};
use crate::color::Theme;
use crate::markdown;
use crate::session::ContentBlock;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

pub fn render(frame: &mut Frame, app: &mut AppState, theme: &Theme) {
    let outer = Layout::vertical([
        Constraint::Length(1), // title bar
        Constraint::Min(3),    // main content
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    render_title_bar(frame, outer[0], theme);
    render_main_content(frame, outer[1], app, theme);
    render_status_bar(frame, outer[2], app, theme);

    // Overlays
    match app.mode {
        AppMode::FuzzySearch => render_search_overlay(frame, app, theme),
        AppMode::DateFilter => render_date_filter_overlay(frame, app, theme),
        AppMode::Help => render_help_overlay(frame, theme),
        _ => {}
    }
}

fn render_title_bar(frame: &mut Frame, area: Rect, theme: &Theme) {
    let title = Paragraph::new(Line::from(vec![Span::styled(
        " cchist - Claude Code History Browser",
        theme.title,
    )]));
    frame.render_widget(title, area);
}

fn render_main_content(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).split(area);

    render_session_list(frame, chunks[0], app, theme);
    render_conversation_view(frame, chunks[1], app, theme);
}

fn render_session_list(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let border_style = if app.active_panel == Panel::SessionList {
        theme.border_active
    } else {
        theme.border_inactive
    };

    let block = Block::default()
        .title(" Sessions ")
        .borders(Borders::ALL)
        .border_style(border_style);

    // Calculate items_per_page from actual panel height (2 lines for border)
    let inner_height = area.height.saturating_sub(2) as usize;
    let lines_per_item = 4; // project+branch, date, preview, blank
    app.items_per_page = (inner_height / lines_per_item).max(1);

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .map(|&session_idx| {
            let session = &app.sessions[session_idx];

            let mut first_line_spans = vec![Span::styled(
                &session.project_display,
                theme.session_project,
            )];
            if let Some(ref branch) = session.git_branch {
                first_line_spans.push(Span::raw(" "));
                first_line_spans.push(Span::styled(format!("({branch})"), theme.session_branch));
            }

            let date_str = session.modified.format("%Y-%m-%d %H:%M").to_string();
            let second_line = Line::from(vec![Span::styled(date_str, theme.session_date)]);

            let preview: String = if session.first_prompt.is_empty() {
                "(no prompt)".to_string()
            } else {
                session.first_prompt.chars().take(60).collect()
            };
            let third_line = Line::from(vec![Span::styled(preview, theme.session_preview)]);

            ListItem::new(vec![
                Line::from(first_line_spans),
                second_line,
                third_line,
                Line::from(""),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme.session_selected)
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_conversation_view(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let border_style = if app.active_panel == Panel::ConversationView {
        theme.border_active
    } else {
        theme.border_inactive
    };

    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.conversation.is_empty() {
        let placeholder = Paragraph::new("Select a session to view conversation")
            .block(block)
            .style(theme.session_preview);
        frame.render_widget(placeholder, area);
        return;
    }

    // Available width for markdown rendering:
    // panel inner width minus border prefix ("│ " = 2 chars) and content indent (2 chars)
    let inner_width = area.width.saturating_sub(2) as usize; // panel border
    let border_prefix_width = 2; // "│ "
    let content_indent_width = 2; // "  "
    let md_width = inner_width
        .saturating_sub(border_prefix_width)
        .saturating_sub(content_indent_width);

    let mut lines: Vec<Line> = Vec::new();
    for msg in &app.conversation {
        if msg.is_sidechain {
            continue;
        }
        let (label, label_style, base_style, border_style) = match msg.role.as_str() {
            "user" => (
                "You:",
                theme.user_label,
                theme.user_message,
                theme.user_border,
            ),
            "assistant" => (
                "Claude:",
                theme.assistant_label,
                theme.assistant_message,
                theme.assistant_border,
            ),
            _ => continue,
        };

        // Label line with border prefix: "│ You:" or "│ Claude:"
        lines.push(Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(label, label_style),
        ]));
        // Content lines with border prefix: "│   content"
        for block_content in &msg.content_blocks {
            if let ContentBlock::Text(text) = block_content {
                let md_lines = markdown::render_markdown(text, base_style, theme, md_width);
                for md_line in md_lines {
                    let mut spans = vec![Span::styled("│ ", border_style), Span::raw("  ")];
                    spans.extend(md_line.spans);
                    lines.push(Line::from(spans));
                }
            }
        }
        // End of message: "└─"
        lines.push(Line::from(Span::styled("└─", border_style)));
        // Blank separator line
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.conversation_scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let session_count = app.filtered_indices.len();
    let total = app.sessions.len();

    let status_text = if session_count == total {
        format!(" {total} sessions")
    } else {
        format!(" {session_count}/{total} sessions")
    };

    let hints = " r:resume  f:search  d:date  h:help  q:quit ";

    let status = Paragraph::new(Line::from(vec![
        Span::styled(status_text, theme.status_bar),
        Span::styled(
            " ".repeat(area.width.saturating_sub(30) as usize),
            theme.status_bar,
        ),
        Span::styled(hints, theme.status_bar),
    ]));

    frame.render_widget(status, area);
}

fn render_search_overlay(frame: &mut Frame, app: &AppState, theme: &Theme) {
    let area = centered_rect(50, 3, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_style(theme.border_active);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("> ", theme.search_input),
        Span::styled(&app.search_query, theme.search_input),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
    ]))
    .block(block);

    frame.render_widget(input, area);
}

fn render_date_filter_overlay(frame: &mut Frame, app: &AppState, theme: &Theme) {
    let area = centered_rect(40, 6, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Date Filter ")
        .borders(Borders::ALL)
        .border_style(theme.border_active);

    let from_indicator = if app.date_field == DateField::From {
        "> "
    } else {
        "  "
    };
    let to_indicator = if app.date_field == DateField::To {
        "> "
    } else {
        "  "
    };

    let text = vec![
        Line::from(vec![
            Span::raw(from_indicator),
            Span::styled("From: ", theme.help_key),
            Span::styled(&app.date_from_input, theme.search_input),
        ]),
        Line::from(vec![
            Span::raw(to_indicator),
            Span::styled("To:   ", theme.help_key),
            Span::styled(&app.date_to_input, theme.search_input),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Enter: apply  Tab: switch  Esc: cancel",
            theme.help_desc,
        )),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_help_overlay(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(50, 19, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(theme.border_active);

    let help_lines = vec![
        help_line("j / k", "Move down / up", theme),
        help_line("g / G", "Jump to top / bottom", theme),
        help_line("Right / Left", "Next / Previous page", theme),
        help_line("Ctrl+d / Ctrl+u", "Half page down / up", theme),
        help_line("Enter / l", "Open session", theme),
        help_line("Esc / q", "Back / Quit", theme),
        help_line("Tab", "Switch panel focus", theme),
        help_line("f", "Fuzzy search sessions", theme),
        help_line("d", "Filter by date range", theme),
        help_line("c", "Clear all filters", theme),
        help_line("r", "Reload sessions", theme),
        help_line("[ / ]", "Prev / Next session (viewing)", theme),
        Line::from(""),
        Line::from(Span::styled(" Press any key to close", theme.help_desc)),
    ];

    let paragraph = Paragraph::new(help_lines).block(block);
    frame.render_widget(paragraph, area);
}

fn help_line<'a>(key: &'a str, desc: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{key:>17}"), theme.help_key),
        Span::raw("  "),
        Span::styled(desc, theme.help_desc),
    ])
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Length((area.height.saturating_sub(height)) / 2),
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .split(area);

    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1]);

    horizontal[1]
}
