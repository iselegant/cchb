use crate::color::Theme;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

enum ListKind {
    Ordered(u64),
    Unordered,
}

struct TableAccumulator {
    header: Vec<Vec<Span<'static>>>,
    rows: Vec<Vec<Vec<Span<'static>>>>,
    current_row: Vec<Vec<Span<'static>>>,
    current_cell: Vec<Span<'static>>,
    in_header: bool,
}

impl TableAccumulator {
    fn new() -> Self {
        Self {
            header: Vec::new(),
            rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: Vec::new(),
            in_header: false,
        }
    }
}

struct MarkdownRenderer<'a> {
    theme: &'a Theme,
    base_style: Style,
    style_stack: Vec<Style>,
    current_spans: Vec<Span<'static>>,
    lines: Vec<Line<'static>>,
    in_code_block: bool,
    list_stack: Vec<ListKind>,
    table: Option<TableAccumulator>,
    link_url: Option<String>,
    available_width: usize,
}

impl<'a> MarkdownRenderer<'a> {
    fn new(base_style: Style, theme: &'a Theme, available_width: usize) -> Self {
        Self {
            theme,
            base_style,
            style_stack: vec![base_style],
            current_spans: Vec::new(),
            lines: Vec::new(),
            in_code_block: false,
            list_stack: Vec::new(),
            table: None,
            link_url: None,
            available_width,
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or(self.base_style)
    }

    fn push_modifier(&mut self, modifier: Modifier) {
        let new_style = self.current_style().add_modifier(modifier);
        self.style_stack.push(new_style);
    }

    fn push_style(&mut self, style: Style) {
        self.style_stack.push(style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            self.lines.push(Line::from(spans));
        }
    }

    fn list_indent(&self) -> String {
        let depth = self.list_stack.len().saturating_sub(1);
        "  ".repeat(depth)
    }

    fn handle_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.handle_start(tag),
            Event::End(tag_end) => self.handle_end(tag_end),
            Event::Text(text) => self.handle_text(&text),
            Event::Code(code) => self.handle_inline_code(&code),
            Event::SoftBreak | Event::HardBreak => self.flush_line(),
            Event::Rule => self.handle_rule(),
            _ => {}
        }
    }

    fn handle_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                let style = match level {
                    pulldown_cmark::HeadingLevel::H1 => self.theme.markdown.heading1,
                    pulldown_cmark::HeadingLevel::H2 => self.theme.markdown.heading2,
                    _ => self.theme.markdown.heading3,
                };
                self.push_style(style);
            }
            Tag::Strong => {
                self.push_modifier(Modifier::BOLD);
            }
            Tag::Emphasis => {
                self.push_modifier(Modifier::ITALIC);
            }
            Tag::Strikethrough => {
                self.push_modifier(Modifier::CROSSED_OUT);
            }
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.to_string());
                self.push_style(self.theme.markdown.link);
            }
            Tag::CodeBlock(kind) => {
                self.in_code_block = true;
                if let CodeBlockKind::Fenced(lang) = kind {
                    let lang_str = lang.to_string();
                    if !lang_str.is_empty() {
                        self.lines.push(Line::from(Span::styled(
                            format!(" {lang_str} "),
                            self.theme.markdown.code_lang_label,
                        )));
                    }
                }
            }
            Tag::Paragraph => {}
            Tag::List(start) => match start {
                Some(n) => self.list_stack.push(ListKind::Ordered(n)),
                None => self.list_stack.push(ListKind::Unordered),
            },
            Tag::Item => {
                let indent = self.list_indent();
                let bullet = match self.list_stack.last_mut() {
                    Some(ListKind::Unordered) => format!("{indent}• "),
                    Some(ListKind::Ordered(n)) => {
                        let s = format!("{indent}{n}. ");
                        *n += 1;
                        s
                    }
                    None => String::new(),
                };
                if !bullet.is_empty() {
                    self.current_spans
                        .push(Span::styled(bullet, self.theme.markdown.list_bullet));
                }
            }
            Tag::Table(_) => {
                self.table = Some(TableAccumulator::new());
            }
            Tag::TableHead => {
                if let Some(ref mut table) = self.table {
                    table.in_header = true;
                    table.current_row.clear();
                }
            }
            Tag::TableRow => {
                if let Some(ref mut table) = self.table {
                    table.current_row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(ref mut table) = self.table {
                    table.current_cell.clear();
                }
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Heading(_) => {
                self.flush_line();
                self.pop_style();
                self.lines.push(Line::from(""));
            }
            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::Link => {
                if let Some(url) = self.link_url.take() {
                    self.current_spans.push(Span::styled(
                        format!(" ({url})"),
                        Style::default().fg(ratatui::style::Color::DarkGray),
                    ));
                }
                self.pop_style();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.lines.push(Line::from(""));
            }
            TagEnd::Paragraph => {
                self.flush_line();
                self.lines.push(Line::from(""));
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.lines.push(Line::from(""));
                }
            }
            TagEnd::Item => {
                self.flush_line();
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.render_table(table);
                }
            }
            TagEnd::TableHead => {
                if let Some(ref mut table) = self.table {
                    table.header = std::mem::take(&mut table.current_row);
                    table.in_header = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(ref mut table) = self.table
                    && !table.in_header
                {
                    let row = std::mem::take(&mut table.current_row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                if let Some(ref mut table) = self.table {
                    let cell = std::mem::take(&mut table.current_cell);
                    table.current_row.push(cell);
                }
            }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str) {
        let style = self.current_style();
        if let Some(ref mut table) = self.table {
            table
                .current_cell
                .push(Span::styled(text.to_string(), style));
            return;
        }

        if self.in_code_block {
            let code_style = self.theme.markdown.code_block;
            for line in text.lines() {
                let content = format!("  {line}  ");
                let content_width = content.width();
                let padded = if content_width < self.available_width {
                    format!(
                        "{content}{}",
                        " ".repeat(self.available_width - content_width)
                    )
                } else {
                    content
                };
                self.lines
                    .push(Line::from(Span::styled(padded, code_style)));
            }
            return;
        }

        self.current_spans
            .push(Span::styled(text.to_string(), self.current_style()));
    }

    fn handle_inline_code(&mut self, code: &str) {
        if let Some(ref mut table) = self.table {
            table.current_cell.push(Span::styled(
                format!(" {code} "),
                self.theme.markdown.inline_code,
            ));
            return;
        }
        self.current_spans.push(Span::styled(
            format!(" {code} "),
            self.theme.markdown.inline_code,
        ));
    }

    fn handle_rule(&mut self) {
        self.lines.push(Line::from(Span::styled(
            "─".repeat(40),
            self.theme.markdown.horizontal_rule,
        )));
        self.lines.push(Line::from(""));
    }

    fn render_table(&mut self, table: TableAccumulator) {
        let num_cols = table.header.len();
        if num_cols == 0 {
            return;
        }

        // Calculate natural column widths
        let mut col_widths: Vec<usize> = table
            .header
            .iter()
            .map(|cell| cell_text_width(cell))
            .collect();

        for row in &table.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    col_widths[i] = col_widths[i].max(cell_text_width(cell));
                }
            }
        }

        // Shrink columns to fit available width
        // Total width = 1(left border) + sum(col_width + 3(padding + separator)) for each col
        // = 1 + num_cols * 3 + sum(col_widths)
        let overhead = 1 + num_cols * 3;
        let total_content_width: usize = col_widths.iter().sum();
        let total_width = overhead + total_content_width;

        if self.available_width > 0 && total_width > self.available_width {
            let available_content = self.available_width.saturating_sub(overhead);
            if available_content > 0 && total_content_width > 0 {
                shrink_columns(&mut col_widths, available_content);
            }
        }

        let border_style = self.theme.markdown.table_border;
        let header_style = self.theme.markdown.table_header;

        // Top border: ┌─────┬─────┐
        let top = build_table_border(&col_widths, '┌', '┬', '┐', border_style);
        self.lines.push(top);

        // Header row
        let header_line =
            build_table_row(&table.header, &col_widths, border_style, Some(header_style));
        self.lines.push(header_line);

        // Separator: ├─────┼─────┤
        let sep = build_table_border(&col_widths, '├', '┼', '┤', border_style);
        self.lines.push(sep);

        // Data rows
        for row in &table.rows {
            let row_line = build_table_row(row, &col_widths, border_style, None);
            self.lines.push(row_line);
        }

        // Bottom border: └─────┴─────┘
        let bottom = build_table_border(&col_widths, '└', '┴', '┘', border_style);
        self.lines.push(bottom);

        self.lines.push(Line::from(""));
    }

    fn render(mut self, text: &str) -> Vec<Line<'static>> {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);

        let parser = Parser::new_ext(text, options);
        for event in parser {
            self.handle_event(event);
        }
        self.flush_line();
        self.lines
    }
}

fn cell_text_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

fn shrink_columns(col_widths: &mut [usize], available: usize) {
    let total: usize = col_widths.iter().sum();
    if total == 0 {
        return;
    }

    // Proportionally shrink each column, with a minimum width of 3 (for "...")
    let min_width = 3usize;
    let mut new_widths: Vec<usize> = col_widths
        .iter()
        .map(|&w| {
            let scaled = (w as f64 / total as f64 * available as f64).floor() as usize;
            scaled.max(min_width)
        })
        .collect();

    // Adjust rounding errors: trim from the widest column
    let mut sum: usize = new_widths.iter().sum();
    while sum > available {
        if let Some(max_idx) = new_widths
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > min_width)
            .max_by_key(|(_, w)| **w)
            .map(|(i, _)| i)
        {
            new_widths[max_idx] -= 1;
            sum -= 1;
        } else {
            break;
        }
    }

    col_widths.copy_from_slice(&new_widths);
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let target = max_width - 3; // reserve space for "..."
    let mut current_width = 0;
    let mut result = String::new();
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }
    result.push_str("...");
    result
}

fn build_table_border(
    col_widths: &[usize],
    left: char,
    mid: char,
    right: char,
    style: Style,
) -> Line<'static> {
    let mut s = String::new();
    s.push(left);
    for (i, &w) in col_widths.iter().enumerate() {
        s.extend(std::iter::repeat_n('─', w + 2));
        if i < col_widths.len() - 1 {
            s.push(mid);
        }
    }
    s.push(right);
    Line::from(Span::styled(s, style))
}

fn build_table_row(
    cells: &[Vec<Span<'static>>],
    col_widths: &[usize],
    border_style: Style,
    cell_style_override: Option<Style>,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::styled("│", border_style));

    for (i, col_width) in col_widths.iter().enumerate() {
        spans.push(Span::raw(" "));

        let cell = cells.get(i);
        let cell_width: usize = cell.map(|c| cell_text_width(c)).unwrap_or(0);

        let actual_width = if let Some(cell_spans) = cell {
            if cell_width <= *col_width {
                // Cell fits — render as-is
                for span in cell_spans {
                    let style = cell_style_override.unwrap_or(span.style);
                    spans.push(Span::styled(span.content.to_string(), style));
                }
                cell_width
            } else {
                // Cell too wide — concatenate text and truncate once
                let full_text: String = cell_spans.iter().map(|s| s.content.as_ref()).collect();
                let style = cell_style_override
                    .or(cell_spans.first().map(|s| s.style))
                    .unwrap_or_default();
                let truncated = truncate_to_width(&full_text, *col_width);
                let w = truncated.width();
                spans.push(Span::styled(truncated, style));
                w
            }
        } else {
            0
        };

        let padding = col_width.saturating_sub(actual_width);
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding)));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled("│", border_style));
    }

    Line::from(spans)
}

pub fn render_markdown(
    text: &str,
    base_style: Style,
    theme: &Theme,
    available_width: usize,
) -> Vec<Line<'static>> {
    let renderer = MarkdownRenderer::new(base_style, theme, available_width);
    renderer.render(text)
}

/// Wrap a single `Line` into multiple lines so each fits within `max_width` display columns.
/// Prefers breaking at word boundaries (ASCII spaces).
pub fn wrap_line(line: Line<'static>, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![line];
    }

    let total_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
    if total_width <= max_width {
        return vec![line];
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width: usize = 0;

    for span in line.spans {
        let style = span.style;
        let full_text: String = span.content.into_owned();
        let span_total_w = full_text.width();
        let mut offset = 0;
        let mut consumed_w: usize = 0;

        while offset < full_text.len() {
            let remaining = &full_text[offset..];
            let remaining_w = span_total_w - consumed_w;
            let space_left = max_width.saturating_sub(current_width);

            if remaining_w <= space_left {
                current.push(Span::styled(remaining.to_string(), style));
                current_width += remaining_w;
                offset = full_text.len();
            } else if space_left == 0 {
                if !current.is_empty() {
                    result.push(Line::from(std::mem::take(&mut current)));
                    current_width = 0;
                }
            } else {
                let break_byte = find_break_point(remaining, space_left);
                if break_byte == 0 {
                    if !current.is_empty() {
                        result.push(Line::from(std::mem::take(&mut current)));
                        current_width = 0;
                    } else {
                        let forced = force_break_at(remaining, max_width);
                        let chunk_w = full_text[offset..offset + forced].width();
                        current.push(Span::styled(remaining[..forced].to_string(), style));
                        result.push(Line::from(std::mem::take(&mut current)));
                        current_width = 0;
                        offset += forced;
                        consumed_w += chunk_w;
                        let spaces = count_leading_spaces(&full_text[offset..]);
                        offset += spaces;
                        consumed_w += spaces; // ASCII spaces are 1 width each
                    }
                } else {
                    let chunk = remaining[..break_byte].trim_end();
                    current.push(Span::styled(chunk.to_string(), style));
                    result.push(Line::from(std::mem::take(&mut current)));
                    current_width = 0;
                    consumed_w += full_text[offset..offset + break_byte].width();
                    offset += break_byte;
                    let spaces = count_leading_spaces(&full_text[offset..]);
                    offset += spaces;
                    consumed_w += spaces;
                }
            }
        }
    }

    if !current.is_empty() {
        result.push(Line::from(current));
    }

    if result.is_empty() {
        result.push(Line::from(""));
    }

    result
}

/// Find byte position to break text at, preferring word boundaries.
fn find_break_point(text: &str, max_width: usize) -> usize {
    let mut width = 0;
    let mut last_space_end = 0;
    let mut found_space = false;

    for (i, ch) in text.char_indices() {
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_w > max_width {
            if ch == ' ' {
                return i;
            }
            return if found_space { last_space_end } else { i };
        }
        width += ch_w;
        if ch == ' ' {
            last_space_end = i + 1;
            found_space = true;
        }
    }
    text.len()
}

/// Force break at max_width display columns (for words longer than max_width).
fn force_break_at(text: &str, max_width: usize) -> usize {
    let mut width = 0;
    for (i, ch) in text.char_indices() {
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_w > max_width {
            return i;
        }
        width += ch_w;
    }
    text.len()
}

fn count_leading_spaces(text: &str) -> usize {
    text.bytes().take_while(|&b| b == b' ').count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Theme;
    use ratatui::style::{Color, Modifier, Style};

    fn theme() -> Theme {
        Theme::default_theme()
    }

    fn base_style() -> Style {
        Style::default().fg(Color::White)
    }

    fn render_md(text: &str, base_style: Style, theme: &Theme) -> Vec<Line<'static>> {
        render_markdown(text, base_style, theme, 80)
    }

    fn spans_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn find_span_with_text<'a>(lines: &'a [Line<'_>], needle: &str) -> Option<&'a Span<'a>> {
        for line in lines {
            for span in &line.spans {
                if span.content.contains(needle) {
                    return Some(span);
                }
            }
        }
        None
    }

    #[test]
    fn test_empty_input() {
        let t = theme();
        let result = render_md("", base_style(), &t);
        assert!(result.is_empty() || result.iter().all(|l| l.spans.is_empty()));
    }

    #[test]
    fn test_plain_text() {
        let t = theme();
        let result = render_md("Hello world", base_style(), &t);
        let text = spans_text(&result);
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn test_bold() {
        let t = theme();
        let result = render_md("**bold text**", base_style(), &t);
        let span = find_span_with_text(&result, "bold text").expect("bold span not found");
        assert!(
            span.style.add_modifier.contains(Modifier::BOLD),
            "Expected BOLD modifier"
        );
    }

    #[test]
    fn test_italic() {
        let t = theme();
        let result = render_md("*italic text*", base_style(), &t);
        let span = find_span_with_text(&result, "italic text").expect("italic span not found");
        assert!(
            span.style.add_modifier.contains(Modifier::ITALIC),
            "Expected ITALIC modifier"
        );
    }

    #[test]
    fn test_bold_italic_nested() {
        let t = theme();
        let result = render_md("***both***", base_style(), &t);
        let span = find_span_with_text(&result, "both").expect("bold+italic span not found");
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_inline_code() {
        let t = theme();
        let result = render_md("use `code` here", base_style(), &t);
        let span = find_span_with_text(&result, "code").expect("inline code span not found");
        assert_eq!(span.style.bg, Some(Color::Indexed(236)));
        assert_eq!(span.style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_heading1() {
        let t = theme();
        let result = render_md("# Title", base_style(), &t);
        let span = find_span_with_text(&result, "Title").expect("heading span not found");
        assert_eq!(span.style.fg, Some(Color::Cyan));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_heading2() {
        let t = theme();
        let result = render_md("## Subtitle", base_style(), &t);
        let span = find_span_with_text(&result, "Subtitle").expect("heading2 span not found");
        assert_eq!(span.style.fg, Some(Color::Blue));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_heading3() {
        let t = theme();
        let result = render_md("### Section", base_style(), &t);
        let span = find_span_with_text(&result, "Section").expect("heading3 span not found");
        assert_eq!(span.style.fg, Some(Color::Green));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_code_block() {
        let t = theme();
        let result = render_md("```\nlet x = 1;\n```", base_style(), &t);
        let span = find_span_with_text(&result, "let x = 1;").expect("code block span not found");
        assert_eq!(span.style.bg, Some(Color::Indexed(236)));
    }

    #[test]
    fn test_code_block_with_language() {
        let t = theme();
        let result = render_md("```rust\nfn main() {}\n```", base_style(), &t);
        let lang_span = find_span_with_text(&result, "rust").expect("language label not found");
        assert!(lang_span.style.add_modifier.contains(Modifier::ITALIC));
        let code_span =
            find_span_with_text(&result, "fn main()").expect("code block span not found");
        assert_eq!(code_span.style.bg, Some(Color::Indexed(236)));
    }

    #[test]
    fn test_unordered_list() {
        let t = theme();
        let result = render_md("- item one\n- item two", base_style(), &t);
        let text = spans_text(&result);
        assert!(text.contains("•"), "Expected bullet character");
        assert!(text.contains("item one"));
        assert!(text.contains("item two"));
    }

    #[test]
    fn test_ordered_list() {
        let t = theme();
        let result = render_md("1. first\n2. second", base_style(), &t);
        let text = spans_text(&result);
        assert!(text.contains("1."));
        assert!(text.contains("2."));
        assert!(text.contains("first"));
        assert!(text.contains("second"));
    }

    #[test]
    fn test_link() {
        let t = theme();
        let result = render_md("[click](https://example.com)", base_style(), &t);
        let link_span = find_span_with_text(&result, "click").expect("link text not found");
        assert!(link_span.style.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(link_span.style.fg, Some(Color::Cyan));
        let url_text = spans_text(&result);
        assert!(url_text.contains("https://example.com"));
    }

    #[test]
    fn test_horizontal_rule() {
        let t = theme();
        let result = render_md("---", base_style(), &t);
        let text = spans_text(&result);
        assert!(text.contains("─"), "Expected horizontal rule character");
    }

    #[test]
    fn test_table() {
        let t = theme();
        let result = render_md(
            "| Name | Age |\n|------|-----|\n| Alice | 30 |\n| Bob | 25 |",
            base_style(),
            &t,
        );
        let text = spans_text(&result);
        assert!(text.contains("│"), "Expected table border character");
        assert!(text.contains("Alice"));
        assert!(text.contains("Bob"));
        assert!(text.contains("Name"));
        assert!(text.contains("Age"));
    }

    #[test]
    fn test_mixed_formatting() {
        let t = theme();
        let result = render_md(
            "This has **bold** and *italic* and `code`",
            base_style(),
            &t,
        );
        let bold_span = find_span_with_text(&result, "bold").expect("bold span");
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
        let italic_span = find_span_with_text(&result, "italic").expect("italic span");
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));
        let code_span = find_span_with_text(&result, "code").expect("code span");
        assert_eq!(code_span.style.bg, Some(Color::Indexed(236)));
    }

    #[test]
    fn test_multiline_paragraph() {
        let t = theme();
        let result = render_md("line one\nline two", base_style(), &t);
        let text = spans_text(&result);
        assert!(text.contains("line one"));
        assert!(text.contains("line two"));
    }

    #[test]
    fn test_table_cjk_alignment() {
        let t = theme();
        let result = render_md(
            "| Name | Value |\n|------|-------|\n| hello | world |\n| \u{3053}\u{3093}\u{306b}\u{3061}\u{306f} | \u{4e16}\u{754c} |",
            base_style(),
            &t,
        );
        let text = spans_text(&result);
        // Verify table structure is present
        assert!(text.contains("\u{2502}"), "Expected table border");
        assert!(text.contains("hello"));
        assert!(text.contains("\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}"));

        // Verify border and data rows have consistent widths
        let line_widths: Vec<usize> = result
            .iter()
            .filter(|l| {
                let s = l
                    .spans
                    .iter()
                    .map(|sp| sp.content.as_ref())
                    .collect::<String>();
                s.contains('\u{2502}') || s.contains('\u{2500}')
            })
            .map(|l| l.spans.iter().map(|sp| sp.content.width()).sum::<usize>())
            .collect();

        // All table lines (borders + data rows) should have the same display width
        if let Some(&first) = line_widths.first() {
            for (i, &w) in line_widths.iter().enumerate() {
                assert_eq!(w, first, "Table line {i} has width {w}, expected {first}");
            }
        }
    }

    #[test]
    fn test_table_shrinks_to_fit_width() {
        let t = theme();
        let md = "| Column One | Column Two | Column Three |\n|---|---|---|\n| long value here | another long value | yet another value |";
        // Render with a narrow width of 30
        let result = render_markdown(md, base_style(), &t, 30);
        let text = spans_text(&result);
        assert!(text.contains("│"), "Expected table border");

        // All table lines should fit within 30 characters
        let line_widths: Vec<usize> = result
            .iter()
            .filter(|l| {
                let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
                s.contains('│') || s.contains('─')
            })
            .map(|l| l.spans.iter().map(|sp| sp.content.width()).sum::<usize>())
            .collect();

        for (i, &w) in line_widths.iter().enumerate() {
            assert!(w <= 30, "Table line {i} has width {w}, expected <= 30");
        }

        // All table lines should have consistent width
        if let Some(&first) = line_widths.first() {
            for (i, &w) in line_widths.iter().enumerate() {
                assert_eq!(w, first, "Table line {i} has width {w}, expected {first}");
            }
        }
    }

    #[test]
    fn test_table_no_shrink_when_fits() {
        let t = theme();
        let md = "| A | B |\n|---|---|\n| x | y |";
        let result_wide = render_markdown(md, base_style(), &t, 200);
        let result_default = render_md(md, base_style(), &t);
        // Should produce identical output when width is sufficient
        assert_eq!(spans_text(&result_wide), spans_text(&result_default));
    }

    #[test]
    fn test_truncate_to_width() {
        assert_eq!(truncate_to_width("hello world", 8), "hello...");
        assert_eq!(truncate_to_width("short", 10), "short");
        assert_eq!(truncate_to_width("abc", 3), "abc");
        assert_eq!(truncate_to_width("abcdef", 3), "...");
        assert_eq!(truncate_to_width("テスト長い文字列", 10), "テスト...");
    }

    #[test]
    fn test_code_block_lines_padded_to_full_width() {
        let t = theme();
        let result = render_markdown("```\nlet x = 1;\n```", base_style(), &t, 40);
        // Find the code block line
        let code_line = result
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("let x = 1;")))
            .expect("code block line not found");
        // The line should have total display width of 40 (full available width)
        let total_width: usize = code_line.spans.iter().map(|s| s.content.width()).sum();
        assert_eq!(
            total_width, 40,
            "Code block line should be padded to full width, got {total_width}"
        );
    }

    #[test]
    fn test_code_block_uses_indexed_colors() {
        let t = theme();
        let result = render_markdown("```\ncode\n```", base_style(), &t, 80);
        let code_span = find_span_with_text(&result, "code").expect("code span not found");
        // Should use Indexed(236) background for better contrast
        assert_eq!(code_span.style.bg, Some(Color::Indexed(236)));
        // Should use Indexed(252) foreground for better readability
        assert_eq!(code_span.style.fg, Some(Color::Indexed(252)));
    }

    #[test]
    fn test_shrink_columns() {
        let mut widths = vec![20, 30, 10];
        shrink_columns(&mut widths, 30);
        let total: usize = widths.iter().sum();
        assert!(total <= 30, "Total {total} exceeds 30");
        for &w in &widths {
            assert!(w >= 3, "Column width {w} is below minimum 3");
        }
    }

    #[test]
    fn test_wrap_line_short_line_unchanged() {
        let line = Line::from("short");
        let result = wrap_line(line, 20);
        assert_eq!(result.len(), 1);
        assert_eq!(spans_text(&result), "short");
    }

    #[test]
    fn test_wrap_line_breaks_at_word_boundary() {
        let line = Line::from("hello world foo");
        // Width 11 fits "hello world" exactly, but "hello world foo" = 15
        let result = wrap_line(line, 11);
        assert_eq!(result.len(), 2);
        let first: String = result[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let second: String = result[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(first, "hello world");
        assert_eq!(second, "foo");
    }

    #[test]
    fn test_wrap_line_preserves_styles() {
        let style_a = Style::default().fg(Color::Red);
        let style_b = Style::default().fg(Color::Blue);
        let line = Line::from(vec![
            Span::styled("red ", style_a),
            Span::styled("blue text here", style_b),
        ]);
        // "red blue text here" = 18 chars. Width 10 should wrap.
        let result = wrap_line(line, 10);
        assert!(
            result.len() >= 2,
            "Expected at least 2 lines, got {}",
            result.len()
        );
        // First line should contain red-styled span
        assert!(
            result[0]
                .spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Red))
        );
    }

    #[test]
    fn test_wrap_line_cjk_characters() {
        // Each CJK char is 2 columns wide
        // "あいうえお" = 10 columns
        let line = Line::from("あいうえお かきくけこ");
        // Width 12 should fit "あいうえお " (11 cols) then wrap
        let result = wrap_line(line, 12);
        assert!(result.len() >= 2, "Expected wrapping for CJK text");
        for r in &result {
            let w: usize = r.spans.iter().map(|s| s.content.width()).sum();
            assert!(w <= 12, "Line width {w} exceeds max 12");
        }
    }

    #[test]
    fn test_wrap_line_zero_width_returns_as_is() {
        let line = Line::from("test");
        let result = wrap_line(line, 0);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_wrap_line_long_word_force_break() {
        let line = Line::from("abcdefghijklmnop");
        let result = wrap_line(line, 5);
        assert!(result.len() >= 3);
        for r in &result {
            let w: usize = r.spans.iter().map(|s| s.content.width()).sum();
            assert!(w <= 5, "Line width {w} exceeds max 5");
        }
    }
}
