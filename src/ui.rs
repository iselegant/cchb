use crate::app::{AppMode, AppState, DateField, Panel};
use crate::color::Theme;
use crate::markdown;
use crate::session::ContentBlock;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
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

fn render_conversation_view(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let border_style = if app.active_panel == Panel::ConversationView {
        theme.border_active
    } else {
        theme.border_inactive
    };

    // Clone session metadata before mutably borrowing app later
    let session_meta: Option<(String, String, Option<String>)> =
        app.selected_session().map(|session| {
            let session_id = session.session_id.clone();
            let project_path = session.project_path.clone();
            let branch = session.git_branch.clone();
            (session_id, project_path, branch)
        });

    // Split area: header (5 lines = border + 3 content lines + border) + conversation body
    let chunks = Layout::vertical([Constraint::Length(5), Constraint::Min(1)]).split(area);

    // Render session header pane
    let header_block = Block::default()
        .title(Line::from(Span::styled(" Conversation ", theme.title)))
        .borders(Borders::ALL)
        .border_style(border_style);

    if let Some((session_id, project_path, branch)) = &session_meta {
        let label_style = Style::default().fg(Color::Gray);
        let mut lines = vec![
            Line::from(vec![
                Span::styled("ID:        ", label_style),
                Span::styled(session_id.as_str(), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("Directory: ", label_style),
                Span::styled(project_path.as_str(), theme.session_project),
            ]),
        ];
        lines.push(if let Some(branch) = branch {
            Line::from(vec![
                Span::styled("Branch:    ", label_style),
                Span::styled(branch.as_str(), theme.session_branch),
            ])
        } else {
            Line::from(vec![
                Span::styled("Branch:    ", label_style),
                Span::styled("-", label_style),
            ])
        });
        let header = Paragraph::new(lines).block(header_block);
        frame.render_widget(header, chunks[0]);
    } else {
        let header = Paragraph::new("").block(header_block);
        frame.render_widget(header, chunks[0]);
    }

    // Conversation body
    let body_area = chunks[1];
    let body_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
        .border_style(border_style);

    if app.conversation.is_empty() {
        let placeholder = Paragraph::new("Select a session to view conversation")
            .block(body_block)
            .style(theme.session_preview);
        frame.render_widget(placeholder, body_area);
        return;
    }

    // Available width for markdown rendering:
    // panel inner width minus border prefix ("│ " = 2 chars) and content indent (2 chars)
    let inner_width = body_area.width.saturating_sub(2) as usize; // panel border
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
        let content_start = lines.len();
        for block_content in &msg.content_blocks {
            if let ContentBlock::Text(text) = block_content {
                let md_lines = markdown::render_markdown(text, base_style, theme, md_width);
                for md_line in md_lines {
                    let wrapped = markdown::wrap_line(md_line, md_width);
                    for wrapped_line in wrapped {
                        let mut spans = vec![Span::styled("│ ", border_style), Span::raw("  ")];
                        spans.extend(wrapped_line.spans);
                        lines.push(Line::from(spans));
                    }
                }
            }
        }
        // Remove trailing empty content lines (border prefix + whitespace only)
        while lines.len() > content_start {
            let last = &lines[lines.len() - 1];
            let text: String = last.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.trim().is_empty() || text.trim() == "│" {
                lines.pop();
            } else {
                break;
            }
        }
        // End of message: "└─"
        lines.push(Line::from(Span::styled("└─", border_style)));
    }

    let lines = if !app.search_query.is_empty() {
        let query_lower = app.search_query.to_lowercase();
        let mut match_positions: Vec<(usize, usize)> = Vec::new();
        // Collect all (line_index, occurrence_index) pairs
        for (i, line) in lines.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let text_lower = text.to_lowercase();
            let mut occ = 0;
            let mut start = 0;
            while let Some(pos) = text_lower[start..].find(&query_lower) {
                match_positions.push((i, occ));
                occ += 1;
                start += pos + query_lower.len();
            }
        }
        app.search_match_positions = match_positions;
        // Reset current index if it's out of bounds
        if let Some(idx) = app.search_match_current
            && idx >= app.search_match_positions.len()
        {
            app.search_match_current = None;
        }
        // Determine which line has the current match and which occurrence within it
        let current_highlight: Option<(usize, usize)> = app
            .search_match_current
            .map(|idx| app.search_match_positions[idx]);
        let lines: Vec<Line> = lines
            .into_iter()
            .enumerate()
            .map(|(i, line)| {
                if let Some((cur_line, cur_occ)) = current_highlight
                    && cur_line == i
                {
                    highlight_line_with_current(
                        line,
                        &app.search_query,
                        theme.search_highlight,
                        theme.search_highlight_current,
                        cur_occ,
                    )
                } else {
                    highlight_line(line, &app.search_query, theme.search_highlight)
                }
            })
            .collect();
        lines
    } else {
        app.search_match_positions.clear();
        app.search_match_current = None;
        lines
    };

    // Clamp scroll so content cannot scroll past the last line
    let visible_height = body_area.height.saturating_sub(2) as usize; // subtract border lines
    let total_lines = lines.len();
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.conversation_scroll = app.conversation_scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(body_block)
        .wrap(Wrap { trim: false })
        .scroll((app.conversation_scroll as u16, 0));

    frame.render_widget(paragraph, body_area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let session_count = app.filtered_indices.len();
    let total = app.sessions.len();

    let status_text = if session_count == total {
        format!(" {total} sessions")
    } else {
        format!(" {session_count}/{total} sessions")
    };

    let search_indicator = if !app.search_query.is_empty() {
        let match_info = if app.search_match_positions.is_empty() {
            String::new()
        } else if let Some(idx) = app.search_match_current {
            format!(" {}/{}", idx + 1, app.search_match_positions.len())
        } else {
            format!(" {}", app.search_match_positions.len())
        };
        format!("  [search: {}{}]", app.search_query, match_info)
    } else {
        String::new()
    };

    let hints = " r:resume  f:search  d:date  h:help  q:quit ";

    let left_len = status_text.len() + search_indicator.len();
    let fill_len = (area.width as usize)
        .saturating_sub(left_len)
        .saturating_sub(hints.len());

    let status = Paragraph::new(Line::from(vec![
        Span::styled(status_text, theme.status_bar),
        Span::styled(search_indicator, theme.search_input.bg(Color::DarkGray)),
        Span::styled(" ".repeat(fill_len), theme.status_bar),
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

    let mut spans = vec![
        Span::styled("> ", theme.search_input),
        Span::styled(&app.search_query, theme.search_input),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
    ];
    if app.search_cache_loading {
        spans.push(Span::styled(
            " (loading...)",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let input = Paragraph::new(Line::from(spans)).block(block);

    frame.render_widget(input, area);
}

fn render_date_filter_overlay(frame: &mut Frame, app: &AppState, theme: &Theme) {
    let area = centered_rect(50, 7, frame.area());
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
            " Up/Down: +/- 1 day  Tab: switch field",
            theme.help_desc,
        )),
        Line::from(Span::styled(" Enter: apply  Esc: cancel", theme.help_desc)),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_help_overlay(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(50, 20, frame.area());
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
        help_line("n / N", "Next / Prev search match (viewing)", theme),
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

/// Highlight all case-insensitive occurrences of `query` within a line's spans.
/// Matching portions are rendered with `highlight_style`.
fn highlight_line<'a>(line: Line<'a>, query: &str, highlight_style: Style) -> Line<'a> {
    if query.is_empty() {
        return line;
    }
    let query_lower = query.to_lowercase();
    let mut result_spans: Vec<Span<'a>> = Vec::new();

    for span in line.spans {
        let text = span.content.to_string();
        let text_lower = text.to_lowercase();
        let style = span.style;

        let mut pos = 0;
        let mut has_match = false;

        for (start, _) in text_lower.match_indices(&query_lower) {
            has_match = true;
            if start > pos {
                result_spans.push(Span::styled(text[pos..start].to_string(), style));
            }
            let end = start + query.len();
            result_spans.push(Span::styled(text[start..end].to_string(), highlight_style));
            pos = end;
        }

        if has_match {
            if pos < text.len() {
                result_spans.push(Span::styled(text[pos..].to_string(), style));
            }
        } else {
            result_spans.push(Span::styled(text, style));
        }
    }

    Line::from(result_spans)
}

/// Highlight all occurrences of `query`, using `current_style` for the `current_occ`-th
/// occurrence and `highlight_style` for all others.
fn highlight_line_with_current<'a>(
    line: Line<'a>,
    query: &str,
    highlight_style: Style,
    current_style: Style,
    current_occ: usize,
) -> Line<'a> {
    if query.is_empty() {
        return line;
    }
    let query_lower = query.to_lowercase();
    let mut result_spans: Vec<Span<'a>> = Vec::new();
    let mut global_occ: usize = 0;

    for span in line.spans {
        let text = span.content.to_string();
        let text_lower = text.to_lowercase();
        let style = span.style;

        let mut pos = 0;
        let mut has_match = false;

        for (start, _) in text_lower.match_indices(&query_lower) {
            has_match = true;
            if start > pos {
                result_spans.push(Span::styled(text[pos..start].to_string(), style));
            }
            let end = start + query.len();
            let hl = if global_occ == current_occ {
                current_style
            } else {
                highlight_style
            };
            result_spans.push(Span::styled(text[start..end].to_string(), hl));
            global_occ += 1;
            pos = end;
        }

        if has_match {
            if pos < text.len() {
                result_spans.push(Span::styled(text[pos..].to_string(), style));
            }
        } else {
            result_spans.push(Span::styled(text, style));
        }
    }

    Line::from(result_spans)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hl_style() -> Style {
        Style::default().fg(Color::Black).bg(Color::Yellow)
    }

    #[test]
    fn test_highlight_line_no_match() {
        let line = Line::from("hello world");
        let result = highlight_line(line, "xyz", hl_style());
        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn test_highlight_line_empty_query() {
        let line = Line::from("hello world");
        let result = highlight_line(line, "", hl_style());
        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn test_highlight_line_single_match() {
        let line = Line::from("hello world");
        let result = highlight_line(line, "world", hl_style());
        assert_eq!(result.spans.len(), 2);
        assert_eq!(result.spans[0].content.as_ref(), "hello ");
        assert_eq!(result.spans[1].content.as_ref(), "world");
        assert_eq!(result.spans[1].style, hl_style());
    }

    #[test]
    fn test_highlight_line_case_insensitive() {
        let line = Line::from("Hello World");
        let result = highlight_line(line, "hello", hl_style());
        assert_eq!(result.spans.len(), 2);
        assert_eq!(result.spans[0].content.as_ref(), "Hello");
        assert_eq!(result.spans[0].style, hl_style());
        assert_eq!(result.spans[1].content.as_ref(), " World");
    }

    #[test]
    fn test_highlight_line_multiple_matches() {
        let line = Line::from("foo bar foo baz foo");
        let result = highlight_line(line, "foo", hl_style());
        assert_eq!(result.spans.len(), 5);
        assert_eq!(result.spans[0].content.as_ref(), "foo");
        assert_eq!(result.spans[0].style, hl_style());
        assert_eq!(result.spans[1].content.as_ref(), " bar ");
        assert_eq!(result.spans[2].content.as_ref(), "foo");
        assert_eq!(result.spans[2].style, hl_style());
        assert_eq!(result.spans[3].content.as_ref(), " baz ");
        assert_eq!(result.spans[4].content.as_ref(), "foo");
        assert_eq!(result.spans[4].style, hl_style());
    }

    #[test]
    fn test_highlight_line_across_styled_spans() {
        let base = Style::default().fg(Color::Green);
        let line = Line::from(vec![
            Span::styled("hello ", base),
            Span::styled("world test", base),
        ]);
        let result = highlight_line(line, "world", hl_style());
        // "hello " (no match) + "world" (highlight) + " test" (base)
        assert_eq!(result.spans.len(), 3);
        assert_eq!(result.spans[0].content.as_ref(), "hello ");
        assert_eq!(result.spans[0].style, base);
        assert_eq!(result.spans[1].content.as_ref(), "world");
        assert_eq!(result.spans[1].style, hl_style());
        assert_eq!(result.spans[2].content.as_ref(), " test");
        assert_eq!(result.spans[2].style, base);
    }

    #[test]
    fn test_highlight_line_match_at_start() {
        let line = Line::from("terraform plan");
        let result = highlight_line(line, "terraform", hl_style());
        assert_eq!(result.spans.len(), 2);
        assert_eq!(result.spans[0].content.as_ref(), "terraform");
        assert_eq!(result.spans[0].style, hl_style());
        assert_eq!(result.spans[1].content.as_ref(), " plan");
    }

    #[test]
    fn test_highlight_line_match_at_end() {
        let line = Line::from("run terraform");
        let result = highlight_line(line, "terraform", hl_style());
        assert_eq!(result.spans.len(), 2);
        assert_eq!(result.spans[0].content.as_ref(), "run ");
        assert_eq!(result.spans[1].content.as_ref(), "terraform");
        assert_eq!(result.spans[1].style, hl_style());
    }
}
