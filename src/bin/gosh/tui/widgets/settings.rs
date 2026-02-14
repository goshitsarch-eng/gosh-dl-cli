use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph},
};

use super::btop_border::btop_block;
use super::dialogs::centered_rect;
use crate::tui::app::{DialogState, TuiApp};

pub fn render_settings(frame: &mut Frame, dialog: &DialogState, app: &TuiApp) {
    let DialogState::Settings {
        active_tab,
        selected_row,
        scroll_offset: _,
        editing,
        draft,
        dirty: _,
    } = dialog
    else {
        return;
    };

    let theme = app.theme();
    let area = centered_rect(65, 80, frame.area());
    frame.render_widget(Clear, area);

    let title = "Settings";
    let block = btop_block(title, theme, true).style(Style::default().bg(theme.bg));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 5 || inner.width < 20 {
        return;
    }

    // Tab bar
    let tab_names = TuiApp::settings_tab_names();
    let mut tab_spans = Vec::new();
    for (i, name) in tab_names.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" | ", Style::default().fg(theme.surface2)));
        }
        let style = if i == *active_tab {
            Style::default()
                .fg(theme.bg_deep)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.subtext0)
        };
        tab_spans.push(Span::styled(format!(" {} ", name), style));
    }
    let tab_line = Line::from(tab_spans);
    let tab_area = Rect::new(inner.x + 1, inner.y, inner.width - 2, 1);
    frame.render_widget(Paragraph::new(tab_line), tab_area);

    // Separator
    let sep_y = inner.y + 1;
    let sep = Paragraph::new(Line::from(
        "\u{2500}".repeat(inner.width as usize - 2),
    ))
    .style(Style::default().fg(theme.surface1));
    frame.render_widget(sep, Rect::new(inner.x + 1, sep_y, inner.width - 2, 1));

    // Settings rows
    let row_count = TuiApp::settings_row_count(*active_tab);
    let content_y = sep_y + 1;
    let content_height = (inner.height as usize).saturating_sub(3);

    for row in 0..row_count {
        if row >= content_height {
            break;
        }
        let y = content_y + row as u16;
        let label = TuiApp::get_settings_label(*active_tab, row);
        let value = TuiApp::get_settings_value(draft, *active_tab, row);
        let is_selected = row == *selected_row;
        let is_bool = TuiApp::is_settings_bool(*active_tab, row);

        let row_style = if is_selected {
            Style::default().fg(theme.text).bg(theme.surface0)
        } else {
            Style::default().fg(theme.subtext0)
        };

        // Check if this row is being edited
        let display_value = if is_selected {
            if let Some(ref edit_buf) = editing {
                edit_buf.clone()
            } else {
                value
            }
        } else {
            value
        };

        let value_style = if is_bool {
            if display_value == "ON" {
                Style::default().fg(theme.success)
            } else {
                Style::default().fg(theme.error)
            }
        } else if is_selected && editing.is_some() {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.text)
        };

        let label_width = (inner.width / 2).min(30) as usize;
        let padded_label = format!("  {:<width$}", label, width = label_width);

        let editing_indicator = if is_selected && editing.is_some() {
            Span::styled(" [editing]", Style::default().fg(theme.warning))
        } else {
            Span::raw("")
        };

        let line = Line::from(vec![
            Span::styled(padded_label, row_style),
            Span::styled(format!(" {} ", display_value), value_style.bg(if is_selected { theme.surface0 } else { Color::Reset })),
            editing_indicator,
        ]);

        let row_area = Rect::new(inner.x, y, inner.width, 1);
        frame.render_widget(Paragraph::new(line), row_area);
    }

    // Footer hint
    let footer_y = inner.y + inner.height - 1;
    let hint = if editing.is_some() {
        "Type to edit | Enter: confirm | Esc: cancel"
    } else {
        "j/k: navigate | Enter/Space: edit | Left/Right: tabs | Esc: save & close"
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        format!("  {}", hint),
        Style::default().fg(theme.overlay0),
    )));
    frame.render_widget(footer, Rect::new(inner.x, footer_y, inner.width, 1));
}
