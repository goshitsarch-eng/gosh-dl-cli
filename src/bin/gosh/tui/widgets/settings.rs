//! Settings dialog: four key/value tabs and the schedule tab.

use chrono::{Datelike, Local, Timelike, Weekday};
use ratatui::{prelude::*, widgets::Paragraph};

use super::dialogs::{line_at, open_popup};
use super::{centered, fit, key_line, pad, pad_left, text_width};
use crate::config::CliConfig;
use crate::tui::app::{DialogState, TuiApp};
use crate::tui::model::{active_schedule_rule_at, tilde_path};
use crate::tui::settings::{self, Editor, SCHEDULE_TAB, TAB_NAMES};

const DAYS: [(Weekday, &str); 7] = [
    (Weekday::Mon, "Mon"),
    (Weekday::Tue, "Tue"),
    (Weekday::Wed, "Wed"),
    (Weekday::Thu, "Thu"),
    (Weekday::Fri, "Fri"),
    (Weekday::Sat, "Sat"),
    (Weekday::Sun, "Sun"),
];

pub fn render(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let DialogState::Settings {
        active_tab,
        selected_row,
        editing,
        draft,
        dirty,
    } = dialog
    else {
        return;
    };
    let theme = app.theme();
    let area = centered(frame.area(), 112, 26);
    let inner = open_popup(frame, area, app, "Settings");
    if inner.width < 24 || inner.height < 4 {
        return;
    }

    // Tabs
    let mut tabs = Vec::new();
    for (index, name) in TAB_NAMES.iter().enumerate() {
        if index > 0 {
            tabs.push(Span::styled(
                " \u{2502} ",
                Style::default().fg(theme.surface2),
            ));
        }
        let active = index == *active_tab;
        tabs.push(Span::styled(
            format!("{} ", index + 1),
            Style::default().fg(if active { theme.accent } else { theme.overlay0 }),
        ));
        tabs.push(Span::styled(
            // Narrow screens get three-letter tab names.
            if inner.width < 60 {
                name.chars().take(3).collect::<String>()
            } else {
                (*name).to_string()
            },
            if active {
                Style::default()
                    .fg(theme.text)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(theme.subtext0)
            },
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(tabs)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    // Body between the tab line and the two footer lines.
    let body = Rect::new(
        inner.x,
        inner.y + 2.min(inner.height),
        inner.width,
        inner.height.saturating_sub(4),
    );
    if *active_tab == SCHEDULE_TAB {
        render_schedule(frame, body, app, draft, *selected_row);
    } else {
        render_rows(frame, body, app, draft, *active_tab, *selected_row, editing);
    }

    // Footer: where it is saved, then the keys.
    let bottom = inner.height.saturating_sub(1);
    if let Some(rect) = line_at(inner, bottom.saturating_sub(1)) {
        let state = if *dirty {
            Span::styled(
                "\u{25cf} unsaved changes",
                Style::default().fg(theme.warning),
            )
        } else {
            Span::styled("no changes", Style::default().fg(theme.overlay0))
        };
        let path = format!("  \u{b7}  {}", tilde_path(&app.config_path()));
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                state,
                Span::styled(
                    fit(&path, (inner.width as usize).saturating_sub(18)),
                    Style::default().fg(theme.overlay0),
                ),
            ])),
            rect,
        );
    }
    if let Some(rect) = line_at(inner, bottom) {
        let keys: &[(&str, &str)] = if editing.is_some() {
            &[("Enter", "apply"), ("Esc", "discard edit")]
        } else if *active_tab == SCHEDULE_TAB {
            &[
                ("j/k", "move"),
                ("\u{2190}/\u{2192}", "tabs"),
                ("Enter", "toggle rule"),
                ("Esc", "save & close"),
            ]
        } else {
            &[
                ("j/k", "move"),
                ("\u{2190}/\u{2192}", "tabs"),
                ("Enter", "change"),
                ("Esc", "save & close"),
            ]
        };
        frame.render_widget(
            Paragraph::new(key_line(theme, keys, inner.width as usize)),
            rect,
        );
    }
}

fn render_rows(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    draft: &CliConfig,
    tab: usize,
    selected: usize,
    editing: &Option<String>,
) {
    let theme = app.theme();
    let rows = settings::rows(tab);
    let visible = area.height as usize;
    if visible == 0 {
        return;
    }
    // Keep the selected row on screen.
    let offset = (selected + 1).saturating_sub(visible);
    let width = area.width as usize;
    let label_width = 28.min(width / 2);
    // The config key is shown on the right when there is room for it.
    let key_width = if width >= 90 { 36 } else { 0 };
    let value_width = width.saturating_sub(label_width + key_width + 6);

    for (slot, (index, setting)) in rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .enumerate()
    {
        let rect = Rect::new(area.x, area.y + slot as u16, area.width, 1);
        let is_selected = index == selected;
        let editing_this = is_selected.then_some(editing.as_ref()).flatten();
        let value = match editing_this {
            Some(buffer) => buffer.clone(),
            None => settings::display_value(draft, setting),
        };
        let value_style = match (editing_this.is_some(), setting.editor) {
            (true, _) => Style::default().fg(theme.text).bg(theme.surface1),
            (_, Editor::Bool) if value == "ON" => Style::default().fg(theme.success),
            (_, Editor::Bool) => Style::default().fg(theme.overlay1),
            (_, Editor::Choice(_)) => Style::default().fg(theme.peach),
            _ => Style::default().fg(theme.text),
        };
        // While editing, show the end of the text: that is where typing goes.
        let shown = if editing_this.is_some() {
            pad(
                &super::fit_left(&value, value_width.saturating_sub(1)),
                value_width,
            )
        } else if settings::is_path(setting) {
            pad(&super::fit_left(&value, value_width), value_width)
        } else {
            pad(&value, value_width)
        };
        let line = Line::from(vec![
            Span::styled(
                if is_selected { "\u{25b6} " } else { "  " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                pad(setting.label, label_width),
                if is_selected {
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.subtext1)
                },
            ),
            Span::styled(shown, value_style),
            Span::styled(
                if setting.restart { " \u{21bb} " } else { "   " },
                Style::default().fg(theme.warning),
            ),
            Span::styled(
                pad_left(setting.key, key_width),
                Style::default().fg(theme.overlay0),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(if is_selected {
                Style::default().bg(theme.surface0)
            } else {
                Style::default()
            }),
            rect,
        );
        if let Some(buffer) = editing_this {
            let typed = text_width(buffer).min(value_width.saturating_sub(1));
            frame.set_cursor_position((rect.x + (2 + label_width + typed) as u16, rect.y));
        }
    }

    // Legend under the rows, if a row here needs a restart.
    let used = rows.len().min(visible) as u16;
    if rows.iter().any(|s| s.restart) && used + 1 < area.height {
        frame.render_widget(
            Paragraph::new(Line::styled(
                "\u{21bb} takes effect the next time gosh starts",
                Style::default().fg(theme.overlay0),
            )),
            Rect::new(
                area.x + 2,
                area.y + used + 1,
                area.width.saturating_sub(2),
                1,
            ),
        );
    }
}

fn render_schedule(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    draft: &CliConfig,
    selected: usize,
) {
    let theme = app.theme();
    let muted = Style::default().fg(theme.overlay1);
    let rules = &draft.schedule.rules;
    let now = Local::now();
    let active = active_schedule_rule_at(draft, now.hour() as u8, now.weekday()).map(|(i, _)| i);
    let mut y = 0u16;
    let mut put = |frame: &mut Frame, line: Line<'_>| {
        if let Some(rect) = line_at(area, y) {
            frame.render_widget(Paragraph::new(line), rect);
        }
        y += 1;
    };

    if rules.is_empty() {
        put(
            frame,
            Line::styled("No schedule rules yet.", Style::default().fg(theme.text)),
        );
        put(frame, Line::raw(""));
        for text in [
            "Rules limit speed by time of day. Add them to the config file:",
            "",
            "  [[schedule.rules]]",
            "  start_hour = 9",
            "  end_hour = 17",
            "  days = \"weekdays\"",
            "  download_limit = \"2M\"",
        ] {
            put(frame, Line::styled(text, muted));
        }
        return;
    }

    // Rule table
    put(
        frame,
        Line::styled(
            format!(
                "  {:<3}{:<5}{:<13}{:<16}{:<11}{:<11}",
                "#", "on", "hours", "days", "\u{2193} limit", "\u{2191} limit"
            ),
            muted,
        ),
    );
    // Leave room for the week map (10 lines) when the dialog is tall enough.
    let table_room = (area.height as usize).saturating_sub(12).max(3);
    let offset = (selected + 1).saturating_sub(table_room);
    for (index, rule) in rules.iter().enumerate().skip(offset).take(table_room) {
        let is_selected = index == selected;
        let limit =
            |value: &Option<String>| value.clone().unwrap_or_else(|| "unlimited".to_string());
        let mut spans = vec![
            Span::styled(
                if is_selected { "\u{25b6} " } else { "  " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<3}", index + 1),
                Style::default().fg(rule_color(app, index)),
            ),
            Span::styled(
                format!("{:<5}", if rule.enabled { "[x]" } else { "[ ]" }),
                Style::default().fg(if rule.enabled {
                    theme.success
                } else {
                    theme.overlay0
                }),
            ),
            Span::styled(
                format!(
                    "{:<13}",
                    format!("{:02}:00\u{2013}{:02}:59", rule.start_hour, rule.end_hour)
                ),
                Style::default().fg(theme.text),
            ),
            Span::styled(pad(&rule.days, 16), Style::default().fg(theme.text)),
            Span::styled(
                pad(&limit(&rule.download_limit), 11),
                Style::default().fg(theme.teal),
            ),
            Span::styled(
                pad(&limit(&rule.upload_limit), 11),
                Style::default().fg(theme.peach),
            ),
        ];
        if active == Some(index) {
            spans.push(Span::styled(
                "active now",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        let line = Line::from(spans);
        if let Some(rect) = line_at(area, y) {
            frame.render_widget(
                Paragraph::new(line).style(if is_selected {
                    Style::default().bg(theme.surface0)
                } else {
                    Style::default()
                }),
                rect,
            );
        }
        y += 1;
    }

    // Week map: which rule is in force for every hour of every day.
    if area.height < y + 10 || area.width < 34 {
        return;
    }
    let mut put = |frame: &mut Frame, line: Line<'_>| {
        if let Some(rect) = line_at(area, y) {
            frame.render_widget(Paragraph::new(line), rect);
        }
        y += 1;
    };
    put(frame, Line::raw(""));
    put(
        frame,
        Line::styled(
            "This week",
            Style::default()
                .fg(theme.subtext0)
                .add_modifier(Modifier::BOLD),
        ),
    );
    let mut axis = String::from("     ");
    for hour in (0..24).step_by(3) {
        axis.push_str(&format!("{hour:<3}"));
    }
    put(frame, Line::styled(axis, muted));
    for (weekday, name) in DAYS {
        let today = weekday == now.weekday();
        let mut spans = vec![Span::styled(
            format!("{name}  "),
            if today {
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
            } else {
                muted
            },
        )];
        for hour in 0..24u8 {
            let rule = active_schedule_rule_at(draft, hour, weekday).map(|(i, _)| i);
            let is_now = today && u32::from(hour) == now.hour();
            let mut style = match rule {
                Some(index) => Style::default().fg(rule_color(app, index)),
                None => Style::default().fg(theme.surface1),
            };
            if is_now {
                style = style.add_modifier(Modifier::REVERSED);
            }
            spans.push(Span::styled(
                if rule.is_some() { "\u{2588}" } else { "\u{b7}" },
                style,
            ));
        }
        put(frame, Line::from(spans));
    }
}

/// A stable colour per rule, shared by the table and the week map.
fn rule_color(app: &TuiApp, index: usize) -> Color {
    let theme = app.theme();
    [
        theme.accent,
        theme.peach,
        theme.mauve,
        theme.teal,
        theme.pink,
        theme.sky,
    ][index % 6]
}
