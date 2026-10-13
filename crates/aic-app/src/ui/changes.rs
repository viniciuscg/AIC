//! Aba Changes: os arquivos mudados de um repo e o diff de cada um, como o "Source Control" do
//! VS Code. Segue o terminal em foco (o repo da pasta dele) até você escolher outro repo.
//!
//! ```text
//!  repo ▾  branch  ↻         │  src/app.rs
//!  [mensagem do commit    ]  │  @@ -10,6 +10,8 @@
//!  ✓ Commit                  │   linha igual
//!  ▾ Staged (1)              │  -linha removida
//!    M  app.rs  src          │  +linha nova
//!  ▾ Changes (2)             │
//!    U  novo.txt             │
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aic_core::git::{self, Change};
use egui::{Align, Color32, FontId, Key, Layout, Modifiers, RichText, ScrollArea, Sense, TextEdit, Ui, Vec2};

use super::widgets::{self, I_BRANCH, I_DIFF, I_RESTART};
use crate::actions::consume_exact;
use crate::app::{App, ToastKind};
use crate::theme::Theme;

const I_STAGE: &str = "\u{f067}";
const I_UNSTAGE: &str = "\u{f068}";
const I_DISCARD: &str = "\u{f0e2}";
const I_OPEN: &str = "\u{f120}";

#[derive(Default)]
pub struct ChangesState {
    /// Repo escolhido à mão; `None` segue o terminal em foco.
    pub pinned: Option<PathBuf>,
    /// Repo mostrado agora.
    root: Option<PathBuf>,
    list: Vec<Change>,
    list_at: Option<Instant>,
    err: Option<String>,
    /// Repos dos terminais do workspace (pro seletor), lidos junto com a lista.
    sources: Vec<(String, PathBuf)>,
    /// Arquivo selecionado: caminho + se é a versão staged.
    sel: Option<(String, bool)>,
    /// Diff em cache: (arquivo, staged, linhas).
    diff: Option<(String, bool, Vec<String>)>,
    pub message: String,
    confirm_discard: Option<Change>,
}

impl ChangesState {
    fn invalidate(&mut self) {
        self.list_at = None;
        self.diff = None;
    }

    /// Linhas na ordem da tela: staged primeiro, depois o resto.
    fn rows(&self) -> Vec<(Change, bool)> {
        let staged = self.list.iter().filter(|c| c.staged()).map(|c| (c.clone(), true));
        let unstaged = self.list.iter().filter(|c| c.unstaged()).map(|c| (c.clone(), false));
        staged.chain(unstaged).collect()
    }
}

/// Repo do terminal em foco (a raiz do git da pasta dele).
fn focused_root(app: &App) -> Option<PathBuf> {
    let id = app.focused()?;
    git::toplevel(&app.sessions.get(&id)?.s.cwd)
}

fn label_of(app: &App, root: &Path) -> String {
    app.cfg.repo_of(root).unwrap_or_else(|| root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| root.display().to_string()))
}

fn refresh(app: &mut App) {
    let fresh = app.changes.list_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
    if fresh {
        return;
    }
    // repos dos terminais do workspace, sem repetir
    let mut sources: Vec<(String, PathBuf)> = Vec::new();
    for id in app.ws_sessions() {
        if let Some(root) = app.sessions.get(&id).and_then(|r| git::toplevel(&r.s.cwd)) {
            if !sources.iter().any(|(_, p)| *p == root) {
                sources.push((label_of(app, &root), root));
            }
        }
    }
    let root = app.changes.pinned.clone().or_else(|| focused_root(app)).or_else(|| sources.first().map(|(_, p)| p.clone()));
    if root != app.changes.root {
        app.changes.sel = None;
        app.changes.diff = None;
    }
    app.changes.sources = sources;
    app.changes.root = root.clone();
    app.changes.list_at = Some(Instant::now());
    let Some(root) = root else {
        app.changes.list.clear();
        return;
    };
    match git::changes(&root) {
        Ok(l) => {
            if l != app.changes.list {
                app.changes.diff = None;
            }
            app.changes.list = l;
            app.changes.err = None;
        }
        Err(e) => {
            app.changes.list.clear();
            app.changes.err = Some(format!("{e:#}"));
        }
    }
    // a seleção sumiu (commitou, descartou…): vai pro primeiro
    let rows = app.changes.rows();
    let still = app.changes.sel.as_ref().is_some_and(|(p, s)| rows.iter().any(|(c, st)| c.path == *p && st == s));
    if !still {
        app.changes.sel = rows.first().map(|(c, s)| (c.path.clone(), *s));
        app.changes.diff = None;
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    refresh(app);

    ui.horizontal(|ui| {
        widgets::heading(ui, &format!("{I_DIFF}  Changes"), &theme);
        ui.label(widgets::subtle("arquivos mudados e o diff de cada um · segue o terminal em foco", &theme).small());
    });
    ui.add_space(8.0);

    let Some(root) = app.changes.root.clone() else {
        repo_picker(app, ui, &theme, None);
        widgets::empty_state(ui, "Nenhum repo", "abra um terminal numa pasta com git ou escolha um repo acima", &theme);
        return;
    };

    keyboard(app, ui, &root);

    egui::Panel::left("changes-files").resizable(true).default_size(340.0).size_range(240.0..=700.0).show(ui, |ui| {
        files(app, ui, &theme, &root);
    });

    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin { left: 10, ..Default::default() })).show(ui, |ui| {
        diff_view(app, ui, &theme, &root);
    });

    discard_modal(app, ui, &theme, &root);
}

fn repo_picker(app: &mut App, ui: &mut Ui, theme: &Theme, root: Option<&Path>) {
    let current = root.map(|r| label_of(app, r)).unwrap_or_else(|| "escolher repo".into());
    let mut pick: Option<Option<PathBuf>> = None;
    egui::ComboBox::from_id_salt("changes-repo").selected_text(RichText::new(current).font(FontId::monospace(14.0)).color(theme.accent_hi)).width(200.0).show_ui(ui, |ui| {
        if ui.selectable_label(app.changes.pinned.is_none(), "seguir o terminal em foco").clicked() {
            pick = Some(None);
        }
        if !app.changes.sources.is_empty() {
            ui.separator();
            ui.label(widgets::subtle("terminais deste workspace", theme).small());
            for (name, p) in app.changes.sources.clone() {
                if ui.selectable_label(app.changes.pinned.as_ref() == Some(&p), &name).on_hover_text(aic_core::paths::pretty(&p)).clicked() {
                    pick = Some(Some(p));
                }
            }
        }
        ui.separator();
        ui.label(widgets::subtle("todos os repos", theme).small());
        for (name, p) in app.cfg.repo_paths() {
            if ui.selectable_label(app.changes.pinned.as_ref() == Some(&p), &name).clicked() {
                pick = Some(Some(p));
            }
        }
    });
    if let Some(p) = pick {
        app.changes.pinned = p;
        app.changes.invalidate();
    }
}

fn files(app: &mut App, ui: &mut Ui, theme: &Theme, root: &Path) {
    ui.horizontal(|ui| {
        repo_picker(app, ui, theme, Some(root));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, I_RESTART, "Atualizar changes", "Atualizar (r)").clicked() {
                app.changes.invalidate();
            }
        });
    });
    if let Some(b) = app.sessions.values().find(|r| r.s.cwd.starts_with(root)).and_then(|r| r.git.as_ref()).and_then(|g| g.branch.clone()) {
        ui.label(widgets::subtle(format!("{I_BRANCH} {b}"), theme).small());
    }
    ui.add_space(6.0);

    // commit
    let staged = app.changes.list.iter().filter(|c| c.staged()).count();
    let has_changes = !app.changes.list.is_empty();
    let edit = ui.add(TextEdit::multiline(&mut app.changes.message).hint_text("Mensagem do commit (Ctrl+Enter)").desired_rows(2).desired_width(f32::INFINITY));
    let ctrl_enter = edit.has_focus() && ui.input_mut(|i| consume_exact(i, Modifiers::COMMAND, Key::Enter));
    let label = if staged > 0 || !has_changes { format!("{} Commit", widgets::I_CHECK) } else { format!("{} Commit tudo", widgets::I_CHECK) };
    let tip = if staged > 0 { "Commita o que está em Staged" } else { "Nada em Staged: adiciona todas as mudanças e commita" };
    let ok = has_changes && !app.changes.message.trim().is_empty();
    let clicked = ui.add_enabled(ok, egui::Button::new(label).min_size(Vec2::new(ui.available_width(), 0.0))).on_hover_text(tip).clicked();
    if (clicked || ctrl_enter) && ok {
        commit(app, root, staged == 0);
    }
    ui.add_space(8.0);

    if let Some(e) = app.changes.err.clone() {
        ui.label(RichText::new(e).small().color(theme.err));
        return;
    }
    if app.changes.list.is_empty() {
        ui.label(widgets::subtle("Nada mudado.", theme));
        return;
    }

    let rows = app.changes.rows();
    let n_staged = rows.iter().filter(|(_, s)| *s).count();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (group, is_staged) in [("Staged", true), ("Changes", false)] {
            let items: Vec<&(Change, bool)> = rows.iter().filter(|(_, s)| *s == is_staged).collect();
            if items.is_empty() {
                continue;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{group} ({})", items.len())).strong().color(theme.dim));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let paths: Vec<&str> = items.iter().map(|(c, _)| c.path.as_str()).collect();
                    if is_staged {
                        if widgets::icon_button(ui, I_UNSTAGE, "Unstage tudo", "Tirar tudo de Staged").clicked() {
                            run(app, git::unstage(root, &paths));
                        }
                    } else if widgets::icon_button(ui, I_STAGE, "Stage tudo", "Mandar tudo pra Staged").clicked() {
                        run(app, git::stage(root, &paths));
                    }
                });
            });
            for (c, s) in items {
                file_row(app, ui, theme, root, c, *s);
            }
            if is_staged && n_staged > 0 {
                ui.add_space(6.0);
            }
        }
    });
}

fn status_letter(c: &Change, staged: bool) -> char {
    if c.untracked() {
        'U'
    } else if c.conflict() {
        '!'
    } else if staged {
        c.index
    } else {
        c.worktree
    }
}

fn letter_color(l: char, theme: &Theme) -> Color32 {
    match l {
        'M' => theme.warn,
        'A' | 'U' => theme.ok,
        'D' => theme.err,
        'R' | 'C' => theme.accent,
        '!' => theme.attention,
        _ => theme.dim,
    }
}

fn file_row(app: &mut App, ui: &mut Ui, theme: &Theme, root: &Path, c: &Change, staged: bool) {
    let selected = app.changes.sel.as_ref().is_some_and(|(p, s)| *p == c.path && *s == staged);
    let (dir, name) = match c.path.rsplit_once('/') {
        Some((d, n)) => (d, n),
        None => ("", c.path.as_str()),
    };
    let letter = status_letter(c, staged);
    let h = 22.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
    let hovered = resp.hovered();
    if selected {
        ui.painter().rect_filled(rect, 2.0, theme.bg3);
        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(2.0, h)), 0.0, theme.accent);
    } else if hovered {
        ui.painter().rect_filled(rect, 2.0, theme.bg3.gamma_multiply(0.6));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(8.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
    child.label(RichText::new(letter.to_string()).font(FontId::monospace(12.5)).color(letter_color(letter, theme)).strong());
    child.add_space(4.0);
    let deleted = letter == 'D';
    let mut nm = RichText::new(name).color(theme.ink);
    if deleted {
        nm = nm.strikethrough();
    }
    child.add(egui::Label::new(nm).selectable(false).truncate());
    if !dir.is_empty() {
        child.add(egui::Label::new(widgets::subtle(dir, theme).small()).selectable(false).truncate());
    }
    if hovered || selected {
        child.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if staged {
                if widgets::icon_button(ui, I_UNSTAGE, "Unstage", "Tirar de Staged (u)").clicked() {
                    run(app, git::unstage(root, &[&c.path]));
                }
            } else {
                if widgets::icon_button(ui, I_STAGE, "Stage", "Mandar pra Staged (s)").clicked() {
                    run(app, git::stage(root, &[&c.path]));
                }
                if widgets::icon_button(ui, I_DISCARD, "Descartar", "Descartar a mudança").clicked() {
                    app.changes.confirm_discard = Some(c.clone());
                }
            }
        });
    }
    let resp = resp.on_hover_text(match &c.orig {
        Some(o) => format!("{o} → {}", c.path),
        None => c.path.clone(),
    });
    if resp.clicked() {
        app.changes.sel = Some((c.path.clone(), staged));
    }
}

fn diff_view(app: &mut App, ui: &mut Ui, theme: &Theme, root: &Path) {
    let Some((path, staged)) = app.changes.sel.clone() else {
        widgets::empty_state(ui, "Nenhum arquivo selecionado", "escolha um arquivo à esquerda", theme);
        return;
    };
    let Some(change) = app.changes.list.iter().find(|c| c.path == path).cloned() else { return };
    let cached = app.changes.diff.as_ref().is_some_and(|(p, s, _)| *p == path && *s == staged);
    if !cached {
        let text = git::diff(root, &change, staged).unwrap_or_else(|e| format!("erro: {e:#}"));
        let lines = if text.trim().is_empty() { vec!["(sem diferença de texto: modo, permissão ou fim de linha)".to_string()] } else { text.lines().map(str::to_string).collect() };
        app.changes.diff = Some((path.clone(), staged, lines));
    }

    ui.horizontal(|ui| {
        ui.label(RichText::new(&path).font(FontId::monospace(14.0)).color(theme.ink));
        widgets::chip(ui, if staged { "staged" } else { "working tree" }, if staged { theme.ok } else { theme.dim }, theme);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button(format!("{I_OPEN} abrir no terminal")).on_hover_text("Abre um shell na pasta do arquivo").clicked() {
                let dir = root.join(&path).parent().map(Path::to_path_buf).unwrap_or_else(|| root.to_path_buf());
                let ctx = ui.ctx().clone();
                let name = format!("{} (shell)", label_of(app, root));
                app.create_session(&ctx, name, aic_core::SessionKind::Shell, dir, None, None, vec![]);
                app.set_view(crate::app::View::Sessions);
            }
        });
    });
    ui.add_space(4.0);

    let lines = app.changes.diff.as_ref().map(|(_, _, l)| l.clone()).unwrap_or_default();
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, theme.radius(), theme.term.background);
    let font = FontId::monospace(app.cfg.font_size.clamp(10.0, 20.0) - 1.0);
    let row_h = ui.fonts_mut(|f| f.row_height(&font)) + 2.0;
    let add_bg = theme.ok.gamma_multiply(0.14);
    let del_bg = theme.err.gamma_multiply(0.14);
    ScrollArea::both().auto_shrink([false, false]).show_rows(ui, row_h, lines.len(), |ui, range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for l in &lines[range] {
            let (color, bg) = if l.starts_with("+++") || l.starts_with("---") || l.starts_with("diff --git") || l.starts_with("index ") {
                (theme.faint, None)
            } else if l.starts_with('+') {
                (theme.ok, Some(add_bg))
            } else if l.starts_with('-') {
                (theme.err, Some(del_bg))
            } else if l.starts_with("@@") {
                (theme.accent, None)
            } else {
                (theme.ink, None)
            };
            let text = l.replace('\t', "    ");
            let galley = ui.painter().layout_no_wrap(text, font.clone(), color);
            let w = (galley.size().x + 16.0).max(ui.available_width());
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, row_h), Sense::hover());
            if let Some(bg) = bg {
                ui.painter().rect_filled(r, 0.0, bg);
            }
            ui.painter().galley(r.min + Vec2::new(8.0, 1.0), galley, color);
        }
    });
}

fn keyboard(app: &mut App, ui: &mut Ui, root: &Path) {
    if app.modal.is_some() || app.changes.confirm_discard.is_some() || ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let rows = app.changes.rows();
    let key = |k: Key| ui.input_mut(|i| consume_exact(i, Modifiers::NONE, k));
    let cur = app.changes.sel.as_ref().and_then(|(p, s)| rows.iter().position(|(c, st)| c.path == *p && st == s));
    if !rows.is_empty() {
        let n = rows.len();
        let mv = if key(Key::J) || key(Key::ArrowDown) {
            Some(cur.map_or(0, |i| (i + 1) % n))
        } else if key(Key::K) || key(Key::ArrowUp) {
            Some(cur.map_or(0, |i| (i + n - 1) % n))
        } else {
            None
        };
        if let Some(i) = mv {
            app.changes.sel = Some((rows[i].0.path.clone(), rows[i].1));
        }
    }
    if let Some((c, staged)) = cur.map(|i| rows[i].clone()) {
        if !staged && key(Key::S) {
            run(app, git::stage(root, &[&c.path]));
        } else if staged && key(Key::U) {
            run(app, git::unstage(root, &[&c.path]));
        }
    }
    if key(Key::R) {
        app.changes.invalidate();
    }
}

fn run(app: &mut App, r: anyhow::Result<()>) {
    if let Err(e) = r {
        app.toast(&format!("{e:#}"), ToastKind::Err);
    }
    app.changes.invalidate();
}

fn commit(app: &mut App, root: &Path, all: bool) {
    if all {
        let paths: Vec<String> = app.changes.list.iter().map(|c| c.path.clone()).collect();
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        if let Err(e) = git::stage(root, &refs) {
            app.toast(&format!("{e:#}"), ToastKind::Err);
            return app.changes.invalidate();
        }
    }
    match git::commit(root, app.changes.message.trim()) {
        Ok(out) => {
            let first = out.lines().next().unwrap_or("commit feito").to_string();
            app.toast(&first, ToastKind::Ok);
            app.changes.message.clear();
        }
        Err(e) => app.toast(&format!("{e:#}"), ToastKind::Err),
    }
    app.changes.invalidate();
}

fn discard_modal(app: &mut App, ui: &mut Ui, theme: &Theme, root: &Path) {
    let Some(c) = app.changes.confirm_discard.clone() else { return };
    let mut close = false;
    egui::Modal::new(egui::Id::new("changes-discard")).show(ui.ctx(), |ui| {
        let what = if c.untracked() { "Apagar o arquivo novo" } else { "Descartar as mudanças de" };
        ui.label(RichText::new(format!("{what} {}?", c.path)).size(15.0));
        ui.label(widgets::subtle("Não dá pra desfazer. O que está em Staged continua lá.", theme));
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Descartar").color(theme.err)).clicked() {
                run(app, git::discard(root, &c));
                close = true;
            }
            if ui.button("Cancelar").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                close = true;
            }
        });
    });
    if close {
        app.changes.confirm_discard = None;
    }
}
