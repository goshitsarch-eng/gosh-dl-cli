//! The download list: tabs, then one entry per download or mirror job.

use ratatui::{prelude::*, widgets::Paragraph};

use super::{
    fit, gradient_bar, kind_color, pad, pad_left, panel, panel_title, spinner_symbol, state_color,
    state_icon, text_width,
};
use crate::tui::app::{SearchState, TuiApp};
use crate::tui::model::{Row, ViewMode};

/// Width of the right-hand summary on a wide row's second line.
const META_WIDTH: usize = 32;
/// Columns before the name on a wide row: selector, icon, tag.
const INDENT: usize = 14;

/// How the list is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyle {
    /// Bordered panel, tabs inside, two lines per row
    Wide,
    /// No border, one line per row (tabs are drawn separately)
    Compact,
}

impl ListStyle {
    fn lines_per_row(self) -> u16 {
        match self {
            Self::Wide => 2,
            Self::Compact => 1,
        }
    }
}

/// The area rows are drawn in, inside borders, tabs and the filter line.
fn rows_area(area: Rect, style: ListStyle, searching: bool) -> Rect {
    let mut inner = area;
    if style == ListStyle::Wide {
        // Border all round, then the tab line and its separator.
        inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        inner.y += 2.min(inner.height);
        inner.height = inner.height.saturating_sub(2);
    }
    if searching {
        inner.height = inner.height.saturating_sub(1);
    }
    inner
}

/// How many rows fit; the app uses this for scrolling and paging.
pub fn visible_rows(area: Rect, style: ListStyle, searching: bool) -> usize {
    (rows_area(area, style, searching).height / style.lines_per_row()) as usize
}

/// `1 All 4 │ 2 Active 2 │ …` with the current view highlighted.
pub fn tabs_line(app: &TuiApp, short: bool) -> Line<'static> {
    let theme = app.theme();
    let mut spans = Vec::new();
    for (index, mode) in ViewMode::ALL.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(
                " \u{2502} ",
                Style::default().fg(theme.surface2),
            ));
        }
        let active = mode == app.mode;
        let label = if short {
            mode.short_label()
        } else {
            mode.label()
        };
        spans.push(Span::styled(
            format!("{}", index + 1),
            Style::default().fg(if active { theme.accent } else { theme.overlay0 }),
        ));
        spans.push(Span::styled(
            format!(" {label}"),
            if active {
                Style::default()
                    .fg(theme.text)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(theme.subtext0)
            },
        ));
        spans.push(Span::styled(
            format!(" {}", app.counts[index]),
            Style::default().fg(if active { theme.accent } else { theme.overlay0 }),
        ));
    }
    Line::from(spans)
}

pub fn render(frame: &mut Frame, area: Rect, app: &TuiApp, style: ListStyle) {
    let theme = app.theme();
    let searching = app.search.is_some();

    if style == ListStyle::Wide {
        let mut block = panel(
            theme,
            panel_title(theme, "Downloads", app.counts[0].to_string()),
            true,
        );
        if app.rows.len() > visible_rows(area, style, searching) {
            block = block.title_bottom(
                Line::from(Span::styled(
                    format!(" {}/{} ", app.selected + 1, app.rows.len()),
                    Style::default().fg(theme.overlay1),
                ))
                .right_aligned(),
            );
        }
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.height >= 2 {
            let mut tabs = tabs_line(app, false);
            tabs.spans.insert(0, Span::raw(" "));
            frame.render_widget(
                Paragraph::new(tabs),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
            frame.render_widget(
                Paragraph::new("\u{2500}".repeat(inner.width as usize))
                    .style(Style::default().fg(theme.surface0)),
                Rect::new(inner.x, inner.y + 1, inner.width, 1),
            );
        }
    }

    let rows = rows_area(area, style, searching);
    if let Some(search) = &app.search {
        let line_area = Rect::new(rows.x, rows.y + rows.height, rows.width, 1);
        if line_area.y < area.y + area.height {
            render_search_line(frame, line_area, app, search);
        }
    }
    if rows.height == 0 || rows.width < 8 {
        return;
    }

    if app.rows.is_empty() {
        render_empty(frame, rows, app);
        return;
    }

    let per_row = style.lines_per_row();
    let visible = (rows.height / per_row) as usize;
    let spinner = spinner_symbol(&app.throbber_state);
    for (slot, (index, row)) in app
        .rows
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible)
        .enumerate()
    {
        let row_area = Rect::new(rows.x, rows.y + slot as u16 * per_row, rows.width, per_row);
        let selected = index == app.selected;
        if selected {
            frame.render_widget(
                Paragraph::new("").style(Style::default().bg(theme.surface0)),
                row_area,
            );
        }
        match style {
            ListStyle::Wide => render_wide_row(frame, row_area, app, row, selected, spinner),
            ListStyle::Compact => render_compact_row(frame, row_area, app, row, selected, spinner),
        }
    }
}

fn selector<'a>(app: &TuiApp, selected: bool) -> Span<'a> {
    if selected {
        Span::styled(
            "\u{25b6} ",
            Style::default()
                .fg(app.theme().accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("  ")
    }
}

fn name_style(app: &TuiApp, selected: bool) -> Style {
    let theme = app.theme();
    if selected {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtext1)
    }
}

fn render_wide_row(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    row: &Row,
    selected: bool,
    spinner: &'static str,
) {
    let theme = app.theme();
    let state = row.state();
    let color = state_color(theme, state);
    let width = area.width as usize;

    // Line 1: selector, icon, tag, name ... state
    let label = state.label();
    let name_width = width.saturating_sub(INDENT + text_width(label) + 3);
    let line = Line::from(vec![
        selector(app, selected),
        Span::styled(
            format!("{} ", state_icon(state, spinner)),
            Style::default().fg(color),
        ),
        Span::styled(
            pad(&row.tag(), 10),
            Style::default()
                .fg(kind_color(theme, row.kind()))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(pad(&row.name(), name_width), name_style(app, selected)),
        Span::raw(" "),
        Span::styled(
            pad_left(label, text_width(label) + 1),
            Style::default().fg(color),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line),
        Rect::new(area.x, area.y, area.width, 1),
    );

    // Line 2: progress bar and summary
    if area.height < 2 {
        return;
    }
    let meta_width = META_WIDTH.min(width.saturating_sub(INDENT + 6));
    let bar_width = width.saturating_sub(INDENT + meta_width + 2);
    let y = area.y + 1;
    if bar_width >= 4 {
        gradient_bar(
            frame.buffer_mut(),
            Rect::new(area.x + INDENT as u16, y, bar_width as u16, 1),
            row.percent() / 100.0,
            theme,
        );
    }
    let mut meta = row.meta();
    if text_width(&meta) > meta_width {
        meta = row.meta_short();
    }
    frame.render_widget(
        Paragraph::new(pad_left(&meta, meta_width)).style(Style::default().fg(theme.subtext0)),
        Rect::new(
            area.x + (width - meta_width - 1) as u16,
            y,
            meta_width as u16,
            1,
        ),
    );
}

fn render_compact_row(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    row: &Row,
    selected: bool,
    spinner: &'static str,
) {
    let theme = app.theme();
    let state = row.state();
    let width = area.width as usize;
    let meta_width = 17.min(width / 3);
    let bar_width = if width >= 56 { 12 } else { 0 };
    let name_width = width.saturating_sub(4 + bar_width + meta_width + 3);

    let line = Line::from(vec![
        selector(app, selected),
        Span::styled(
            format!("{} ", state_icon(state, spinner)),
            Style::default().fg(state_color(theme, state)),
        ),
        Span::styled(pad(&row.name(), name_width), name_style(app, selected)),
    ]);
    frame.render_widget(Paragraph::new(line), area);

    if bar_width > 0 {
        gradient_bar(
            frame.buffer_mut(),
            Rect::new(
                area.x + (4 + name_width + 1) as u16,
                area.y,
                bar_width as u16,
                1,
            ),
            row.percent() / 100.0,
            theme,
        );
    }
    frame.render_widget(
        Paragraph::new(pad_left(&row.meta_short(), meta_width))
            .style(Style::default().fg(theme.subtext0)),
        Rect::new(
            area.x + (width - meta_width - 1) as u16,
            area.y,
            meta_width as u16,
            1,
        ),
    );
}

fn render_empty(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let theme = app.theme();
    let muted = Style::default().fg(theme.overlay1);
    let key = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let line = match app.search.as_ref().filter(|s| !s.query.is_empty()) {
        Some(search) => Line::from(Span::styled(
            fit(
                &format!(
                    "Nothing matches \u{201c}{}\u{201d}. Esc clears the filter.",
                    search.query
                ),
                area.width as usize,
            ),
            muted,
        )),
        None if area.width < 66 => Line::from(vec![
            Span::styled("Nothing here. Press ", muted),
            Span::styled("a", key),
            Span::styled(" to add.", muted),
        ]),
        None => Line::from(vec![
            Span::styled("Nothing in this view. Press ", muted),
            Span::styled("a", key),
            Span::styled(" to add a URL, magnet link or torrent.", muted),
        ]),
    };
    let y = area.y + area.height.saturating_sub(1) / 2;
    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center),
        Rect::new(area.x, y, area.width, 1),
    );
}

fn render_search_line(frame: &mut Frame, area: Rect, app: &TuiApp, search: &SearchState) {
    let theme = app.theme();
    let muted = Style::default().fg(theme.overlay1);
    let mut spans = vec![
        Span::styled(
            " / ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(search.query.clone(), Style::default().fg(theme.text)),
    ];
    let hint = if search.input_active {
        format!(
            "  in {} \u{b7} Tab scope \u{b7} Enter keep \u{b7} Esc clear",
            search.scope.label()
        )
    } else {
        format!(
            "  in {} \u{b7} {} shown \u{b7} / edit \u{b7} Esc clear",
            search.scope.label(),
            app.rows.len()
        )
    };
    spans.push(Span::styled(hint, muted));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.bg_dim)),
        area,
    );
    if search.input_active {
        let prefix: String = search.query.chars().take(search.cursor).collect();
        let x = area.x + 3 + text_width(&prefix) as u16;
        if x < area.x + area.width {
            frame.set_cursor_position((x, area.y));
        }
    }
}
