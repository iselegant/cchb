use ratatui::style::{Color, Modifier, Style};

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
    pub assistant_label: Style,
    pub assistant_message: Style,
    pub search_input: Style,
    pub help_title: Style,
    pub help_key: Style,
    pub help_desc: Style,
    pub border_active: Style,
    pub border_inactive: Style,
    pub status_bar: Style,
    pub title: Style,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            session_selected: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            session_normal: Style::default(),
            session_project: Style::default().fg(Color::Cyan),
            session_date: Style::default().fg(Color::DarkGray),
            session_branch: Style::default().fg(Color::Green),
            session_preview: Style::default().fg(Color::Gray),
            user_label: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            user_message: Style::default().fg(Color::White),
            assistant_label: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            assistant_message: Style::default().fg(Color::White),
            search_input: Style::default().fg(Color::Yellow),
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
        }
    }
}
