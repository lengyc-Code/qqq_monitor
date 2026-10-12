use super::super::{DesktopApp, Page};
use crate::{
    components::{bytes, label_width},
    i18n, pixel, theme,
};
use eframe::egui::{
    self, FontId, RichText, Sense, Stroke, pos2,
    text::{LayoutJob, TextFormat, TextWrapping},
    vec2,
};
use qqq_domain::{Priority, ProcessAction, ProcessHost, ProcessId, ProcessInfo};
use qqq_engine::{MonitorCommand, MonitorView, Notice, ProcessRow, ProcessTable};
use std::{cmp::Ordering, sync::Arc};

const ROW: f32 = 26.0;
const HEADER: f32 = 28.0;
/// Horizontal text padding inside a cell.
const PAD: f32 = 8.0;
/// Width of the priority selector in the toolbar.
const PRIORITY_WIDTH: f32 = 150.0;
/// Room for the sort arrow after a column title.
const ARROW: f32 = 18.0;
/// Longest command line a tooltip shows, in characters.
const TOOLTIP_CHARS: usize = 400;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Name,
    Pid,
    User,
    Cpu,
    Memory,
    Threads,
    Priority,
    Command,
}

impl Column {
    fn title(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::Name => t.process_name,
            Self::Pid => "PID",
            Self::User => t.process_user,
            Self::Cpu => "CPU",
            Self::Memory => t.process_memory,
            Self::Threads => t.process_threads,
            Self::Priority => t.process_priority,
            Self::Command => t.process_command,
        }
    }
    /// Numbers read best right-aligned and sorted largest first.
    fn numeric(self) -> bool {
        matches!(
            self,
            Self::Pid | Self::Cpu | Self::Memory | Self::Threads | Self::Priority
        )
    }
    fn compare(self, a: &ProcessRow, b: &ProcessRow) -> Ordering {
        let (x, y) = (&a.info, &b.info);
        let text = |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase());
        match self {
            Self::Name => text(&x.name, &y.name),
            Self::Pid => x.id.pid.cmp(&y.id.pid),
            Self::User => text(&x.user, &y.user),
            Self::Cpu => a.cpu.unwrap_or(-1.0).total_cmp(&b.cpu.unwrap_or(-1.0)),
            Self::Memory => x.memory.cmp(&y.memory),
            Self::Threads => x.threads.cmp(&y.threads),
            Self::Priority => x.priority.cmp(&y.priority),
            Self::Command => text(&x.command, &y.command),
        }
    }
    fn text(self, row: &ProcessRow) -> String {
        let info = &row.info;
        match self {
            Self::Name => info.name.clone(),
            Self::Pid => info.id.pid.to_string(),
            Self::User => info.user.clone(),
            Self::Cpu => row.cpu.map(|v| format!("{v:.1}%")).unwrap_or_default(),
            Self::Memory => bytes(info.memory as f64),
            Self::Threads => info.threads.to_string(),
            Self::Priority => info.priority.map(i18n::priority).unwrap_or_default().into(),
            Self::Command => info.command.clone(),
        }
    }
}

/// Column positions across the table width. Each fixed column fits its title, given in
/// width by `title`, with room for the sort arrow; the command line takes the rest.
fn layout(width: f32, title: impl Fn(Column) -> f32) -> Vec<(Column, f32, f32)> {
    let fixed = [
        (Column::Pid, 68.0),
        (Column::User, 108.0),
        (Column::Cpu, 66.0),
        (Column::Memory, 96.0),
        (Column::Threads, 66.0),
        (Column::Priority, 100.0),
    ]
    .map(|(column, least)| (column, (title(column) + PAD * 2.0 + ARROW).max(least)));
    let name = (width * 0.22).clamp(150.0, 260.0);
    let rest = (width - name - fixed.iter().map(|(_, w)| w).sum::<f32>()).max(0.0);
    let mut x = 0.0;
    std::iter::once((Column::Name, name))
        .chain(fixed)
        .chain(std::iter::once((Column::Command, rest)))
        .map(|(column, width)| {
            let cell = (column, x, width);
            x += width;
            cell
        })
        .collect()
}

fn host_label(host: &ProcessHost) -> String {
    match host {
        ProcessHost::Windows => "Windows".into(),
        ProcessHost::Wsl(distro) => format!("WSL · {distro}"),
    }
}

/// State of the process page.
pub(in crate::app) struct ProcessPage {
    host: ProcessHost,
    search: String,
    sort: Column,
    descending: bool,
    selected: Option<ProcessId>,
    /// End action waiting for confirmation, with the process name.
    confirm: Option<(ProcessId, String, ProcessAction)>,
    /// Host the engine polls, as last requested.
    watching: Option<ProcessHost>,
    /// Commands whose results report process actions.
    pending: Vec<u64>,
    order: Option<Order>,
}

/// Row order of a table for a search and sort.
struct Order {
    table: Arc<ProcessTable>,
    search: String,
    sort: Column,
    descending: bool,
    rows: Vec<usize>,
}

impl Default for ProcessPage {
    fn default() -> Self {
        Self {
            host: ProcessHost::Windows,
            search: String::new(),
            sort: Column::Cpu,
            descending: true,
            selected: None,
            confirm: None,
            watching: None,
            pending: Vec::new(),
            order: None,
        }
    }
}

impl ProcessPage {
    /// Whether `id` answers a process action, which this page then stops waiting for.
    pub(in crate::app) fn finished(&mut self, id: u64) -> bool {
        let index = self.pending.iter().position(|pending| *pending == id);
        index.map(|index| self.pending.swap_remove(index)).is_some()
    }

    /// Indices of the rows that match the search, in sort order.
    fn order(&mut self, table: &Arc<ProcessTable>) -> Vec<usize> {
        if let Some(order) = &self.order
            && Arc::ptr_eq(&order.table, table)
            && order.search == self.search
            && order.sort == self.sort
            && order.descending == self.descending
        {
            return order.rows.clone();
        }
        let needle = self.search.trim().to_lowercase();
        let mut order: Vec<usize> = (0..table.rows.len())
            .filter(|&i| {
                let info = &table.rows[i].info;
                needle.is_empty()
                    || info.name.to_lowercase().contains(&needle)
                    || info.user.to_lowercase().contains(&needle)
                    || info.command.to_lowercase().contains(&needle)
                    || info.id.pid.to_string().contains(&needle)
            })
            .collect();
        order.sort_by(|&a, &b| {
            let (a, b) = (&table.rows[a], &table.rows[b]);
            let ordering = self
                .sort
                .compare(a, b)
                .then_with(|| a.info.id.pid.cmp(&b.info.id.pid));
            if self.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
        self.order = Some(Order {
            table: Arc::clone(table),
            search: self.search.clone(),
            sort: self.sort,
            descending: self.descending,
            rows: order.clone(),
        });
        order
    }
}

/// Paints `text` in one line inside a cell, ending in an ellipsis when it does not fit.
fn cell(ui: &egui::Ui, rect: egui::Rect, text: &str, right: bool, color: egui::Color32) {
    if text.is_empty() || rect.width() <= PAD * 2.0 {
        return;
    }
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat::simple(FontId::proportional(13.0), color),
    );
    job.wrap = TextWrapping {
        max_width: rect.width() - PAD * 2.0,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.painter().layout_job(job);
    let x = if right {
        rect.right() - PAD - galley.size().x
    } else {
        rect.left() + PAD
    };
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

impl DesktopApp {
    /// Polls the selected host while the process page is on screen.
    pub(in crate::app) fn watch_processes(&mut self, ctx: &egui::Context) {
        let hosts = &self.view.processes.hosts;
        if !hosts.is_empty() && !hosts.contains(&self.process.host) {
            self.process.host = ProcessHost::Windows;
            self.process.selected = None;
        }
        let shown = self.page == Page::Processes
            && self.dock.edge().is_none()
            && ctx.input(|i| i.viewport().minimized != Some(true));
        let watch = shown.then(|| self.process.host.clone());
        if watch != self.process.watching {
            match self
                .client
                .try_command(MonitorCommand::WatchProcesses(watch.clone()))
            {
                Ok(_) => self.process.watching = watch,
                Err(error) => self.message = Some(error),
            }
        }
    }

    /// Reports the result of a process action in the status bar.
    pub(in crate::app) fn process_result(&mut self, result: Result<(), Notice>) {
        match result {
            Err(error) => self.message = Some(error),
            Ok(()) if matches!(self.message, Some(Notice::Process(_))) => self.message = None,
            Ok(()) => {}
        }
    }

    fn act(&mut self, process: ProcessId, action: ProcessAction) {
        match self
            .client
            .try_command(MonitorCommand::Process { process, action })
        {
            Ok(id) => self.process.pending.push(id),
            Err(error) => self.message = Some(error),
        }
    }

    /// Asks before ending a process.
    fn ask(&mut self, info: &ProcessInfo, action: ProcessAction) {
        self.process.confirm = Some((info.id.clone(), info.name.clone(), action));
    }

    pub(in crate::app) fn processes(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let table = view
            .processes
            .table
            .clone()
            .filter(|table| table.host == self.process.host);
        let selected = table.as_ref().and_then(|table| {
            let id = self.process.selected.as_ref()?;
            table.rows.iter().find(|row| row.info.id == *id).cloned()
        });
        if table.is_some() && selected.is_none() {
            self.process.selected = None;
        }
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            self.hosts(ui, view);
            let order = table.as_ref().map(|table| self.process.order(table));
            self.toolbar(ui, table.as_deref(), order.as_deref(), selected.as_ref());
            if let Some(reason) = &view.processes.error {
                let t = i18n::t();
                ui.label(
                    RichText::new((t.processes_failed)(&i18n::reason(reason)))
                        .size(12.0)
                        .color(theme::ERROR),
                );
            }
            self.table(ui, table.as_ref().zip(order.as_deref()));
        });
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
            && let Some(row) = &selected
        {
            self.ask(&row.info, ProcessAction::End);
        }
        self.confirm(ui.ctx());
    }

    fn hosts(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let hosts = if view.processes.hosts.is_empty() {
                vec![ProcessHost::Windows]
            } else {
                view.processes.hosts.clone()
            };
            for host in &hosts {
                if theme::chip(ui, &host_label(host), *host == self.process.host).clicked()
                    && *host != self.process.host
                {
                    self.process.host = host.clone();
                    self.process.selected = None;
                }
            }
            if hosts.len() == 1 {
                ui.add_space(4.0);
                ui.label(RichText::new(t.wsl_hint).size(12.0).color(theme::MUTED));
            }
        });
    }

    fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        table: Option<&ProcessTable>,
        order: Option<&[usize]>,
        selected: Option<&ProcessRow>,
    ) {
        let t = i18n::t();
        let wsl = matches!(self.process.host, ProcessHost::Wsl(_));
        let button = |text: &str| vec2(label_width(ui, &[text], 14.0) + 30.0, 32.0);
        let (end, kill) = (button(t.end_process), button(t.kill_process));
        let count = match (table, order) {
            (Some(table), Some(order)) => (t.process_count)(order.len(), table.rows.len()),
            _ => String::new(),
        };
        // The search box takes the room the count and the actions leave.
        let actions = PRIORITY_WIDTH + end.x + if wsl { kill.x + 8.0 } else { 0.0 } + 24.0;
        let search = (ui.available_width() - actions - label_width(ui, &[&count], 12.0) - 24.0)
            .clamp(140.0, 300.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.process.search)
                    .hint_text(t.process_search)
                    .desired_width(search),
            );
            ui.label(RichText::new(count).size(12.0).color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(selected.is_some(), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if theme::pixel_button(ui, t.end_process, None, false, end).clicked()
                        && let Some(row) = selected
                    {
                        self.ask(&row.info, ProcessAction::End);
                    }
                    if wsl
                        && theme::pixel_button(ui, t.kill_process, None, false, kill).clicked()
                        && let Some(row) = selected
                    {
                        self.ask(&row.info, ProcessAction::Kill);
                    }
                    let current = selected.and_then(|row| row.info.priority);
                    let mut chosen = current;
                    let text = current.map_or_else(
                        || t.process_priority.to_string(),
                        |priority| (t.labeled)(t.process_priority, i18n::priority(priority)),
                    );
                    egui::ComboBox::from_id_salt("process-priority")
                        .width(PRIORITY_WIDTH)
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            for priority in Priority::SETTABLE {
                                ui.selectable_value(
                                    &mut chosen,
                                    Some(priority),
                                    i18n::priority(priority),
                                );
                            }
                        });
                    if chosen != current
                        && let (Some(row), Some(priority)) = (selected, chosen)
                    {
                        self.act(row.info.id.clone(), ProcessAction::SetPriority(priority));
                    }
                });
            });
        });
    }

    fn table(&mut self, ui: &mut egui::Ui, data: Option<(&Arc<ProcessTable>, &[usize])>) {
        let t = i18n::t();
        let width = ui.available_width();
        let columns = layout(width, |column| label_width(ui, &[column.title()], 13.0));
        let (header, _) = ui.allocate_exact_size(vec2(width, HEADER), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(header, 0.0, theme::WELL);
        painter.hline(
            header.x_range(),
            header.bottom(),
            Stroke::new(pixel::unit(ui.ctx(), 2.0), theme::INK),
        );
        for &(column, x, w) in &columns {
            let rect =
                egui::Rect::from_min_size(pos2(header.left() + x, header.top()), vec2(w, HEADER));
            if w <= 0.0 {
                continue;
            }
            let id = ui.id().with(("process-column", column as u8));
            let response = ui
                .interact(rect, id, Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, column.title())
            });
            if response.clicked() {
                if self.process.sort == column {
                    self.process.descending = !self.process.descending;
                } else {
                    self.process.sort = column;
                    self.process.descending = column.numeric();
                }
            }
            let mut title = column.title().to_string();
            if self.process.sort == column {
                title.push_str(if self.process.descending {
                    " ▼"
                } else {
                    " ▲"
                });
            }
            let color = if response.hovered() {
                theme::PINK
            } else {
                theme::INK
            };
            cell(ui, rect, &title, column.numeric(), color);
        }
        let Some((table, order)) = data else {
            ui.add_space(12.0);
            ui.label(RichText::new(t.processes_loading).color(theme::MUTED));
            return;
        };
        if order.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new(t.processes_none).color(theme::MUTED));
            return;
        }
        ui.add_space(4.0);
        // `show_rows` places rows by the item spacing of this Ui.
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical()
            .id_salt("process-rows")
            .auto_shrink([false, false])
            .show_rows(ui, ROW, order.len(), |ui, range| {
                for index in range {
                    self.row(ui, &table.rows[order[index]], &columns, index % 2 == 1);
                }
            });
    }

    fn row(
        &mut self,
        ui: &mut egui::Ui,
        row: &ProcessRow,
        columns: &[(Column, f32, f32)],
        stripe: bool,
    ) {
        let info = &row.info;
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, ROW), Sense::click());
        let selected = self.process.selected.as_ref() == Some(&info.id);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                true,
                selected,
                format!("{} {}", info.name, info.id.pid),
            )
        });
        if response.clicked() || response.secondary_clicked() {
            self.process.selected = Some(info.id.clone());
        }
        if ui.is_rect_visible(rect) {
            let fill = if selected {
                theme::TEAL_SOFT
            } else if response.hovered() {
                theme::PAGE
            } else if stripe {
                theme::WELL
            } else {
                theme::PAPER
            };
            ui.painter().rect_filled(rect, 0.0, fill);
            if selected {
                let bar = egui::Rect::from_min_size(
                    rect.min,
                    vec2(pixel::unit(ui.ctx(), 3.0), rect.height()),
                );
                ui.painter().rect_filled(bar, 0.0, theme::TEAL);
            }
            for &(column, x, w) in columns {
                let cell_rect =
                    egui::Rect::from_min_size(pos2(rect.left() + x, rect.top()), vec2(w, ROW));
                let color = match column {
                    Column::Cpu => theme::level_color(row.cpu.unwrap_or(0.0), theme::INK),
                    Column::Command | Column::User => theme::MUTED,
                    _ => theme::INK,
                };
                cell(ui, cell_rect, &column.text(row), column.numeric(), color);
            }
        }
        // The full command line shows over its own cell, shortened past a screenful.
        let command = columns
            .last()
            .map_or(f32::INFINITY, |&(_, x, _)| rect.left() + x);
        let response =
            if !info.command.is_empty() && response.hover_pos().is_some_and(|at| at.x >= command) {
                let mut text: String = info.command.chars().take(TOOLTIP_CHARS).collect();
                if text.len() < info.command.len() {
                    text.push('…');
                }
                response.on_hover_text(text)
            } else {
                response
            };
        response.context_menu(|ui| self.menu(ui, info));
    }

    fn menu(&mut self, ui: &mut egui::Ui, info: &ProcessInfo) {
        let t = i18n::t();
        if ui.button(t.end_process).clicked() {
            self.ask(info, ProcessAction::End);
        }
        if matches!(info.id.host, ProcessHost::Wsl(_)) && ui.button(t.kill_process).clicked() {
            self.ask(info, ProcessAction::Kill);
        }
        ui.separator();
        ui.menu_button(t.process_priority, |ui| {
            for priority in Priority::SETTABLE {
                if ui
                    .radio(info.priority == Some(priority), i18n::priority(priority))
                    .clicked()
                {
                    self.act(info.id.clone(), ProcessAction::SetPriority(priority));
                }
            }
        });
    }

    fn confirm(&mut self, ctx: &egui::Context) {
        let Some((id, name, action)) = self.process.confirm.clone() else {
            return;
        };
        let t = i18n::t();
        let label = if action == ProcessAction::Kill {
            t.kill_process
        } else {
            t.end_process
        };
        let modal = egui::Modal::new(egui::Id::new("process-confirm")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new((t.end_confirm)(&name, id.pid))
                    .size(15.0)
                    .strong(),
            );
            ui.label(
                RichText::new(if action == ProcessAction::Kill {
                    t.kill_warning
                } else {
                    t.end_warning
                })
                .color(theme::MUTED),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let width = |ui: &egui::Ui, text: &str| label_width(ui, &[text], 14.0) + 40.0;
                let (yes, no) = (
                    vec2(width(ui, label), 34.0),
                    vec2(width(ui, t.cancel), 34.0),
                );
                let confirm = theme::pixel_button(ui, label, None, true, yes).clicked();
                let cancel = theme::pixel_button(ui, t.cancel, None, false, no).clicked();
                (confirm, cancel)
            })
            .inner
        });
        let (confirm, cancel) = modal.inner;
        if confirm {
            self.process.confirm = None;
            self.act(id, action);
        } else if cancel || modal.should_close() {
            self.process.confirm = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_fill_the_width_and_keep_numbers_visible() {
        for width in [700.0, 1100.0, 1800.0] {
            let columns = layout(width, |column| column.title().chars().count() as f32 * 7.0);
            let (_, x, w) = *columns.last().unwrap();
            assert_eq!(columns.len(), 8);
            assert!((x + w - width).abs() < 0.01);
            assert!(
                columns
                    .iter()
                    .all(|&(c, _, w)| c == Column::Command || w >= 60.0)
            );
        }
    }
}
