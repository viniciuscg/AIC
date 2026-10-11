//! Visualizações por bloco:
//! - **Blocos**: um bloco embaixo do outro, cada um com seus terminais, card, notas e tarefas;
//! - **Abas**: um bloco por aba, terminais divididos (o jeito tmux).

use egui::{Align, Color32, CursorIcon, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, TextEdit, Ui, UiBuilder, Vec2};
use aic_core::{Id, Module, SessionState, Split, TaskStatus};

use super::sessions::pane;
use super::statusbar::truncate;
use super::widgets::{self, I_CLOSE, I_PLUS, I_TASK, I_TERM};
use crate::actions::Action;
use crate::app::App;
use crate::theme::Theme;

const HEADER: f32 = 34.0;
const SIDE_W: f32 = 300.0;

/// Retângulos dos terminais de um bloco conforme o arranjo.
pub fn split_rects(area: Rect, n: usize, split: Split, gap: f32) -> Vec<Rect> {
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![area];
    }
    let mut out = Vec::with_capacity(n);
    match split {
        Split::Row => {
            let w = (area.width() - gap * (n - 1) as f32) / n as f32;
            for i in 0..n {
                out.push(Rect::from_min_size(area.min + Vec2::new(i as f32 * (w + gap), 0.0), Vec2::new(w, area.height())));
            }
        }
        Split::Column => {
            let h = (area.height() - gap * (n - 1) as f32) / n as f32;
            for i in 0..n {
                out.push(Rect::from_min_size(area.min + Vec2::new(0.0, i as f32 * (h + gap)), Vec2::new(area.width(), h)));
            }
        }
        Split::Main => {
            let w = (area.width() - gap) * 0.6;
            out.push(Rect::from_min_size(area.min, Vec2::new(w, area.height())));
            let rest = Rect::from_min_max(Pos2::new(area.left() + w + gap, area.top()), area.max);
            out.extend(split_rects(rest, n - 1, Split::Column, gap));
        }
        Split::Grid => {
            let cols = (n as f32).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            let w = (area.width() - gap * (cols - 1) as f32) / cols as f32;
            let h = (area.height() - gap * (rows - 1) as f32) / rows as f32;
            for i in 0..n {
                let (r, c) = (i / cols, i % cols);
                out.push(Rect::from_min_size(area.min + Vec2::new(c as f32 * (w + gap), r as f32 * (h + gap)), Vec2::new(w, h)));
            }
        }
    }
    out
}

// ───────────────────────── Blocos (lista) ─────────────────────────

pub fn list(app: &mut App, ui: &mut Ui) {
    let blocks = app.ws_blocks();
    if blocks.is_empty() {
        return super::sessions::empty(app, ui);
    }
    let current = app.current_block();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, b) in blocks.iter().enumerate() {
            block_section(app, ui, *b, i, Some(*b) == current);
            ui.add_space(10.0);
        }
        ui.horizontal(|ui| {
            if ui.button(format!("{I_PLUS}  Novo bloco  ({})", app.keymap.label(Action::NewBlock))).clicked() {
                app.run_action(ui.ctx(), Action::NewBlock);
            }
            if app.cfg.on(Module::Launcher) && ui.button(format!("{I_TASK}  Abrir card  ({})", app.keymap.label(Action::OpenCard))).clicked() {
                app.run_action(ui.ctx(), Action::OpenCard);
            }
        });
    });
}

fn block_section(app: &mut App, ui: &mut Ui, id: Id, index: usize, current: bool) {
    let Some(b) = app.blocks.get(&id).cloned() else { return };
    let theme = app.theme.clone();
    let color = theme.agent(b.color);
    let width = ui.available_width();
    let body_h = if b.collapsed { 0.0 } else { b.height };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEADER + body_h + if b.collapsed { 0.0 } else { 8.0 }), Sense::hover());

    // moldura
    if let Some(o) = theme.hard_shadow {
        ui.painter().rect_filled(rect.translate(Vec2::splat(o)), theme.radius(), Color32::from_black_alpha(200));
    }
    ui.painter().rect_filled(rect, theme.radius(), theme.bg2);
    ui.painter().rect_stroke(rect, theme.radius(), Stroke::new(if current { 2.0 } else { 1.0 }, if current { color } else { theme.border }), egui::StrokeKind::Inside);
    ui.painter().rect_filled(Rect::from_min_size(rect.min, Vec2::new(4.0, rect.height())), egui::CornerRadius::ZERO, color);

    let hrect = Rect::from_min_size(rect.min + Vec2::new(4.0, 0.0), Vec2::new(rect.width() - 4.0, HEADER));
    block_header(app, ui, id, hrect, index, current, true);

    if b.collapsed {
        return;
    }
    let body = Rect::from_min_max(Pos2::new(rect.left() + 8.0, hrect.bottom() + 2.0), Pos2::new(rect.right() - 6.0, rect.bottom() - 8.0));
    block_body(app, ui, id, body);

    // alça de redimensionar (arrasta a borda de baixo)
    let handle = Rect::from_min_max(Pos2::new(rect.left() + 20.0, rect.bottom() - 6.0), Pos2::new(rect.right() - 20.0, rect.bottom() + 2.0));
    let r = ui.interact(handle, egui::Id::new(("block-resize", id)), Sense::drag());
    if r.hovered() || r.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        ui.painter().line_segment([handle.center() - Vec2::new(20.0, 0.0), handle.center() + Vec2::new(20.0, 0.0)], Stroke::new(3.0, theme.accent));
    }
    if r.dragged() {
        let dy = r.drag_delta().y;
        if let Some(bl) = app.blocks.get_mut(&id) {
            bl.height = (bl.height + dy).clamp(160.0, 1400.0);
        }
    }
    if r.drag_stopped() {
        app.save_block(id);
    }
}

/// Cabeçalho do bloco: nome, card, estados dos terminais e ações (com o atalho na dica).
fn block_header(app: &mut App, ui: &mut Ui, id: Id, hrect: Rect, index: usize, current: bool, collapsible: bool) {
    let Some(b) = app.blocks.get(&id).cloned() else { return };
    let theme = app.theme.clone();
    let t = app.t();
    // fundo clicável primeiro (os botões por cima continuam funcionando)
    let bg = ui.interact(hrect, egui::Id::new(("block-head", id)), Sense::click());
    if bg.clicked() {
        app.focus_block(id);
    }
    if bg.double_clicked() {
        app.modal = Some(super::modals::Modal::RenameBlock { id, text: b.name.clone(), first: true });
    }
    block_menu(app, &bg, id);

    let mut h = ui.new_child(UiBuilder::new().max_rect(hrect.shrink2(Vec2::new(8.0, 4.0))).layout(Layout::left_to_right(Align::Center)));
    let ui = &mut h;
    if collapsible {
        let arrow = if b.collapsed { "▸" } else { "▾" };
        let label = if b.collapsed { "Expandir bloco" } else { "Recolher bloco" };
        let r = ui.add(egui::Button::new(arrow).frame(false));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        if r.on_hover_text(app.keymap.tip(Action::CollapseBlock, label)).clicked() {
            app.update_block(id, |b| b.collapsed = !b.collapsed);
        }
    }
    if index < 9 {
        ui.label(RichText::new(format!("{}", index + 1)).font(FontId::monospace(10.0)).color(theme.faint)).on_hover_text(format!("Alt+{}", index + 1));
    }
    ui.label(RichText::new(&b.name).strong().size(15.0).color(if current { theme.accent_hi } else { theme.ink }));
    if let Some(task) = b.task.and_then(|tid| app.tasks.iter().find(|t| t.id == tid)).cloned() {
        let label = match &task.card_key {
            Some(k) => format!("{I_TASK} {k}"),
            None => format!("{I_TASK} {}", truncate(&task.title, 28)),
        };
        let color = match task.status {
            TaskStatus::Done => theme.ok,
            TaskStatus::Review => theme.attention,
            _ => theme.accent_hi,
        };
        if widgets::chip(ui, label, color, &theme).on_hover_text(format!("{} · {}", task.title, task.status.label())).clicked() {
            app.board.selected = Some(task.id);
            app.set_view(crate::app::View::Board);
        }
    }
    // pontinho por terminal
    for p in &b.panes {
        let st = app.state_of(*p);
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::click());
        widgets::state_dot(ui, r.center(), 4.0, st, &theme, t);
        if resp.on_hover_text(format!("{} · {}", app.session_label(*p), app.skin.state_label(st))).clicked() {
            app.focus(*p);
        }
    }
    let agents: usize = b.panes.iter().filter_map(|p| app.sessions.get(p)).map(|r| r.live.agents.len()).sum();
    if agents > 0 {
        ui.label(RichText::new(format!("{} {agents}", widgets::I_AGENTS)).small().color(theme.attention));
    }

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if widgets::icon_button(ui, I_CLOSE, "Fechar bloco", &app.keymap.tip(Action::CloseBlock, "Fechar bloco")).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::CloseBlock);
        }
        if widgets::icon_button(ui, "◧", "Painel do bloco", &app.keymap.tip(Action::ToggleBlockSide, "Painel do bloco: card, notas, tarefas")).clicked() {
            app.update_block(id, |b| b.side = !b.side);
        }
        if ui.small_button(format!("⊞ {}", b.split.label())).on_hover_text(app.keymap.tip(Action::CycleSplit, "Arranjo dos terminais")).clicked() {
            app.update_block(id, |b| b.split = b.split.next());
        }
        if ui.small_button(format!("{I_TERM} terminal")).on_hover_text(app.keymap.tip(Action::NewShell, "Novo terminal neste bloco")).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::NewShell);
        }
        if ui.small_button(format!("{I_PLUS} Claude")).on_hover_text(app.keymap.tip(Action::NewSession, "Novo Claude neste bloco")).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::NewSession);
        }
    });
}

fn block_menu(app: &mut App, resp: &egui::Response, id: Id) {
    resp.context_menu(|ui| {
        if ui.button(format!("Renomear  ({})", app.keymap.label(Action::RenameBlock))).clicked() {
            app.focus_block(id);
            let name = app.blocks.get(&id).map(|b| b.name.clone()).unwrap_or_default();
            app.modal = Some(super::modals::Modal::RenameBlock { id, text: name, first: true });
            ui.close();
        }
        if app.cfg.on(Module::Board) && ui.button(format!("Vincular tarefa/card  ({})", app.keymap.label(Action::LinkTaskToSession))).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::LinkTaskToSession);
            ui.close();
        }
        ui.menu_button("Arranjo", |ui| {
            for s in Split::ALL {
                if ui.button(s.label()).clicked() {
                    app.update_block(id, |b| b.split = s);
                    ui.close();
                }
            }
        });
        if ui.button(format!("Subir  ({})", app.keymap.label(Action::MoveBlockUp))).clicked() {
            move_block(app, id, -1);
            ui.close();
        }
        if ui.button(format!("Descer  ({})", app.keymap.label(Action::MoveBlockDown))).clicked() {
            move_block(app, id, 1);
            ui.close();
        }
        ui.separator();
        if ui.button(RichText::new(format!("Fechar bloco  ({})", app.keymap.label(Action::CloseBlock))).color(app.theme.err)).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::CloseBlock);
            ui.close();
        }
    });
}

pub fn move_block(app: &mut App, id: Id, delta: i32) {
    if let Some(w) = app.current_ws_mut() {
        if let Some(i) = w.blocks.iter().position(|b| *b == id) {
            let j = (i as i32 + delta).clamp(0, w.blocks.len() as i32 - 1) as usize;
            w.blocks.swap(i, j);
        }
    }
    app.persist_ws();
}

/// Terminais (no arranjo do bloco) + painel lateral.
fn block_body(app: &mut App, ui: &mut Ui, id: Id, body: Rect) {
    let Some(b) = app.blocks.get(&id).cloned() else { return };
    let side_on = b.side && body.width() > 700.0;
    let panes_rect = if side_on { Rect::from_min_max(body.min, Pos2::new(body.right() - SIDE_W - 8.0, body.bottom())) } else { body };
    if b.panes.is_empty() {
        empty_block(app, ui, id, panes_rect);
    } else {
        let rects = split_rects(panes_rect, b.panes.len(), b.split, 6.0);
        for (p, r) in b.panes.iter().zip(rects) {
            pane(app, ui, *p, r, true);
        }
    }
    if side_on {
        let side = Rect::from_min_max(Pos2::new(body.right() - SIDE_W, body.top()), body.max);
        side_panel(app, ui, id, side);
    }
}

fn empty_block(app: &mut App, ui: &mut Ui, id: Id, r: Rect) {
    let theme = app.theme.clone();
    ui.painter().rect_stroke(r, theme.radius(), Stroke::new(1.0, theme.border), egui::StrokeKind::Inside);
    let mut c = ui.new_child(UiBuilder::new().max_rect(r.shrink(20.0)).layout(Layout::top_down(Align::Center)));
    c.add_space(r.height() * 0.25);
    c.label(RichText::new("Bloco vazio").size(16.0));
    c.label(widgets::subtle("Abra um Claude ou um terminal aqui.", &theme));
    c.horizontal(|ui| {
        if ui.button(format!("{I_PLUS} Claude ({})", app.keymap.label(Action::NewSession))).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::NewSession);
        }
        if ui.button(format!("{I_TERM} Terminal ({})", app.keymap.label(Action::NewShell))).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::NewShell);
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SideTab {
    #[default]
    Card,
    Notes,
    Tasks,
}

/// Card / Notas / Tarefas do bloco.
fn side_panel(app: &mut App, ui: &mut Ui, id: Id, r: Rect) {
    let theme = app.theme.clone();
    ui.painter().rect_filled(r, theme.radius(), theme.bg);
    ui.painter().rect_stroke(r, theme.radius(), Stroke::new(1.0, theme.border), egui::StrokeKind::Inside);
    let mut c = ui.new_child(UiBuilder::new().max_rect(r.shrink(10.0)));
    let ui = &mut c;
    let key = egui::Id::new(("block-side-tab", id));
    let mut tab: SideTab = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    // notas pedidas pelo atalho
    if app.focus_notes == Some(id) {
        tab = SideTab::Notes;
    }
    ui.horizontal(|ui| {
        for (t, label) in [(SideTab::Card, "Card"), (SideTab::Notes, "Notas"), (SideTab::Tasks, "Tarefas")] {
            if (t != SideTab::Notes || app.cfg.on(Module::Notes)) && ui.selectable_label(tab == t, label).clicked() {
                tab = t;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(key, tab));
    ui.separator();
    match tab {
        SideTab::Card => card_tab(app, ui, id, &theme),
        SideTab::Notes => notes_tab(app, ui, id),
        SideTab::Tasks => tasks_tab(app, ui, id, &theme),
    }
}

fn card_tab(app: &mut App, ui: &mut Ui, id: Id, theme: &Theme) {
    let task = app.blocks.get(&id).and_then(|b| b.task).and_then(|tid| app.tasks.iter().find(|t| t.id == tid)).cloned();
    let Some(task) = task else {
        ui.label(widgets::subtle("Sem card neste bloco.", theme));
        ui.add_space(4.0);
        if ui.button(format!("{I_TASK} Vincular tarefa/card ({})", app.keymap.label(Action::LinkTaskToSession))).clicked() {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::LinkTaskToSession);
        }
        return;
    };
    egui::ScrollArea::vertical().id_salt(("card", id)).show(ui, |ui| {
        if let Some(k) = &task.card_key {
            ui.label(RichText::new(k).font(FontId::monospace(12.0)).color(theme.accent_hi));
        }
        ui.label(RichText::new(&task.title).size(15.0).strong());
        ui.horizontal_wrapped(|ui| {
            widgets::chip(ui, task.status.label(), theme.accent, theme);
            widgets::chip(ui, task.priority.label(), theme.warn, theme);
            if let Some(r) = &task.repo {
                widgets::chip(ui, format!("#{r}"), theme.dim, theme);
            }
        });
        if let Some((a, n)) = task.subtasks_progress() {
            ui.label(widgets::subtle(format!("☑ {a}/{n} subtarefas"), theme));
        }
        if !task.notes.trim().is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&task.notes).small());
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(u) = task.notion_url.clone() {
                if ui.button(format!("{} Notion ↗", widgets::I_NOTION)).clicked() {
                    let _ = open::that_detached(u);
                }
            }
            if let Some(u) = task.slack_thread.clone() {
                if ui.button(format!("{} Thread ↗", widgets::I_SLACK)).clicked() {
                    let _ = open::that_detached(u);
                }
            }
            if ui.button("󰈙 Mandar pro Claude").on_hover_text("Escreve .aic/context.md e cita no prompt do Claude em foco").clicked() {
                let target = app.blocks.get(&id).and_then(|b| b.focused.or_else(|| b.panes.first().copied()));
                if let Some(s) = target {
                    app.write_context(s);
                }
            }
            if app.cfg.on(Module::Notion) && ui.button("↻ Buscar no Notion").on_hover_text("Atualiza o card pelo conector").clicked() {
                crate::connector::refresh_card(app, task.id);
            }
        });
        if let Some(sum) = crate::connector::summary_of(app, task.id) {
            ui.add_space(6.0);
            ui.label(RichText::new("RESUMO DO CARD").font(FontId::monospace(10.5)).color(theme.dim));
            ui.label(RichText::new(sum).small());
        }
    });
}

fn notes_tab(app: &mut App, ui: &mut Ui, id: Id) {
    let Some(mut notes) = app.blocks.get(&id).map(|b| b.notes.clone()) else { return };
    let r = ui.add(
        TextEdit::multiline(&mut notes)
            .hint_text("Anotações do bloco (Markdown). Ficam salvas com o bloco.")
            .desired_width(f32::INFINITY)
            .desired_rows(14)
            .font(FontId::monospace(12.5)),
    );
    if app.focus_notes == Some(id) {
        r.request_focus();
        app.focus_notes = None;
        app.term_focus = false;
    }
    if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        r.surrender_focus();
        app.term_focus = true;
    }
    if r.changed() {
        if let Some(b) = app.blocks.get_mut(&id) {
            b.notes = notes;
        }
    }
    if r.lost_focus() {
        app.save_block(id);
    }
}

fn tasks_tab(app: &mut App, ui: &mut Ui, id: Id, theme: &Theme) {
    let main = app.blocks.get(&id).and_then(|b| b.task);
    let panes = app.blocks.get(&id).map(|b| b.panes.clone()).unwrap_or_default();
    let mine: Vec<_> = app
        .tasks
        .iter()
        .filter(|t| !matches!(t.status, TaskStatus::Archived))
        .filter(|t| Some(t.id) == main || t.session.is_some_and(|s| panes.contains(&s)))
        .cloned()
        .collect();
    egui::ScrollArea::vertical().id_salt(("tasks", id)).max_height(ui.available_height() - 34.0).show(ui, |ui| {
        if mine.is_empty() {
            ui.label(widgets::subtle("Nenhuma tarefa ligada a este bloco.", theme));
        }
        for t in &mine {
            ui.horizontal(|ui| {
                let mut done = t.status == TaskStatus::Done;
                if ui.checkbox(&mut done, "").changed() {
                    let st = if done { TaskStatus::Done } else { TaskStatus::Doing };
                    app.update_task(t.id, |x| x.status = st);
                }
                let label = RichText::new(truncate(&t.title, 34));
                if ui.link(if done { label.strikethrough() } else { label }).clicked() {
                    app.board.selected = Some(t.id);
                    app.set_view(crate::app::View::Board);
                }
            });
            for (k, s) in t.subtasks.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    let mut d = s.done;
                    if ui.checkbox(&mut d, RichText::new(truncate(&s.text, 30)).small()).changed() {
                        let tid = t.id;
                        app.update_task(tid, |x| {
                            if let Some(st) = x.subtasks.get_mut(k) {
                                st.done = d;
                            }
                        });
                    }
                });
            }
        }
    });
    // captura rápida já ligada ao bloco
    let key = egui::Id::new(("block-quick", id));
    let mut txt: String = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    let r = ui.add(TextEdit::singleline(&mut txt).hint_text("+ tarefa neste bloco (Enter)").desired_width(f32::INFINITY));
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !txt.trim().is_empty() {
        let target = app.blocks.get(&id).and_then(|b| b.focused.or_else(|| b.panes.first().copied()));
        app.quick_add(&txt, TaskStatus::Doing, target);
        txt.clear();
    }
    ui.data_mut(|d| d.insert_temp(key, txt));
}

// ───────────────────────── Abas ─────────────────────────

pub fn tabs(app: &mut App, ui: &mut Ui) {
    let blocks = app.ws_blocks();
    if blocks.is_empty() {
        return super::sessions::empty(app, ui);
    }
    let theme = app.theme.clone();
    let t = app.t();
    let current = app.current_block().unwrap_or(blocks[0]);

    // barra de abas
    ui.horizontal(|ui| {
        for (i, b) in blocks.iter().enumerate() {
            let Some(bl) = app.blocks.get(b).cloned() else { continue };
            let sel = *b == current;
            let color = theme.agent(bl.color);
            let worst = bl.panes.iter().map(|p| app.state_of(*p)).max_by_key(|s| match s {
                SessionState::Waiting => 5,
                SessionState::Done => 4,
                SessionState::Working => 3,
                SessionState::GameOver => 2,
                SessionState::Boot => 1,
                SessionState::Idle => 0,
            });
            let text = format!("{}  {}  ·{}", if i < 9 { format!("{}", i + 1) } else { " ".into() }, truncate(&bl.name, 22), bl.panes.len());
            let galley = ui.painter().layout_no_wrap(text, FontId::proportional(13.0), if sel { theme.ink } else { theme.dim });
            let size = galley.size() + Vec2::new(34.0, 12.0);
            let (r, resp) = ui.allocate_exact_size(size, Sense::click());
            ui.painter().rect_filled(r, egui::CornerRadius { nw: theme.radius, ne: theme.radius, sw: 0, se: 0 }, if sel { theme.bg3 } else { theme.bg2 });
            if sel {
                ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(2.0, color));
            }
            if let Some(s) = worst {
                widgets::state_dot(ui, r.left_center() + Vec2::new(12.0, 0.0), 4.0, s, &theme, t);
            }
            ui.painter().galley(r.min + Vec2::new(24.0, 6.0), galley, theme.ink);
            let resp = resp.on_hover_text(format!("Alt+{} · clique direito: opções", i + 1));
            if resp.clicked() {
                app.focus_block(*b);
            }
            if resp.double_clicked() {
                app.modal = Some(super::modals::Modal::RenameBlock { id: *b, text: bl.name.clone(), first: true });
            }
            block_menu(app, &resp, *b);
        }
        if widgets::icon_button(ui, I_PLUS, "Novo bloco", &app.keymap.tip(Action::NewBlock, "Novo bloco")).clicked() {
            app.run_action(ui.ctx(), Action::NewBlock);
        }
    });

    let area = ui.available_rect_before_wrap();
    let hrect = Rect::from_min_size(area.min, Vec2::new(area.width(), HEADER));
    ui.painter().rect_filled(hrect, egui::CornerRadius::ZERO, theme.bg2);
    let idx = blocks.iter().position(|b| *b == current).unwrap_or(0);
    block_header(app, ui, current, hrect, idx, true, false);
    let body = Rect::from_min_max(Pos2::new(area.left(), hrect.bottom() + 4.0), area.max);
    block_body(app, ui, current, body);
    ui.allocate_rect(area, Sense::hover());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranjos_cobrem_a_area() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        for split in Split::ALL {
            for n in 1..=5 {
                let rs = split_rects(area, n, split, 6.0);
                assert_eq!(rs.len(), n);
                for r in &rs {
                    assert!(area.expand(0.5).contains_rect(*r), "{split:?} {n}");
                    assert!(r.width() > 50.0 && r.height() > 50.0);
                }
            }
        }
    }
}
