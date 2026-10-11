//! Board: um kanban só com tarefas locais e cards do Notion. Tudo no teclado.

use std::collections::HashSet;

use chrono::Local;
use egui::{Align, Color32, FontId, Key, Layout, Modifiers, RichText, ScrollArea, Sense, TextEdit, Ui, Vec2};
use aic_core::quickadd::{self, due_label};
use aic_core::{Id, Priority, Subtask, Task, TaskSource, TaskStatus};

use super::statusbar::truncate;
use super::widgets::{self, I_BRANCH, I_LINK, I_NOTION, I_SLACK, I_TASK, I_TERM};
use crate::app::{App, ToastKind};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Notion,
    Mine,
}

#[derive(Default)]
pub struct BoardState {
    pub filter: Filter,
    pub selected: Option<Id>,
    pub marked: HashSet<Id>,
    pub search: String,
    pub searching: bool,
    /// Adição inline no fim de uma coluna.
    pub adding: Option<(TaskStatus, String)>,
    /// Foco no título do editor (tecla `e`).
    pub edit_title: bool,
    pub new_subtask: String,
    pub focus_subtask: bool,
    pub only_ws: bool,
}

fn priority_color(p: Priority, theme: &Theme) -> Color32 {
    match p {
        Priority::Urgent => theme.err,
        Priority::High => theme.warn,
        Priority::Normal => theme.accent,
        Priority::Low => theme.faint,
    }
}

/// Tarefas visíveis por coluna, já ordenadas.
fn columns(app: &App) -> Vec<(TaskStatus, Vec<Task>)> {
    let q = app.board.search.to_lowercase();
    let ws = app.current_ws().map(|w| w.id);
    TaskStatus::BOARD
        .iter()
        .map(|s| {
            let mut v: Vec<Task> = app
                .tasks
                .iter()
                .filter(|t| t.status == *s)
                .filter(|t| match app.board.filter {
                    Filter::All => true,
                    Filter::Notion => t.source == TaskSource::Notion,
                    Filter::Mine => t.source == TaskSource::Local,
                })
                .filter(|t| !app.board.only_ws || t.workspace.is_none() || t.workspace == ws)
                .filter(|t| {
                    q.is_empty()
                        || t.title.to_lowercase().contains(&q)
                        || t.card_key.as_deref().is_some_and(|k| k.to_lowercase().contains(&q))
                        || t.repo.as_deref().is_some_and(|r| r.to_lowercase().contains(&q))
                        || t.tags.iter().any(|g| g.to_lowercase().contains(&q))
                })
                .cloned()
                .collect();
            if *s == TaskStatus::Done {
                v.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            } else {
                v.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.due.cmp(&b.due)).then(a.rank.total_cmp(&b.rank)));
            }
            (*s, v)
        })
        .collect()
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let cols = columns(app);
    keyboard(app, ui, &cols);

    // ── barra ──
    ui.horizontal(|ui| {
        widgets::heading(ui, &format!("{}  Board", widgets::I_BOARD), &theme);
        ui.add_space(12.0);
        for (f, label, key) in [(Filter::All, "Tudo", "1"), (Filter::Notion, "Notion", "2"), (Filter::Mine, "Minhas", "3")] {
            if ui.selectable_label(app.board.filter == f, label).on_hover_text(format!("Filtro ({key})")).clicked() {
                app.board.filter = f;
            }
        }
        ui.checkbox(&mut app.board.only_ws, "só deste workspace");
        ui.add_space(8.0);
        let search = ui.add(TextEdit::singleline(&mut app.board.search).hint_text("/ buscar").desired_width(200.0));
        if app.board.searching {
            search.request_focus();
            app.board.searching = false;
        }
        if search.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            app.board.search.clear();
            search.surrender_focus();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(format!("{}  Nova tarefa", widgets::I_PLUS)).on_hover_text(app.keymap.tip(crate::actions::Action::QuickAdd, "Nova tarefa (captura rápida)")).clicked() {
                app.open_quick_add(None);
            }
            crate::notion_ui::toolbar(app, ui);
        });
    });
    ui.add_space(8.0);

    // ── colunas + editor ──
    let selected_task = app.board.selected.and_then(|id| app.tasks.iter().find(|t| t.id == id).cloned());
    if let Some(task) = &selected_task {
        egui::Panel::right("task-editor")
            .resizable(true)
            .default_size(380.0)
            .size_range(300.0..=600.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 0, top: 0, bottom: 0 }))
            .show(ui, |ui| editor(app, ui, task.clone()));
    }

    let n = cols.len() as f32;
    let gap = 10.0;
    let col_w = ((ui.available_width() - gap * (n - 1.0)) / n).max(180.0);
    ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (status, tasks) in &cols {
                column(app, ui, *status, tasks, col_w);
            }
        });
    });
}

fn column(app: &mut App, ui: &mut Ui, status: TaskStatus, tasks: &[Task], width: f32) {
    let theme = app.theme.clone();
    let t = app.t();
    let height = ui.available_height();
    let frame = egui::Frame::new().fill(theme.bg2.gamma_multiply(0.85)).corner_radius(theme.radius()).inner_margin(egui::Margin::same(8)).stroke(egui::Stroke::new(1.0, theme.border));
    let (_, dropped) = ui.dnd_drop_zone::<Id, ()>(frame, |ui| {
        ui.set_width(width - 16.0);
        ui.set_min_height(height - 20.0);
        ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let color = match status {
                TaskStatus::Todo => theme.dim,
                TaskStatus::Doing => theme.ok,
                TaskStatus::Review => theme.attention,
                TaskStatus::Done => theme.warn,
                TaskStatus::Archived => theme.faint,
            };
            ui.label(RichText::new(status.label().to_uppercase()).font(FontId::monospace(12.0)).color(color).strong());
            ui.label(RichText::new(tasks.len().to_string()).small().color(theme.faint));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, widgets::I_PLUS, &format!("Nova tarefa em {}", status.label()), "Nova tarefa nesta coluna (t)").clicked() {
                    app.board.adding = Some((status, String::new()));
                }
            });
        });
        ui.add_space(4.0);
        ScrollArea::vertical().id_salt(("col", status as u8)).auto_shrink([false, false]).show(ui, |ui| {
            for task in tasks {
                let selected = app.board.selected == Some(task.id);
                let marked = app.board.marked.contains(&task.id);
                // O cartão sente clique E arrasto num widget só. Com o `dnd_drag_source` do egui
                // o arrasto ficava por cima do cartão e o egui descartava o clique de baixo:
                // clicar, dar duplo clique ou abrir o menu num cartão não fazia nada.
                let r = card(app, ui, task, selected, marked, t);
                if r.drag_started() {
                    egui::DragAndDrop::set_payload(ui.ctx(), task.id);
                }
                if r.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    if let Some(p) = ui.ctx().pointer_interact_pos() {
                        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("task-ghost")));
                        let galley = painter.layout_no_wrap(truncate(&task.title, 40), FontId::proportional(13.0), theme.ink);
                        let rect = egui::Rect::from_min_size(p + Vec2::new(14.0, -galley.size().y / 2.0), galley.size());
                        painter.rect(rect.expand(6.0), theme.radius(), theme.bg3, egui::Stroke::new(1.0, theme.accent), egui::StrokeKind::Outside);
                        painter.galley(rect.min, galley, theme.ink);
                    }
                }
                if selected {
                    r.scroll_to_me(None);
                }
                if r.clicked() {
                    app.board.selected = Some(task.id);
                    if ui.input(|i| i.modifiers.ctrl) {
                        toggle_mark(app, task.id);
                    }
                }
                if r.double_clicked() {
                    app.open_task(task.id);
                }
                task_menu(app, &r, task);
                ui.add_space(6.0);
            }
            // adição inline
            if let Some((s, text)) = &mut app.board.adding {
                if *s == status {
                    let edit = ui.add(TextEdit::singleline(text).hint_text("título  #repo @hoje !alta").desired_width(f32::INFINITY));
                    edit.request_focus();
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    let esc = ui.input(|i| i.key_pressed(Key::Escape));
                    if enter && !text.trim().is_empty() {
                        let text = std::mem::take(text);
                        app.quick_add(&text, status, None);
                        // continua aberto pra emendar várias
                    } else if esc || (edit.lost_focus() && text.trim().is_empty()) {
                        app.board.adding = None;
                    }
                }
            } else if tasks.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| ui.label(widgets::subtle("vazio · t pra adicionar", &theme).small()));
            }
        });
        });
    });
    if let Some(id) = dropped {
        move_task(app, *id, status);
    }
}

fn card(app: &App, ui: &mut Ui, task: &Task, selected: bool, marked: bool, t: f64) -> egui::Response {
    let theme = &app.theme;
    let today = Local::now().date_naive();
    let accent = priority_color(task.priority, theme);
    widgets::card_sense(ui, theme, selected || marked, accent, Sense::click_and_drag(), |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            if marked {
                ui.label(RichText::new("◆").color(theme.accent_hi));
            }
            if let Some(k) = &task.card_key {
                ui.label(RichText::new(k).font(FontId::monospace(11.0)).color(theme.accent_hi));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if task.source == TaskSource::Notion {
                    ui.label(RichText::new(I_NOTION).color(theme.dim)).on_hover_text("Card do Notion");
                }
                if task.slack_thread.is_some() {
                    ui.label(RichText::new(I_SLACK).color(theme.dim)).on_hover_text("Thread do Slack vinculada");
                }
                if task.priority >= Priority::High {
                    ui.label(RichText::new(task.priority.label()).small().color(accent));
                }
            });
        });
        let done = task.status == TaskStatus::Done;
        let mut title = RichText::new(&task.title).color(if done { theme.dim } else { theme.ink });
        if done {
            title = title.strikethrough();
        }
        ui.add(egui::Label::new(title).wrap());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if let Some(r) = &task.repo {
                ui.label(RichText::new(format!("#{r}")).small().color(theme.accent));
            }
            if let Some(d) = task.due {
                let overdue = task.is_overdue(today);
                ui.label(RichText::new(format!("󰃭 {}", due_label(d, today))).small().color(if overdue { theme.err } else { theme.warn }));
            }
            if let Some((a, b)) = task.subtasks_progress() {
                ui.label(RichText::new(format!("☑ {a}/{b}")).small().color(if a == b { theme.ok } else { theme.dim }));
            }
            for tag in &task.tags {
                ui.label(RichText::new(format!("~{tag}")).small().color(theme.faint));
            }
            if let Some(sid) = task.session.filter(|s| app.sessions.contains_key(s)) {
                let state = app.state_of(sid);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                widgets::state_dot(ui, r.center(), 3.5, state, theme, t);
                ui.label(RichText::new(format!("{I_TERM} {}", truncate(&app.session_label(sid), 16))).small().color(theme.dim));
            }
        });
    })
}

fn task_menu(app: &mut App, resp: &egui::Response, task: &Task) {
    let id = task.id;
    resp.context_menu(|ui| {
        if ui.button(format!("{I_TERM} Abrir sessão  (Enter)")).clicked() {
            app.open_task(id);
            ui.close();
        }
        ui.menu_button("Mover para", |ui| {
            for s in TaskStatus::BOARD {
                if ui.button(s.label()).clicked() {
                    move_task(app, id, s);
                    ui.close();
                }
            }
        });
        ui.menu_button("Prioridade", |ui| {
            for p in [Priority::Urgent, Priority::High, Priority::Normal, Priority::Low] {
                if ui.button(p.label()).clicked() {
                    app.update_task(id, |t| t.priority = p);
                    ui.close();
                }
            }
        });
        if ui.button("Arquivar  (d)").clicked() {
            move_task(app, id, TaskStatus::Archived);
            ui.close();
        }
        if ui.button(RichText::new("Apagar").color(app.theme.err)).clicked() {
            app.modal = Some(super::modals::Modal::ConfirmDeleteTask(id));
            ui.close();
        }
    });
}

fn move_task(app: &mut App, id: Id, status: TaskStatus) {
    app.update_task(id, |t| t.status = status);
    if status == TaskStatus::Archived && app.board.selected == Some(id) {
        app.board.selected = None;
    }
    crate::notion_ui::on_status_change(app, id, status);
}

fn toggle_mark(app: &mut App, id: Id) {
    if !app.board.marked.remove(&id) {
        app.board.marked.insert(id);
    }
}

/// Alvos de uma ação em lote: os marcados, ou o selecionado.
fn targets(app: &App) -> Vec<Id> {
    if app.board.marked.is_empty() { app.board.selected.into_iter().collect() } else { app.board.marked.iter().copied().collect() }
}

fn keyboard(app: &mut App, ui: &Ui, cols: &[(TaskStatus, Vec<Task>)]) {
    if app.modal.is_some() || ui.ctx().egui_wants_keyboard_input() || app.board.adding.is_some() {
        return;
    }
    // posição atual
    let pos = app.board.selected.and_then(|id| {
        cols.iter().enumerate().find_map(|(c, (_, ts))| ts.iter().position(|t| t.id == id).map(|r| (c, r)))
    });
    let (mut c, mut r) = pos.unwrap_or((0, 0));
    // combinação exata: `consume_key` deixaria `l` engolir `Shift+L` (mover cartão)
    let pressed = |k: Key, m: Modifiers| ui.input_mut(|i| crate::actions::consume_exact(i, m, k));
    let none = Modifiers::NONE;
    let shift = Modifiers::SHIFT;

    let mut moved = false;
    if pressed(Key::J, none) || pressed(Key::ArrowDown, none) {
        r += usize::from(pos.is_some());
        moved = true;
    }
    if pressed(Key::K, none) || pressed(Key::ArrowUp, none) {
        r = r.saturating_sub(1);
        moved = true;
    }
    if pressed(Key::L, none) || pressed(Key::ArrowRight, none) {
        c = (c + 1).min(cols.len() - 1);
        moved = true;
    }
    if pressed(Key::H, none) || pressed(Key::ArrowLeft, none) {
        c = c.saturating_sub(1);
        moved = true;
    }
    if moved {
        // pula colunas vazias na direção do movimento
        let len = cols[c].1.len();
        if len > 0 {
            app.board.selected = Some(cols[c].1[r.min(len - 1)].id);
        } else if pos.is_none() {
            app.board.selected = cols.iter().find_map(|(_, ts)| ts.first().map(|t| t.id));
        } else {
            app.board.selected = None;
            app.board.adding = None;
            // coluna vazia: deixa a seleção "na coluna" via adding desligado
            if let Some(t) = cols[c].1.first() {
                app.board.selected = Some(t.id);
            }
        }
    }

    if pressed(Key::L, shift) || pressed(Key::ArrowRight, shift) {
        for id in targets(app) {
            let s = app.tasks.iter().find(|t| t.id == id).map(|t| t.status.next());
            if let Some(s) = s {
                move_task(app, id, s);
            }
        }
    }
    if pressed(Key::H, shift) || pressed(Key::ArrowLeft, shift) {
        for id in targets(app) {
            let s = app.tasks.iter().find(|t| t.id == id).map(|t| t.status.prev());
            if let Some(s) = s {
                move_task(app, id, s);
            }
        }
    }
    if pressed(Key::Enter, none) {
        if let Some(id) = app.board.selected {
            app.open_task(id);
        }
    }
    if pressed(Key::E, none) {
        app.board.edit_title = true;
    }
    if pressed(Key::X, none) {
        for id in targets(app) {
            let s = app.tasks.iter().find(|t| t.id == id).map(|t| if t.status == TaskStatus::Done { TaskStatus::Doing } else { TaskStatus::Done });
            if let Some(s) = s {
                move_task(app, id, s);
            }
        }
        app.board.marked.clear();
    }
    if pressed(Key::D, none) {
        let n = targets(app).len();
        for id in targets(app) {
            move_task(app, id, TaskStatus::Archived);
        }
        app.board.marked.clear();
        if n > 0 {
            app.toast(&format!("{n} arquivada(s)"), ToastKind::Info);
        }
    }
    if pressed(Key::Space, none) {
        if let Some(id) = app.board.selected {
            toggle_mark(app, id);
        }
    }
    if pressed(Key::T, none) || pressed(Key::N, none) {
        app.board.adding = Some((cols[c].0, String::new()));
    }
    if pressed(Key::P, shift) {
        if let Some(id) = app.board.selected {
            crate::notion_ui::promote(app, id);
        }
    }
    if pressed(Key::Delete, none) {
        if let Some(id) = app.board.selected {
            app.modal = Some(super::modals::Modal::ConfirmDeleteTask(id));
        }
    }
    if pressed(Key::Slash, none) {
        app.board.searching = true;
    }
    if pressed(Key::Escape, none) {
        if app.board.marked.is_empty() {
            app.board.selected = None;
        }
        app.board.marked.clear();
    }
    for (k, f) in [(Key::Num1, Filter::All), (Key::Num2, Filter::Notion), (Key::Num3, Filter::Mine)] {
        if pressed(k, none) {
            app.board.filter = f;
        }
    }
}

fn editor(app: &mut App, ui: &mut Ui, mut task: Task) {
    let theme = app.theme.clone();
    let original = task.clone();
    let today = Local::now().date_naive();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{I_TASK}  TAREFA")).font(FontId::monospace(11.0)).color(theme.dim));
            if task.source == TaskSource::Notion {
                ui.label(RichText::new(format!("{I_NOTION} Notion")).small().color(theme.dim));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, widgets::I_CLOSE, "Fechar editor", "Fechar o editor (Esc)").clicked() {
                    app.board.selected = None;
                }
            });
        });
        let title = ui.add(TextEdit::multiline(&mut task.title).font(FontId::proportional(17.0)).desired_rows(1).desired_width(f32::INFINITY).frame(egui::Frame::NONE));
        if app.board.edit_title {
            title.request_focus();
            app.board.edit_title = false;
        }
        if title.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            title.surrender_focus();
        }
        ui.add_space(6.0);

        egui::Grid::new("task-fields").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label(widgets::subtle("Status", &theme));
            ui.horizontal(|ui| {
                for s in TaskStatus::BOARD {
                    ui.selectable_value(&mut task.status, s, s.label());
                }
            });
            ui.end_row();

            ui.label(widgets::subtle("Prioridade", &theme));
            ui.horizontal(|ui| {
                for p in [Priority::Low, Priority::Normal, Priority::High, Priority::Urgent] {
                    ui.selectable_value(&mut task.priority, p, RichText::new(p.label()).color(priority_color(p, &theme)));
                }
            });
            ui.end_row();

            ui.label(widgets::subtle("Prazo", &theme));
            ui.horizontal(|ui| {
                let mut txt = ui.data(|d| d.get_temp::<String>(egui::Id::new(("due", task.id)))).unwrap_or_else(|| task.due.map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_default());
                let r = ui.add(TextEdit::singleline(&mut txt).hint_text("hoje, sexta, 20/10…").desired_width(130.0));
                if r.changed() {
                    ui.data_mut(|d| d.insert_temp(egui::Id::new(("due", task.id)), txt.clone()));
                }
                if r.lost_focus() {
                    task.due = if txt.trim().is_empty() { None } else { quickadd::parse_date(txt.trim(), today).or(task.due) };
                    ui.data_mut(|d| d.remove::<String>(egui::Id::new(("due", task.id))));
                }
                if let Some(d) = task.due {
                    ui.label(RichText::new(due_label(d, today)).small().color(if task.is_overdue(today) { theme.err } else { theme.warn }));
                }
            });
            ui.end_row();

            ui.label(widgets::subtle("Repo", &theme));
            let label = task.repo.clone().unwrap_or_else(|| "—".into());
            egui::ComboBox::from_id_salt(("repo", task.id)).selected_text(label).width(220.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut task.repo, None, "—");
                for r in &app.repos {
                    ui.selectable_value(&mut task.repo, Some(r.clone()), r);
                }
            });
            ui.end_row();

            ui.label(widgets::subtle("Card", &theme));
            let mut key = task.card_key.clone().unwrap_or_default();
            if ui.add(TextEdit::singleline(&mut key).hint_text("DEV-142").desired_width(140.0)).changed() {
                task.card_key = (!key.trim().is_empty()).then(|| key.trim().to_uppercase());
            }
            ui.end_row();

            ui.label(widgets::subtle(format!("{I_NOTION} Notion"), &theme));
            let mut url = task.notion_url.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                if ui.add(TextEdit::singleline(&mut url).hint_text("link do card").desired_width(200.0)).changed() {
                    task.notion_url = (!url.trim().is_empty()).then(|| url.trim().to_string());
                }
                if let Some(u) = &task.notion_url {
                    if ui.small_button("↗").clicked() {
                        let _ = open::that_detached(u);
                    }
                }
            });
            ui.end_row();

            ui.label(widgets::subtle(format!("{I_SLACK} Slack"), &theme));
            let mut th = task.slack_thread.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                if ui.add(TextEdit::singleline(&mut th).hint_text("link da thread").desired_width(200.0)).changed() {
                    task.slack_thread = (!th.trim().is_empty()).then(|| th.trim().to_string());
                }
                if let Some(u) = &task.slack_thread {
                    if ui.small_button("↗").clicked() {
                        let _ = open::that_detached(u);
                    }
                }
            });
            ui.end_row();

            ui.label(widgets::subtle("Tags", &theme));
            let mut tags = task.tags.join(" ");
            if ui.add(TextEdit::singleline(&mut tags).hint_text("front bug …").desired_width(200.0)).changed() {
                task.tags = tags.split_whitespace().map(|s| s.trim_start_matches('~').to_string()).collect();
            }
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.label(RichText::new("SUBTAREFAS").font(FontId::monospace(11.0)).color(theme.dim));
        let mut remove = None;
        for (i, s) in task.subtasks.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.done, "");
                ui.add(TextEdit::singleline(&mut s.text).frame(egui::Frame::NONE).desired_width(ui.available_width() - 30.0));
                if widgets::icon_button(ui, "×", "Remover subtarefa", "Remover subtarefa").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            task.subtasks.remove(i);
        }
        let r = ui.add(TextEdit::singleline(&mut app.board.new_subtask).hint_text("+ subtarefa (Enter adiciona, Tab também)").desired_width(f32::INFINITY));
        if app.board.focus_subtask {
            r.request_focus();
            app.board.focus_subtask = false;
        }
        let tab = r.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
        if (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) || tab {
            let txt = std::mem::take(&mut app.board.new_subtask);
            if !txt.trim().is_empty() {
                task.subtasks.push(Subtask { text: txt.trim().to_string(), done: false });
            }
            app.board.focus_subtask = true;
        }

        ui.add_space(8.0);
        ui.label(RichText::new("NOTAS").font(FontId::monospace(11.0)).color(theme.dim));
        ui.add(TextEdit::multiline(&mut task.notes).hint_text("contexto, links, critérios de aceite… (vai pro Claude junto com a tarefa)").desired_rows(6).desired_width(f32::INFINITY));

        ui.add_space(10.0);
        ui.label(RichText::new("SESSÃO").font(FontId::monospace(11.0)).color(theme.dim));
        let live = task.session.filter(|s| app.sessions.contains_key(s));
        match live {
            Some(sid) => {
                ui.horizontal(|ui| {
                    let state = app.state_of(sid);
                    widgets::state_badge(ui, app.skin.state_label(state), state, &theme, app.t());
                    if ui.link(app.session_label(sid)).clicked() {
                        app.focus(sid);
                    }
                    if let Some(b) = app.sessions.get(&sid).and_then(|r| r.worktree_branch()) {
                        ui.label(RichText::new(format!("{I_BRANCH} {b}")).small().color(theme.dim));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("󰈙 Mandar contexto pro Claude").clicked() {
                        app.write_context(sid);
                    }
                    if ui.button(format!("{I_LINK} Desvincular")).clicked() {
                        task.session = None;
                        if let Some(rt) = app.sessions.get_mut(&sid) {
                            rt.s.task = None;
                        }
                    }
                });
            }
            None => {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new(format!("{I_TERM}  Abrir sessão Claude  ⏎")).strong()).fill(theme.accent_lo)).clicked() {
                        app.open_task(task.id);
                    }
                    egui::ComboBox::from_id_salt(("link", task.id)).selected_text(format!("{I_LINK} vincular a…")).show_ui(ui, |ui| {
                        let ids: Vec<Id> = app.workspaces.iter().flat_map(|w| w.sessions.clone()).collect();
                        for sid in ids {
                            if ui.button(app.session_label(sid)).clicked() {
                                task.session = Some(sid);
                                if let Some(rt) = app.sessions.get_mut(&sid) {
                                    rt.s.task = Some(task.id);
                                }
                            }
                        }
                    });
                });
            }
        }

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            crate::notion_ui::editor_buttons(app, ui, &task);
            if ui.button("Arquivar").on_hover_text("Tira do board sem apagar (d)").clicked() {
                task.status = TaskStatus::Archived;
                app.board.selected = None;
            }
            if ui.button(RichText::new("Apagar").color(theme.err)).on_hover_text("Apaga a tarefa (Delete)").clicked() {
                app.modal = Some(super::modals::Modal::ConfirmDeleteTask(task.id));
            }
        });
        ui.add_space(6.0);
        ui.label(widgets::subtle(format!("criada {} · atualizada {}", task.created_at.with_timezone(&Local).format("%d/%m %H:%M"), task.updated_at.with_timezone(&Local).format("%d/%m %H:%M")), &theme).small());
    });

    if task != original {
        let status_changed = task.status != original.status;
        task.touch();
        let id = task.id;
        let st = task.status;
        app.save_task(task);
        if status_changed {
            crate::notion_ui::on_status_change(app, id, st);
        }
    }
}
