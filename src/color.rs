use ratatui::style::{Color, Modifier, Style};

#[allow(dead_code)]
pub struct MarkdownStyles {
    pub heading1: Style,
    pub heading2: Style,
    pub heading3: Style,
    pub bold: Style,
    pub italic: Style,
    pub inline_code: Style,
    pub code_block: Style,
    pub code_lang_label: Style,
    pub link: Style,
    pub list_bullet: Style,
    pub table_border: Style,
    pub table_header: Style,
    pub horizontal_rule: Style,
}

#[allow(dead_code)]
pub struct Theme {
    pub session_selected: Style,
    pub session_normal: Style,
    pub session_project: Style,
    pub session_date: Style,
    pub session_branch: Style,
    pub session_preview: Style,
    pub user_label: Style,
    pub user_message: Style,
    pub user_border: Style,
    pub assistant_label: Style,
    pub assistant_message: Style,
    pub assistant_border: Style,
    pub search_input: Style,
    pub search_highlight: Style,
    pub help_title: Style,
    pub help_key: Style,
    pub help_desc: Style,
    pub border_active: Style,
    pub border_inactive: Style,
    pub status_bar: Style,
    pub title: Style,
    pub markdown: MarkdownStyles,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            session_selected: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            session_normal: Style::default(),
            session_project: Style::default().fg(Color::Cyan),
            session_date: Style::default().fg(Color::Yellow),
            session_branch: Style::default().fg(Color::Green),
            session_preview: Style::default().fg(Color::Gray),
            user_label: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            user_message: Style::default().fg(Color::White),
            user_border: Style::default().fg(Color::Green),
            assistant_label: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            assistant_message: Style::default().fg(Color::White),
            assistant_border: Style::default().fg(Color::Magenta),
            search_input: Style::default().fg(Color::Yellow),
            search_highlight: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            help_title: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            help_key: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            help_desc: Style::default().fg(Color::Gray),
            border_active: Style::default().fg(Color::Cyan),
            border_inactive: Style::default().fg(Color::DarkGray),
            status_bar: Style::default().fg(Color::White).bg(Color::DarkGray),
            title: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            markdown: MarkdownStyles {
                heading1: Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                heading2: Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                heading3: Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                bold: Style::default().add_modifier(Modifier::BOLD),
                italic: Style::default().add_modifier(Modifier::ITALIC),
                inline_code: Style::default().fg(Color::Yellow).bg(Color::Indexed(236)),
                code_block: Style::default()
                    .fg(Color::Indexed(252))
                    .bg(Color::Indexed(236)),
                code_lang_label: Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
                link: Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
                list_bullet: Style::default().fg(Color::Yellow),
                table_border: Style::default().fg(Color::DarkGray),
                table_header: Style::default().add_modifier(Modifier::BOLD),
                horizontal_rule: Style::default().fg(Color::DarkGray),
            },
        }
    }
}
