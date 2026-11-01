//! Assistente de primeiro uso: módulos → pastas e atalhos.
//! Só aparece enquanto `setup_done = false` (ou se você mandar rodar de novo em Config).
//! Tudo no teclado: ←/→ escolhe, Enter avança, Esc volta.

use aic_core::modules::{self, ALL as MODULES};
use egui::{Align2, FontId, Key, Pos2, Rect, RichText, Stroke, Ui, UiBuilder, Vec2};

use crate::app::{App, Phase};
use crate::theme::Theme;

#[derive(Debug, Clone, Default)]
pub struct SetupState {
    pub step: usize,
    pub repos_text: Option<String>,
}

const STEPS: [&str; 2] = ["Módulos", "Pastas e atalhos"];

pub fn show(app: &mut App, ui: &mut Ui) {
    let Phase::Setup(mut st) = std::mem::replace(&mut app.phase, Phase::Main) else {
        return;
    };
    let theme = app.theme.clone();
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, theme.bg);

    let typing = ui.ctx().egui_wants_keyboard_input();
    let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));

    let top = rect.top() + 36.0;
    ui.painter().text(
        Pos2::new(rect.center().x, top),
        Align2::CENTER_TOP,
        "This is where the fun begins",
        FontId::monospace(30.0),
        theme.accent_hi,
    );
    ui.painter().text(
        Pos2::new(rect.center().x, top + 42.0),
        Align2::CENTER_TOP,
        "Vamos deixar do seu jeito. Dá pra mudar tudo depois em Config (Ctrl+,).",
        FontId::proportional(14.0),
        theme.dim,
    );
    // trilha de passos
    let w = 160.0;
    let x0 = rect.center().x - w * STEPS.len() as f32 / 2.0;
    for (i, name) in STEPS.iter().enumerate() {
        let c = Pos2::new(x0 + w * i as f32 + w / 2.0, top + 84.0);
        let done = i < st.step;
        let cur = i == st.step;
        let col = if cur {
            theme.accent_hi
        } else if done {
            theme.ok
        } else {
            theme.faint
        };
        ui.painter()
            .circle_filled(c, 11.0, if cur || done { col } else { theme.bg3 });
        ui.painter().circle_stroke(c, 11.0, Stroke::new(1.5, col));
        ui.painter().text(
            c,
            Align2::CENTER_CENTER,
            if done {
                "✓".to_string()
            } else {
                (i + 1).to_string()
            },
            FontId::monospace(11.0),
            if cur || done { theme.bg } else { theme.dim },
        );
        ui.painter().text(
            c + Vec2::new(0.0, 18.0),
            Align2::CENTER_TOP,
            *name,
            FontId::proportional(12.0),
            col,
        );
        if i + 1 < STEPS.len() {
            ui.painter().line_segment(
                [c + Vec2::new(14.0, 0.0), c + Vec2::new(w - 14.0, 0.0)],
                Stroke::new(1.0, if done { theme.ok } else { theme.border }),
            );
        }
    }

    let body = Rect::from_min_max(
        Pos2::new(rect.center().x - 520.0, top + 130.0),
        Pos2::new(rect.center().x + 520.0, rect.bottom() - 70.0),
    );
    let mut child = ui.new_child(UiBuilder::new().max_rect(body));
    match st.step {
        0 => step_modules(app, &mut child, &theme),
        _ => step_paths(app, &mut child, &mut st, &theme),
    }

    // rodapé
    let foot = Rect::from_min_max(
        Pos2::new(body.left(), rect.bottom() - 60.0),
        Pos2::new(body.right(), rect.bottom() - 20.0),
    );
    let mut f = ui.new_child(
        UiBuilder::new()
            .max_rect(foot)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let mut back = esc && st.step > 0;
    let mut next = enter && !typing;
    if st.step > 0 && f.button("← Voltar (Esc)").clicked() {
        back = true;
    }
    f.with_layout(egui::Layout::right_to_left(egui::Align::Center), |f| {
        let label = if st.step + 1 == STEPS.len() {
            "Concluir  ⏎"
        } else {
            "Próximo  ⏎"
        };
        if f.add(egui::Button::new(RichText::new(label).strong()).fill(theme.accent_lo))
            .clicked()
        {
            next = true;
        }
        if f.button("Pular e usar o padrão").clicked() {
            st.step = STEPS.len();
        }
        f.label(RichText::new("Enter avança").small().color(theme.dim));
    });
    if back {
        st.step -= 1;
    } else if next {
        st.step += 1;
    }

    if st.step >= STEPS.len() {
        if let Some(txt) = st.repos_text.take() {
            apply_repos(app, &txt);
        }
        app.cfg.setup_done = true;
        let _ = app.cfg.save();
        let _ = app.keymap.save();
        let ctx = ui.ctx().clone();
        app.enter_main(&ctx);
    } else {
        app.phase = Phase::Setup(st);
    }
}

fn step_modules(app: &mut App, ui: &mut Ui, theme: &Theme) {
    ui.label(RichText::new("O que você quer ligado?").size(18.0));
    ui.label(RichText::new("Tudo vem ligado. Desligar some com a aba, os atalhos e as ações do módulo. Muda depois em Config → Módulos.").color(theme.dim));
    ui.add_space(10.0);
    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - 10.0)
        .show(ui, |ui| {
            let mut group = "";
            egui::Grid::new("setup-mods")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for m in MODULES {
                        if m.group != group {
                            group = m.group;
                            ui.label(
                                RichText::new(group.to_uppercase())
                                    .font(FontId::monospace(11.0))
                                    .color(theme.accent),
                            );
                            ui.end_row();
                        }
                        let mut on = modules::enabled(m.module, &app.cfg.modules);
                        if ui
                            .checkbox(&mut on, RichText::new(m.name).strong())
                            .changed()
                        {
                            app.cfg.modules.insert(m.id.to_string(), on);
                        }
                        ui.label(RichText::new(m.desc).color(theme.dim));
                        ui.end_row();
                    }
                });
        });
}

fn step_paths(app: &mut App, ui: &mut Ui, st: &mut SetupState, theme: &Theme) {
    ui.label(RichText::new("Onde ficam seus projetos?").size(18.0));
    ui.label(RichText::new("Uma pasta por linha. Ele procura repos git dentro delas (pra abrir sessões e worktrees).").color(theme.dim));
    ui.add_space(6.0);
    let txt = st.repos_text.get_or_insert_with(|| {
        app.cfg
            .repos_dirs
            .iter()
            .map(|p| aic_core::paths::pretty(p))
            .collect::<Vec<_>>()
            .join("\n")
    });
    ui.add(
        egui::TextEdit::multiline(txt)
            .desired_rows(4)
            .desired_width(f32::INFINITY)
            .font(FontId::monospace(13.0)),
    );
    let found = {
        let mut c = app.cfg.clone();
        c.repos_dirs = parse_dirs(txt);
        c.repos()
    };
    ui.label(
        RichText::new(format!(
            "{} repos encontrados: {}",
            found.len(),
            found.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
        ))
        .small()
        .color(theme.dim),
    );
    ui.add_space(16.0);
    ui.label(RichText::new("Prefixo (o jeito tmux)").size(18.0));
    ui.label(RichText::new("Aperte o prefixo, solte e aperte a tecla: c abre um Claude, x fecha o terminal em foco, t abre as tarefas. Ctrl+Tab anda entre sessões, tarefas e tokens.").color(theme.dim));
    ui.horizontal(|ui| {
        for opt in ["Ctrl+B", "Ctrl+A", "Ctrl+Space"] {
            if ui.selectable_label(app.cfg.prefix == opt, opt).clicked() {
                if app.cfg.leader == opt {
                    app.cfg.leader = if opt == "Ctrl+Space" {
                        "Alt+Space".into()
                    } else {
                        "Ctrl+Space".into()
                    };
                }
                app.cfg.prefix = opt.to_string();
            }
        }
    });
    ui.label(
        RichText::new(
            "Pra mandar o próprio Ctrl+B pro terminal, aperte duas vezes (Ctrl+B Ctrl+B).",
        )
        .small()
        .color(theme.faint),
    );
    ui.add_space(12.0);
    ui.label(RichText::new("Catálogo de atalhos (tecla líder)").size(18.0));
    ui.label(
        RichText::new(
            "Aperte e aparece um menu com todos os atalhos, agrupados, pra ir descobrindo.",
        )
        .color(theme.dim),
    );
    ui.horizontal(|ui| {
        for opt in ["Ctrl+Space", "Alt+Space", "Ctrl+A"] {
            if opt != app.cfg.prefix && ui.selectable_label(app.cfg.leader == opt, opt).clicked() {
                app.cfg.leader = opt.to_string();
            }
        }
    });
}

fn parse_dirs(txt: &str) -> Vec<std::path::PathBuf> {
    txt.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| match l.strip_prefix("~/") {
            Some(rest) => aic_core::paths::home_dir().join(rest),
            None if l == "~" => aic_core::paths::home_dir(),
            None => l.into(),
        })
        .collect()
}

fn apply_repos(app: &mut App, txt: &str) {
    let dirs = parse_dirs(txt);
    if !dirs.is_empty() {
        app.cfg.repos_dirs = dirs;
        app.repos = app.cfg.repos();
    }
}
