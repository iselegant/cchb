use crate::app::{AppMode, AppState, DateField, Panel};
use crate::color::Theme;
use crate::filter;
use crate::markdown;
use crate::session::ContentBlock;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

/// Compute scrollbar thumb geometry with a fixed thumb size that does not
/// fluctuate across scroll positions.
///
/// Returns `(thumb_start, thumb_size)` in track cells.
///
/// - `track_length`: number of cells available for the scrollbar track
/// - `total_content`: total number of items or lines
/// - `viewport`: number of visible items or lines
/// - `position`: current scroll offset (`0..=total_content - viewport`)
fn scrollbar_geometry(
    track_length: usize,
    total_content: usize,
    viewport: usize,
    position: usize,
) -> (usize, usize) {
    if track_length == 0 {
        return (0, 0);
    }
    if total_content <= viewport {
        return (0, track_length);
    }
    let max_scroll = total_content - viewport;
    let thumb_size = (viewport * track_length / total_content).max(1);
    let max_thumb_start = track_length - thumb_size;
    let thumb_start = if max_scroll == 0 {
        0
    } else {
        (position * max_thumb_start + max_scroll / 2) / max_scroll
    };
    (thumb_start.min(max_thumb_start), thumb_size)
}

/// Render a scrollbar on the right edge of `area` with a fixed-size thumb.
/// Uses `scrollbar_geometry` to avoid the ±1 cell thumb drift caused by
/// ratatui's independent rounding of thumb start and end positions.
fn render_fixed_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total_content: usize,
    viewport: usize,
    position: usize,
) {
    let track_length = area.height as usize;
    let (thumb_start, thumb_size) =
        scrollbar_geometry(track_length, total_content, viewport, position);

    let x = area.right().saturating_sub(1);
    let track_symbol = "║";
    let thumb_symbol = "█";

    for i in 0..track_length {
        let y = area.y + i as u16;
        let (symbol, style) = if i >= thumb_start && i < thumb_start + thumb_size {
            (thumb_symbol, Style::default())
        } else {
            (track_symbol, Style::default().fg(Color::DarkGray))
        };
        frame.buffer_mut()[(x, y)]
            .set_symbol(symbol)
            .set_style(style);
    }
}

pub fn render(frame: &mut Frame, app: &mut AppState, theme: &Theme) {
    let outer = Layout::vertical([
        Constraint::Length(3), // title bar (ASCII art)
        Constraint::Min(3),    // main content
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    render_title_bar(frame, outer[0], app, theme);
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

/// Color palette for the sparkle animation — cycles through these per letter group.
const SPARKLE_COLORS: [Color; 6] = [
    Color::LightRed,
    Color::LightYellow,
    Color::LightGreen,
    Color::LightCyan,
    Color::LightMagenta,
    Color::White,
];

fn render_title_bar(frame: &mut Frame, area: Rect, app: &mut AppState, _theme: &Theme) {
    let sparkling = app.is_logo_sparkling();

    let (c1, c2, c3, c4) = if sparkling {
        // Derive a phase from elapsed time — shift every 150ms for a shimmering effect
        let elapsed_ms = app
            .logo_sparkle_start
            .map(|s| s.elapsed().as_millis() as usize)
            .unwrap_or(0);
        let phase = elapsed_ms / 150;
        let len = SPARKLE_COLORS.len();
        let pick = |offset: usize| {
            Style::default()
                .fg(SPARKLE_COLORS[(phase + offset) % len])
                .add_modifier(Modifier::BOLD)
        };
        (pick(0), pick(1), pick(2), pick(3))
    } else {
        (
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    };
    let dim = Style::default().fg(Color::DarkGray);

    let version = format!("  v{}", env!("CARGO_PKG_VERSION"));

    let lines = vec![
        Line::from(vec![
            Span::styled(" ▄▀▀ ", c1),
            Span::styled("▄▀▀ ", c2),
            Span::styled("█ █ ", c3),
            Span::styled("█▀▄", c4),
        ]),
        Line::from(vec![
            Span::styled(" █   ", c1),
            Span::styled("█   ", c2),
            Span::styled("█▀█ ", c3),
            Span::styled("█▀█", c4),
        ]),
        Line::from(vec![
            Span::styled(" ▀▀▀ ", c1),
            Span::styled("▀▀▀ ", c2),
            Span::styled("▀ ▀ ", c3),
            Span::styled("▀▀ ", c4),
            Span::styled(&version, dim),
        ]),
    ];

    let title = Paragraph::new(lines);
    frame.render_widget(title, area);
}

fn render_main_content(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).split(area);

    app.panel_geometry.session_list = Some(chunks[0]);
    render_session_list(frame, chunks[0], app, theme);
    render_conversation_view(frame, chunks[1], app, theme);
}

fn render_session_list(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let border_style = if app.active_panel == Panel::SessionList {
        theme.border_active
    } else {
        theme.border_inactive
    };

    let title = if app.active_panel == Panel::SessionList {
        Line::from(Span::styled(" ▶ Sessions ", theme.border_active))
    } else {
        Line::from(Span::styled("   Sessions ", theme.border_inactive))
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    // Calculate items_per_page from actual panel height (2 lines for border)
    let inner_height = area.height.saturating_sub(2) as usize;
    let lines_per_item = 4; // project+branch, date, preview, blank
    app.items_per_page = (inner_height / lines_per_item).max(1);

    if app.session_loading {
        let loading_block = block.clone();
        let loading_text = Paragraph::new(Line::from(Span::styled(
            "  Loading sessions...",
            theme.session_date,
        )))
        .block(loading_block);
        frame.render_widget(loading_text, area);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .map(|&session_idx| {
            let session = &app.sessions[session_idx];

            let mut first_line_spans = vec![Span::styled(
                &session.project_display,
                theme.session_project,
            )];
            if !session.branch_display.is_empty() {
                first_line_spans.push(Span::raw(" "));
                first_line_spans.push(Span::styled(&session.branch_display, theme.session_branch));
            }
            let second_line = Line::from(vec![Span::styled(
                &session.date_display,
                theme.session_date,
            )]);

            let third_line = Line::from(vec![Span::styled(
                &session.prompt_preview,
                theme.session_preview,
            )]);

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

    // Render scrollbar
    let total_items = app.filtered_indices.len();
    if total_items > app.items_per_page {
        render_fixed_scrollbar(
            frame,
            area,
            total_items,
            app.items_per_page,
            app.list_state.offset(),
        );
    }
}

fn render_conversation_view(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let border_style = if app.active_panel == Panel::ConversationView {
        theme.border_active
    } else {
        theme.border_inactive
    };

    // Split area: header (5 lines = border + 3 content lines + border) + conversation body
    let chunks = Layout::vertical([Constraint::Length(5), Constraint::Min(1)]).split(area);

    // Render session header pane.
    // Borrow session data immutably and render the header widget here so the borrow
    // is dropped before any mutable access to `app` later.  This avoids cloning
    // session_id / project_path / git_branch every frame.
    let conv_title = if app.active_panel == Panel::ConversationView {
        Line::from(Span::styled(" ▶ Conversation ", theme.border_active))
    } else {
        Line::from(Span::styled("   Conversation ", theme.border_inactive))
    };
    let header_block = Block::default()
        .title(conv_title)
        .borders(Borders::ALL)
        .border_style(border_style);

    if let Some(session) = app.selected_session() {
        let label_style = Style::default().fg(Color::Gray);
        let mut lines = vec![
            Line::from(vec![
                Span::styled("ID:        ", label_style),
                Span::styled(
                    session.session_id.as_str(),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::styled("Directory: ", label_style),
                Span::styled(session.project_path.as_str(), theme.session_project),
            ]),
        ];
        lines.push(if let Some(branch) = &session.git_branch {
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
    // panel inner width minus border prefix ("│ " = 2 chars), content indent (2 chars),
    // and scrollbar (1 char)
    let inner_width = body_area.width.saturating_sub(2) as usize; // panel border
    let border_prefix_width = 2; // "│ "
    let content_indent_width = 2; // "  "
    let scrollbar_width = 1;
    let md_width = inner_width
        .saturating_sub(border_prefix_width)
        .saturating_sub(content_indent_width)
        .saturating_sub(scrollbar_width);

    // Use cached lines if cache key matches, otherwise rebuild and cache.
    let current_cache_key = (app.loaded_session_id.clone(), body_area.width);
    if app.conversation_cache_key != current_cache_key {
        let mut lines: Vec<Line<'static>> = Vec::new();
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
        app.conversation_lines_cache = lines;
        app.conversation_cache_key = current_cache_key;
    }

    // Compute viewport dimensions and clamp scroll BEFORE cloning any lines.
    let content_area = body_block.inner(body_area);
    app.panel_geometry.conversation_body = Some(content_area);
    let visible_height = content_area.height as usize;
    let total_lines = app.conversation_lines_cache.len();
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.conversation_scroll = app.conversation_scroll.min(max_scroll);

    let scroll = app.conversation_scroll;
    let visible_end = (scroll + visible_height).min(total_lines);

    let visible_lines = if !app.search_query.is_empty() {
        // Use cached lowercased query to avoid re-allocating every frame.
        let query_lower = app.search_query_lower().to_string();

        // Recompute match positions only when query or conversation changed.
        let match_cache_key = (app.search_query.clone(), app.conversation_cache_key.clone());
        if app.search_match_cache_key != match_cache_key {
            let mut match_positions: Vec<(usize, usize)> = Vec::new();
            for (i, line) in app.conversation_lines_cache.iter().enumerate() {
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
            app.search_match_cache_key = match_cache_key;
            // Reset current index if it's out of bounds
            if let Some(idx) = app.search_match_current
                && idx >= app.search_match_positions.len()
            {
                app.search_match_current = None;
            }
        }

        // Determine which line has the current match and which occurrence within it
        let current_highlight: Option<(usize, usize)> = app
            .search_match_current
            .map(|idx| app.search_match_positions[idx]);

        // Clone and highlight only the visible lines instead of the entire cache.
        app.conversation_lines_cache[scroll..visible_end]
            .iter()
            .enumerate()
            .map(|(vi, line)| {
                let i = scroll + vi;
                let line = line.clone();
                if let Some((cur_line, cur_occ)) = current_highlight
                    && cur_line == i
                {
                    highlight_line_with_current(
                        line,
                        &query_lower,
                        theme.search_highlight,
                        theme.search_highlight_current,
                        cur_occ,
                    )
                } else {
                    highlight_line(line, &query_lower, theme.search_highlight)
                }
            })
            .collect::<Vec<Line>>()
    } else {
        app.search_match_positions.clear();
        app.search_match_current = None;
        // Clone only the visible portion of the cache.
        app.conversation_lines_cache[scroll..visible_end].to_vec()
    };

    // Apply text selection highlighting if active.
    let visible_lines = if let Some(ref sel) = app.text_selection {
        if !sel.is_empty() {
            apply_selection_highlight(visible_lines, sel, scroll, theme.text_selection)
        } else {
            visible_lines
        }
    } else {
        visible_lines
    };

    let paragraph = Paragraph::new(visible_lines).block(body_block);

    frame.render_widget(paragraph, body_area);

    // Render scrollbar on body_area so it overlaps the right border.
    if total_lines > visible_height {
        render_fixed_scrollbar(
            frame,
            body_area,
            total_lines,
            visible_height,
            app.conversation_scroll,
        );
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    let session_count = app.filtered_indices.len();
    let total = app.sessions.len();

    let status_text = if app.session_loading {
        " Loading...".to_string()
    } else if session_count == total {
        format!(" {total} sessions")
    } else {
        format!(" {session_count}/{total} sessions")
    };

    let reload_indicator = if app.conversation_reloading {
        "  [Reloaded!]"
    } else {
        ""
    };

    let clipboard_indicator = if app.clipboard_flash_at.is_some() {
        "  [Copied!]"
    } else {
        ""
    };

    let search_indicator = if !app.search_query.is_empty() {
        // Recompute total matches across all sessions only when query or cache changes.
        let total_key = (app.search_query.clone(), app.search_content_cache.len());
        if app.search_total_matches_key != total_key {
            app.search_total_matches =
                filter::count_total_search_matches(&app.search_query, &app.search_content_cache);
            app.search_total_matches_key = total_key;
        }

        let conv_info = if app.search_match_positions.is_empty() {
            String::new()
        } else if let Some(idx) = app.search_match_current {
            format!(" | conv: {}/{}", idx + 1, app.search_match_positions.len())
        } else {
            format!(" | conv: {}", app.search_match_positions.len())
        };
        format!(
            "  [search: {} | all: {}{}]",
            app.search_query, app.search_total_matches, conv_info
        )
    } else {
        String::new()
    };

    let has_filters = !app.search_query.is_empty()
        || !app.date_from_input.is_empty()
        || !app.date_to_input.is_empty();

    let hints = match (app.mode == AppMode::Viewing, has_filters) {
        (true, true) => {
            " Enter:resume  Tab:panel  l:reload  f|/:search  d:date  c:clear  n/N:match  h:help  Esc/q:back "
        }
        (true, false) => {
            " Enter:resume  Tab:panel  l:reload  f|/:search  d:date  h:help  Esc/q:back "
        }
        (false, true) => {
            " Enter:resume  Tab:panel  l:reload  f|/:search  d:date  c:clear  h:help  Esc/q:quit "
        }
        (false, false) => {
            " Enter:resume  Tab:panel  l:reload  f|/:search  d:date  h:help  Esc/q:quit "
        }
    };

    let left_len = status_text.len()
        + reload_indicator.len()
        + clipboard_indicator.len()
        + search_indicator.len();
    let fill_len = (area.width as usize)
        .saturating_sub(left_len)
        .saturating_sub(hints.len());

    let status = Paragraph::new(Line::from(vec![
        Span::styled(status_text, theme.status_bar),
        Span::styled(
            reload_indicator,
            Style::default()
                .fg(Color::Green)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            clipboard_indicator,
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
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

    // Position the terminal cursor at the text input location so that
    // IME candidate windows (e.g. Japanese input) appear near the search box
    // instead of at the default terminal cursor position (bottom-right).
    // inner() accounts for the 1-cell border on each side.
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    let query_width = unicode_width::UnicodeWidthStr::width(app.search_query.as_str()) as u16;
    let cursor_x = inner.x + 2 + query_width; // 2 = "> " prefix
    let cursor_y = inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));
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
    let area = centered_rect(50, 22, frame.area());
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
        help_line("Enter", "Resume selected session", theme),
        help_line("Esc / q", "Back / Quit", theme),
        help_line("Tab", "Switch panel focus", theme),
        help_line("l", "Reload conversation", theme),
        help_line("f / /", "Fuzzy search sessions", theme),
        help_line("d", "Filter by date range", theme),
        help_line("c", "Clear all filters", theme),
        help_line("R", "Reload session list", theme),
        help_line("[ / ]", "Prev / Next session (viewing)", theme),
        help_line(
            "n / N",
            "Next / Prev match (cross-session in session panel)",
            theme,
        ),
        help_line("y", "Copy selected text", theme),
        help_line("Mouse drag", "Select text in conversation", theme),
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
/// `query_lower` must be the pre-lowercased form of the original query.
/// Matching portions are rendered with `highlight_style`.
fn highlight_line<'a>(line: Line<'a>, query_lower: &str, highlight_style: Style) -> Line<'a> {
    if query_lower.is_empty() {
        return line;
    }
    let query_len = query_lower.len();
    let mut result_spans: Vec<Span<'a>> = Vec::new();

    for span in line.spans {
        let style = span.style;
        let text_lower = span.content.to_lowercase();

        if !text_lower.contains(query_lower) {
            // Fast path: no match in this span, keep as-is without allocating
            result_spans.push(span);
            continue;
        }

        // Slow path: matches exist, need to split the span
        let text = span.content.into_owned();
        let mut pos = 0;

        for (start, _) in text_lower.match_indices(query_lower) {
            if start > pos {
                result_spans.push(Span::styled(text[pos..start].to_string(), style));
            }
            let end = start + query_len;
            result_spans.push(Span::styled(text[start..end].to_string(), highlight_style));
            pos = end;
        }

        if pos < text.len() {
            result_spans.push(Span::styled(text[pos..].to_string(), style));
        }
    }

    Line::from(result_spans)
}

/// Highlight all occurrences of `query`, using `current_style` for the `current_occ`-th
/// occurrence and `highlight_style` for all others.
/// `query_lower` must be the pre-lowercased form of the original query.
fn highlight_line_with_current<'a>(
    line: Line<'a>,
    query_lower: &str,
    highlight_style: Style,
    current_style: Style,
    current_occ: usize,
) -> Line<'a> {
    if query_lower.is_empty() {
        return line;
    }
    let query_len = query_lower.len();
    let mut result_spans: Vec<Span<'a>> = Vec::new();
    let mut global_occ: usize = 0;

    for span in line.spans {
        let style = span.style;
        let text_lower = span.content.to_lowercase();

        if !text_lower.contains(query_lower) {
            // Fast path: no match in this span, keep as-is
            result_spans.push(span);
            continue;
        }

        // Slow path: matches exist, need to split the span
        let text = span.content.into_owned();
        let mut pos = 0;

        for (start, _) in text_lower.match_indices(query_lower) {
            if start > pos {
                result_spans.push(Span::styled(text[pos..start].to_string(), style));
            }
            let end = start + query_len;
            let hl = if global_occ == current_occ {
                current_style
            } else {
                highlight_style
            };
            result_spans.push(Span::styled(text[start..end].to_string(), hl));
            global_occ += 1;
            pos = end;
        }

        if pos < text.len() {
            result_spans.push(Span::styled(text[pos..].to_string(), style));
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

/// Apply selection highlighting to visible lines.
/// `scroll` is the current conversation scroll offset.
fn apply_selection_highlight<'a>(
    lines: Vec<Line<'a>>,
    sel: &crate::app::TextSelection,
    scroll: usize,
    selection_style: Style,
) -> Vec<Line<'a>> {
    let (sel_start, sel_end) = sel.ordered();
    lines
        .into_iter()
        .enumerate()
        .map(|(vi, line)| {
            let abs_line = scroll + vi;
            if abs_line < sel_start.line || abs_line > sel_end.line {
                return line;
            }
            let start_col = if abs_line == sel_start.line {
                sel_start.col
            } else {
                0
            };
            let end_col = if abs_line == sel_end.line {
                sel_end.col
            } else {
                usize::MAX
            };
            if start_col == end_col {
                return line;
            }
            highlight_selection_range(line, start_col, end_col, selection_style)
        })
        .collect()
}

/// Highlight characters in the column range [start_col, end_col) within a line.
/// Column positions are based on display width (accounting for the border prefix).
/// The prefix occupies 4 display columns ("│ " + "  "), and selection columns
/// are relative to the content after that prefix.
fn highlight_selection_range<'a>(
    line: Line<'a>,
    start_col: usize,
    end_col: usize,
    selection_style: Style,
) -> Line<'a> {
    use unicode_width::UnicodeWidthChar;

    let mut result_spans: Vec<Span<'a>> = Vec::new();
    // Track display-width position, starting negative to skip the 4-col border prefix.
    let mut col: isize = -4;

    for span in line.spans {
        let style = span.style;
        let text = span.content.into_owned();
        let mut span_start = 0;

        for (byte_idx, ch) in text.char_indices() {
            let w = ch.width().unwrap_or(0) as isize;
            let char_end = byte_idx + ch.len_utf8();

            if col >= 0 {
                let ucol = col as usize;
                // Check if this character crosses a selection boundary.
                if ucol == start_col && span_start < byte_idx {
                    // Emit pre-selection portion.
                    result_spans.push(Span::styled(text[span_start..byte_idx].to_string(), style));
                    span_start = byte_idx;
                }
                if ucol == end_col && span_start < byte_idx {
                    // Emit selection portion.
                    result_spans.push(Span::styled(
                        text[span_start..byte_idx].to_string(),
                        selection_style,
                    ));
                    span_start = byte_idx;
                }
            } else if col + w > 0 {
                // This character straddles the prefix boundary — treat content starting here.
                // The remaining characters are content.
            }

            col += w;

            // Handle case where we're past end_col for remaining chars.
            if col > 0 && (col as usize) > end_col && span_start < char_end {
                // Already past selection. Check if we need to close selection span.
                let ucol_prev = (col - w) as usize;
                if ucol_prev < end_col && ucol_prev >= start_col {
                    // This char was the last selected char.
                    result_spans.push(Span::styled(
                        text[span_start..char_end].to_string(),
                        selection_style,
                    ));
                    span_start = char_end;
                }
            }
        }

        // Emit remaining text in this span.
        if span_start < text.len() {
            let ucol_start = col.saturating_sub(
                text[span_start..]
                    .chars()
                    .map(|c| c.width().unwrap_or(0) as isize)
                    .sum::<isize>(),
            );
            let in_selection = ucol_start >= 0
                && (ucol_start as usize) >= start_col
                && (ucol_start as usize) < end_col;
            if in_selection {
                result_spans.push(Span::styled(
                    text[span_start..].to_string(),
                    selection_style,
                ));
            } else {
                result_spans.push(Span::styled(text[span_start..].to_string(), style));
            }
        }
    }

    Line::from(result_spans)
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

    #[test]
    fn test_scrollbar_geometry_thumb_size_constant_across_positions() {
        // track=18, total=30, viewport=4 → thumb must be the same size at every position
        let max_scroll = 30 - 4;
        let mut sizes = Vec::new();
        for pos in 0..=max_scroll {
            let (_, thumb_size) = scrollbar_geometry(18, 30, 4, pos);
            sizes.push(thumb_size);
        }
        let first = sizes[0];
        assert!(
            sizes.iter().all(|&s| s == first),
            "Thumb size must be constant, got: {:?}",
            sizes
        );
    }

    #[test]
    fn test_scrollbar_geometry_thumb_at_start_and_end() {
        // thumb_start == 0 at position 0, thumb reaches end at max scroll
        let (start, size) = scrollbar_geometry(18, 30, 4, 0);
        assert_eq!(start, 0);
        let max_scroll = 30 - 4;
        let (start_end, size_end) = scrollbar_geometry(18, 30, 4, max_scroll);
        assert_eq!(start_end + size_end, 18, "Thumb must reach the bottom");
        assert_eq!(size, size_end);
    }

    #[test]
    fn test_scrollbar_geometry_minimum_thumb_size() {
        // Very large content → thumb should be at least 1
        let (_, size) = scrollbar_geometry(10, 1000, 1, 0);
        assert_eq!(size, 1);
    }

    #[test]
    fn test_scrollbar_geometry_content_fits_viewport() {
        // total <= viewport → thumb fills entire track
        let (start, size) = scrollbar_geometry(18, 4, 4, 0);
        assert_eq!(start, 0);
        assert_eq!(size, 18);
    }

    #[test]
    fn test_scrollbar_geometry_zero_track() {
        let (start, size) = scrollbar_geometry(0, 30, 4, 0);
        assert_eq!(start, 0);
        assert_eq!(size, 0);
    }

    // --- highlight_line_with_current tests ---

    fn cur_style() -> Style {
        Style::default().fg(Color::Black).bg(Color::Indexed(208))
    }

    #[test]
    fn test_highlight_line_with_current_empty_query() {
        let line = Line::from("hello world");
        let result = highlight_line_with_current(line, "", hl_style(), cur_style(), 0);
        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn test_highlight_line_with_current_no_match() {
        let line = Line::from("hello world");
        let result = highlight_line_with_current(line, "xyz", hl_style(), cur_style(), 0);
        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn test_highlight_line_with_current_single_match_current() {
        let line = Line::from("hello world");
        let result = highlight_line_with_current(line, "world", hl_style(), cur_style(), 0);
        assert_eq!(result.spans.len(), 2);
        assert_eq!(result.spans[0].content.as_ref(), "hello ");
        assert_eq!(result.spans[1].content.as_ref(), "world");
        assert_eq!(result.spans[1].style, cur_style());
    }

    #[test]
    fn test_highlight_line_with_current_multiple_occurrences() {
        let line = Line::from("foo bar foo baz foo");
        let result = highlight_line_with_current(line, "foo", hl_style(), cur_style(), 1);
        // foo(0) bar foo(1=current) baz foo(2)
        assert_eq!(result.spans.len(), 5);
        assert_eq!(result.spans[0].style, hl_style()); // foo #0
        assert_eq!(result.spans[2].style, cur_style()); // foo #1 (current)
        assert_eq!(result.spans[4].style, hl_style()); // foo #2
    }

    #[test]
    fn test_highlight_line_with_current_case_insensitive() {
        let line = Line::from("Hello HELLO hello");
        let result = highlight_line_with_current(line, "hello", hl_style(), cur_style(), 2);
        assert_eq!(result.spans.len(), 5);
        assert_eq!(result.spans[0].style, hl_style()); // Hello
        assert_eq!(result.spans[2].style, hl_style()); // HELLO
        assert_eq!(result.spans[4].style, cur_style()); // hello (current)
    }

    // --- centered_rect tests ---

    #[test]
    fn test_centered_rect_standard() {
        let area = Rect::new(0, 0, 100, 50);
        let result = centered_rect(50, 3, area);
        assert_eq!(result.height, 3);
        // Horizontal centering: 25% left margin of 100 = 25
        assert!(result.x >= 20 && result.x <= 30, "x={}", result.x);
        assert!(
            result.width >= 45 && result.width <= 55,
            "w={}",
            result.width
        );
    }

    #[test]
    fn test_centered_rect_full_width() {
        let area = Rect::new(0, 0, 80, 24);
        let result = centered_rect(100, 5, area);
        assert_eq!(result.height, 5);
        // 100% width → x should be 0 and width should be full
        assert_eq!(result.x, 0);
        assert_eq!(result.width, 80);
    }

    #[test]
    fn test_centered_rect_small_area() {
        let area = Rect::new(0, 0, 20, 5);
        let result = centered_rect(50, 3, area);
        assert_eq!(result.height, 3);
        assert!(result.width > 0);
    }

    #[test]
    fn test_scrollbar_geometry_conversation_view_scenario() {
        // Simulate conversation: track=40, total_lines=200, visible=40
        let max_scroll = 200 - 40;
        let mut sizes = Vec::new();
        for pos in 0..=max_scroll {
            let (_, thumb_size) = scrollbar_geometry(40, 200, 40, pos);
            sizes.push(thumb_size);
        }
        let first = sizes[0];
        assert!(
            sizes.iter().all(|&s| s == first),
            "Thumb size must be constant, got varying sizes"
        );
        // thumb at max scroll reaches bottom
        let (start, size) = scrollbar_geometry(40, 200, 40, max_scroll);
        assert_eq!(start + size, 40);
    }

    // --- Selection Highlight Tests ---

    fn sel_style() -> Style {
        Style::default().fg(Color::White).bg(Color::Blue)
    }

    #[test]
    fn test_apply_selection_highlight_no_overlap() {
        use crate::app::{ContentPosition, TextSelection};
        let lines = vec![Line::from("hello world")];
        let sel = TextSelection {
            anchor: ContentPosition::new(5, 0),
            cursor: ContentPosition::new(5, 5),
            active: false,
        };
        // Selection is on line 5, visible lines start at scroll=0, only line 0 visible.
        let result = apply_selection_highlight(lines.clone(), &sel, 0, sel_style());
        // No overlap, line should be unchanged.
        assert_eq!(result[0].spans.len(), 1);
        assert_eq!(result[0].spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn test_apply_selection_highlight_full_line() {
        use crate::app::{ContentPosition, TextSelection};
        // Simulate a content line with border prefix: "│ " + "  " + "Hello"
        let lines = vec![Line::from(vec![
            Span::styled("│ ", Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::raw("Hello"),
        ])];
        let sel = TextSelection {
            anchor: ContentPosition::new(0, 0),
            cursor: ContentPosition::new(0, 100), // select entire line
            active: false,
        };
        let result = apply_selection_highlight(lines, &sel, 0, sel_style());
        // The content "Hello" portion should have selection style applied.
        let has_selection_style = result[0].spans.iter().any(|s| s.style == sel_style());
        assert!(
            has_selection_style,
            "Selection style should be applied to content"
        );
    }

    #[test]
    fn test_apply_selection_highlight_multi_line() {
        use crate::app::{ContentPosition, TextSelection};
        let lines = vec![
            Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::Green)),
                Span::raw("  "),
                Span::raw("Line one"),
            ]),
            Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::Green)),
                Span::raw("  "),
                Span::raw("Line two"),
            ]),
        ];
        let sel = TextSelection {
            anchor: ContentPosition::new(0, 0),
            cursor: ContentPosition::new(1, 8),
            active: false,
        };
        let result = apply_selection_highlight(lines, &sel, 0, sel_style());
        // Both lines should have some spans with selection style.
        for line in &result {
            let has_sel = line.spans.iter().any(|s| s.style == sel_style());
            assert!(has_sel, "Each selected line should have selection style");
        }
    }
}
