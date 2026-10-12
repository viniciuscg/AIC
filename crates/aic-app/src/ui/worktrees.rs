//! Aba Worktrees: worktrees de cada repo, quem está usando, criar/abrir/remover.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{Align, FontId, Key, Layout, Modifiers, RichText, ScrollArea, TextEdit, Ui};
use aic_core::git::{self, GitStatus, Worktree};
use aic_core::{Id, SessionKind};

use super::widgets::{self, I_BRANCH, I_TERM, I_WORKTREE};
use crate::actions::consume_exact;
use crate::app::{App, ToastKind};

#[derive(Default)]
pub struct WorktreeState {
    pub repo: Option<String>,
    pub filter: String,
    pub new_branch: String,
    pub creating: bool,
    pub selected: usize,
    cache: HashMap<String, (Instant, Vec<(Worktree, Option<GitStatus>)>)>,
    confirm_remove: Option<PathBuf>,
}

impl WorktreeState {
    fn list(&mut self, repo: &str, path: &std::path::Path) -> Vec<(Worktree, Option<GitStatus>)> {
        let fresh = self.cache.get(repo).is_some_and(|(t, _)| t.elapsed() < Duration::from_secs(10));
        if !fresh {
            let list = git::worktrees(path)
                .unwrap_or_default()
                .into_iter()
                .map(|w| {
                    let st = git::status(&w.path);
                    (w, st)
                })
                .collect();
            self.cache.insert(repo.to_string(), (Instant::now(), list));
        }
        self.cache.get(repo).map(|(_, l)| l.clone()).unwrap_or_default()
    }

    fn invalidate(&mut self, repo: &str) {
        self.cache.remove(repo);
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    ui.horizontal(|ui| {
        widgets::heading(ui, &format!("{I_WORKTREE}  Worktrees"), &theme);
        ui.label(widgets::subtle("cada sessão pode trabalhar isolada num worktree: <repo>.worktrees/<branch> · remover nunca apaga a branch", &theme).small());
    });
    ui.add_space(8.0);

    egui::Panel::left("wt-repos").resizable(true).default_size(240.0).show(ui, |ui| {
        ui.add(TextEdit::singleline(&mut app.worktrees.filter).hint_text("filtrar repos").desired_width(f32::INFINITY));
        ScrollArea::vertical().show(ui, |ui| {
            let f = app.worktrees.filter.to_lowercase();
            for r in app.repos.clone() {
                if !f.is_empty() && !r.to_lowercase().contains(&f) {
                    continue;
                }
                let in_use = app.sessions.values().filter(|s| s.s.repo.as_deref() == Some(&r)).count();
                let label = if in_use > 0 { format!("{r}  · {in_use}") } else { r.clone() };
                if ui.selectable_label(app.worktrees.repo.as_deref() == Some(&r), label).clicked() {
                    app.worktrees.repo = Some(r.clone());
                    app.worktrees.selected = 0;
                }
            }
        });
    });

    let Some(repo) = app.worktrees.repo.clone().or_else(|| app.repos.first().cloned()) else {
        widgets::empty_state(ui, "Nenhum repo encontrado", &format!("procurei em {}", app.cfg.repos_dirs.iter().map(|p| aic_core::paths::pretty(p)).collect::<Vec<_>>().join(", ")), &theme);
        return;
    };
    app.worktrees.repo = Some(repo.clone());
    let Some(path) = app.cfg.repo_path(&repo) else { return };
    let list = app.worktrees.list(&repo, &path);

    ui.horizontal(|ui| {
        ui.label(RichText::new(&repo).font(FontId::monospace(16.0)).color(theme.accent_hi));
        ui.label(widgets::subtle(format!("branch padrão: {}", git::default_branch(&path)), &theme).small());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Prune").on_hover_text("git worktree prune (limpa registros de pastas que sumiram)").clicked() {
                match git::prune(&path) {
                    Ok(()) => app.toast("Prune feito", ToastKind::Ok),
                    Err(e) => app.toast(&format!("{e}"), ToastKind::Err),
                }
                app.worktrees.invalidate(&repo);
            }
            let r = ui.button("↻");
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Atualizar worktrees"));
            if r.on_hover_text("Atualizar (r)").clicked() {
                app.worktrees.invalidate(&repo);
            }
        });
    });
    ui.add_space(6.0);

    // criar
    ui.horizontal(|ui| {
        let r = ui.add(TextEdit::singleline(&mut app.worktrees.new_branch).hint_text("nova branch (ex: DEV-142-ajuste-juros)").desired_width(320.0));
        if app.worktrees.creating {
            r.request_focus();
            app.worktrees.creating = false;
        }
        let go = ui.button(format!("{}  Criar worktree + sessão Claude", widgets::I_PLUS)).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
        if go && !app.worktrees.new_branch.trim().is_empty() {
            let branch = app.worktrees.new_branch.trim().to_string();
            match git::add_worktree(&path, &branch, None) {
                Ok(p) => {
                    app.worktrees.new_branch.clear();
                    app.worktrees.invalidate(&repo);
                    let ctx = ui.ctx().clone();
                    app.create_session(&ctx, branch, SessionKind::Claude, p.clone(), Some(p), None, vec![]);
                }
                Err(e) => app.toast(&format!("Worktree falhou: {e}"), ToastKind::Err),
            }
        }
    });
    ui.add_space(8.0);

    // teclado: j/k, Enter abre, Delete remove, n nova
    if app.modal.is_none() && !ui.ctx().egui_wants_keyboard_input() {
        let n = list.len().max(1);
        let key = |k: Key| ui.input_mut(|i| consume_exact(i, Modifiers::NONE, k));
        if key(Key::J) || key(Key::ArrowDown) {
            app.worktrees.selected = (app.worktrees.selected + 1) % n;
        }
        if key(Key::K) || key(Key::ArrowUp) {
            app.worktrees.selected = (app.worktrees.selected + n - 1) % n;
        }
        if key(Key::N) {
            app.worktrees.creating = true;
        }
        if key(Key::R) {
            app.worktrees.invalidate(&repo);
        }
    }
    let enter = app.modal.is_none() && !ui.ctx().egui_wants_keyboard_input() && ui.input_mut(|i| consume_exact(i, Modifiers::NONE, Key::Enter));
    let del = app.modal.is_none() && !ui.ctx().egui_wants_keyboard_input() && ui.input_mut(|i| consume_exact(i, Modifiers::NONE, Key::Delete));

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, (wt, st)) in list.iter().enumerate() {
            let users: Vec<Id> = app.sessions.values().filter(|s| s.s.cwd.starts_with(&wt.path)).map(|s| s.s.id).collect();
            let sel = i == app.worktrees.selected;
            let accent = if wt.is_main { theme.dim } else { theme.accent };
            let resp = widgets::card(ui, &theme, sel, accent, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let b = wt.branch.clone().unwrap_or_else(|| format!("(detached {})", wt.head));
                    ui.label(RichText::new(format!("{I_BRANCH} {b}")).strong().color(theme.ink));
                    if wt.is_main {
                        widgets::chip(ui, "principal", theme.dim, &theme);
                    }
                    if wt.locked {
                        widgets::chip(ui, "travado", theme.warn, &theme);
                    }
                    if wt.prunable {
                        widgets::chip(ui, "pasta sumiu", theme.err, &theme);
                    }
                    if let Some(s) = st {
                        if s.dirty > 0 {
                            widgets::chip(ui, format!("±{}", s.dirty), theme.warn, &theme);
                        }
                        if s.ahead > 0 {
                            widgets::chip(ui, format!("↑{}", s.ahead), theme.ok, &theme);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !wt.is_main && widgets::icon_button(ui, "🗑", "Remover worktree", "Remover worktree (Delete)").clicked() {
                            app.worktrees.confirm_remove = Some(wt.path.clone());
                        }
                        if ui.small_button(format!("{I_TERM} sessão")).on_hover_text("Abrir sessão Claude aqui (Enter)").clicked() {
                            open_here(app, ui, wt);
                        }
                    });
                });
                ui.label(widgets::subtle(aic_core::paths::pretty(&wt.path), &theme).small());
                for u in &users {
                    let state = app.state_of(*u);
                    ui.horizontal(|ui| {
                        widgets::state_badge(ui, crate::brand::state_label(state), state, &theme, app.t());
                        if ui.link(app.session_label(*u)).clicked() {
                            app.focus(*u);
                        }
                    });
                }
            });
            if resp.clicked() {
                app.worktrees.selected = i;
            }
            if sel && enter {
                open_here(app, ui, wt);
            }
            if sel && del && !wt.is_main {
                app.worktrees.confirm_remove = Some(wt.path.clone());
            }
            ui.add_space(6.0);
        }
    });

    if let Some(p) = app.worktrees.confirm_remove.clone() {
        let mut close = false;
        egui::Modal::new(egui::Id::new("rm-wt")).show(ui.ctx(), |ui| {
            ui.label(RichText::new(format!("Remover o worktree {}?", aic_core::paths::pretty(&p))).size(15.0));
            ui.label(widgets::subtle("A branch continua existindo. Mudanças não commitadas impedem a remoção (a menos que force).", &theme));
            ui.horizontal(|ui| {
                for (label, force) in [("Remover", false), ("Forçar (descarta mudanças)", true)] {
                    if ui.button(label).clicked() {
                        match git::remove_worktree(&path, &p, force) {
                            Ok(()) => app.toast("Worktree removido", ToastKind::Ok),
                            Err(e) => app.toast(&format!("{e}"), ToastKind::Err),
                        }
                        app.worktrees.invalidate(&repo);
                        close = true;
                    }
                }
                if ui.button("Cancelar").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            });
        });
        if close {
            app.worktrees.confirm_remove = None;
        }
    }
}

fn open_here(app: &mut App, ui: &Ui, wt: &Worktree) {
    if let Some(s) = app.sessions.values().find(|s| s.s.cwd == wt.path).map(|s| s.s.id) {
        return app.focus(s);
    }
    let name = wt.branch.clone().unwrap_or_else(|| "worktree".into());
    let ctx = ui.ctx().clone();
    let worktree = (!wt.is_main).then(|| wt.path.clone());
    app.create_session(&ctx, name, SessionKind::Claude, wt.path.clone(), worktree, None, vec![]);
}
