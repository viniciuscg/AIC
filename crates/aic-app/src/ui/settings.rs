//! Aba Config: geral, som, integrações, cache e o catálogo de atalhos (editável).

use egui::{FontId, Key, KeyboardShortcut, RichText, ScrollArea, TextEdit, Ui};

use super::widgets;
use crate::actions::{self, Action, CONTEXT_KEYS};
use crate::app::{App, ToastKind};
use crate::skins;

#[derive(Default)]
pub struct SettingsState {
    /// Ação esperando a próxima combinação de teclas.
    pub capturing: Option<Action>,
    pub filter: String,
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    capture(app, ui);
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(980.0);
        widgets::heading(ui, &format!("{}  Configurações", widgets::I_GEAR), &theme);
        ui.label(widgets::subtle(format!("salvas em {}", aic_core::paths::pretty(&aic_core::Config::path())), &theme).small());
        ui.add_space(10.0);

        section(ui, &theme, "Geral");
        egui::Grid::new("geral").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Fork");
            ui.horizontal(|ui| {
                for f in skins::all() {
                    if ui.selectable_label(app.skin.id() == f.id(), f.display_name()).clicked() {
                        let ctx = ui.ctx().clone();
                        app.set_theme(&ctx, f.id().to_string(), None);
                    }
                }
            });
            ui.end_row();
            ui.label("Variante");
            ui.horizontal(|ui| {
                for v in app.skin.variants() {
                    if ui.selectable_label(app.theme.variant == *v, *v).clicked() {
                        let ctx = ui.ctx().clone();
                        app.set_theme(&ctx, app.skin.id().to_string(), Some(v.to_string()));
                    }
                }
                ui.label(widgets::subtle(app.keymap.label(Action::CycleThemeVariant), &theme).small());
            });
            ui.end_row();
            ui.label("Visualização");
            ui.horizontal(|ui| {
                for v in aic_core::SessionView::ALL {
                    if ui.selectable_label(app.cfg.view == v, v.label()).on_hover_text(v.desc()).clicked() {
                        app.cfg.view = v;
                        let _ = app.cfg.save();
                    }
                }
                ui.label(widgets::subtle(app.keymap.label(Action::CycleView), &theme).small());
            });
            ui.end_row();
            ui.label("Assistente");
            if ui.button("Rodar o assistente de primeiro uso de novo").clicked() {
                app.phase = crate::app::Phase::Setup(Default::default());
            }
            ui.end_row();
            ui.label("Fonte do terminal");
            ui.horizontal(|ui| {
                let r = ui.add(TextEdit::singleline(&mut app.cfg.font_family).desired_width(260.0));
                if r.lost_focus() {
                    app.reload_fonts();
                    let _ = app.cfg.save();
                }
                let mut size = app.cfg.font_size;
                if ui.add(egui::DragValue::new(&mut size).range(8.0..=32.0).speed(0.2).suffix(" px")).changed() {
                    app.set_font_size(size);
                }
            });
            ui.end_row();
            ui.label("Escala da interface");
            let mut scale = app.cfg.ui_scale;
            if ui.add(egui::Slider::new(&mut scale, 0.8..=1.6).step_by(0.05)).drag_stopped() {
                app.cfg.ui_scale = scale;
                ui.ctx().set_zoom_factor(scale);
                let _ = app.cfg.save();
            } else {
                app.cfg.ui_scale = scale;
            }
            ui.end_row();
            ui.label("Animações");
            if ui.checkbox(&mut app.cfg.animations, "mascote animado e efeitos de fundo").changed() {
                let _ = app.cfg.save();
            }
            ui.end_row();
            ui.label("Pastas de projetos");
            ui.vertical(|ui| {
                let mut s = app.cfg.repos_dirs.iter().map(|p| aic_core::paths::pretty(p)).collect::<Vec<_>>().join("\n");
                if ui.add(TextEdit::multiline(&mut s).desired_rows(3).desired_width(360.0).hint_text("uma pasta por linha")).lost_focus() {
                    app.cfg.repos_dirs = s
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(|l| l.strip_prefix("~/").map(|r| aic_core::paths::home_dir().join(r)).unwrap_or_else(|| l.into()))
                        .collect();
                    app.repos = app.cfg.repos();
                    let _ = app.cfg.save();
                }
                ui.label(widgets::subtle(format!("{} repos encontrados", app.repos.len()), &theme).small());
            });
            ui.end_row();
        });

        section(ui, &theme, "Módulos");
        modules_panel(app, ui);

        section(ui, &theme, "Som e notificações");
        egui::Grid::new("som").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Som");
            ui.horizontal(|ui| {
                ui.checkbox(&mut app.cfg.sound.enabled, "tocar som");
                ui.add(egui::Slider::new(&mut app.cfg.sound.volume, 0.0..=1.5).text("volume"));
            });
            ui.end_row();
            let default_done = crate::sounds::default_for(&app.cfg.theme);
            ui.label("Quando terminar");
            sound_picker(app, ui, "som-done", false, default_done);
            ui.end_row();
            ui.label("Quando precisar de você");
            sound_picker(app, ui, "som-attn", true, default_done);
            ui.end_row();
            ui.label("Notificação");
            ui.vertical(|ui| {
                ui.checkbox(&mut app.cfg.notify.desktop, "notificação do sistema (notify-send)");
                ui.checkbox(&mut app.cfg.notify.only_when_unfocused, "só se a janela estiver sem foco ou a sessão escondida");
                if ui.button("testar notificação").clicked() {
                    crate::notify::desktop(&format!("{} — teste", app.skin.name()), "Se você está vendo isso, funciona. MUAHAHA!", crate::app::icon_path());
                }
            });
            ui.end_row();
        });
        if ui.button("Salvar").clicked() {
            match app.cfg.save() {
                Ok(()) => app.toast("Configurações salvas", ToastKind::Ok),
                Err(e) => app.toast(&format!("{e}"), ToastKind::Err),
            }
        }

        section(ui, &theme, "Claude Code");
        egui::Grid::new("claude").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Sua statusline");
            let mut s = app.cfg.user_statusline.clone().unwrap_or_default();
            if ui.add(TextEdit::singleline(&mut s).hint_text("~/.claude/statusline.sh").desired_width(360.0)).lost_focus() {
                app.cfg.user_statusline = (!s.trim().is_empty()).then(|| s.trim().to_string());
                let _ = app.cfg.save();
            }
            ui.end_row();
            ui.label("Injeção por sessão");
            ui.vertical(|ui| {
                match &app.inj.settings {
                    Some(p) => ui.label(widgets::subtle(format!("--settings {}", aic_core::paths::pretty(p)), &theme).small()),
                    None => ui.label(RichText::new("settings não gerado").color(theme.err)),
                };
                match &app.inj.plugin_dir {
                    Some(p) => ui.label(widgets::subtle(format!("--plugin-dir {}", aic_core::paths::pretty(p)), &theme).small()),
                    None => ui.label(RichText::new("plugin não encontrado (rode ./install.sh do AIC)").color(theme.warn)),
                };
                ui.label(widgets::subtle("hooks e statusline só valem nas sessões abertas pelo AIC; seu ~/.claude não é alterado.", &theme).small());
            });
            ui.end_row();
        });

        section(ui, &theme, "Integrações");
        crate::notion_ui::settings(app, ui);

        section(ui, &theme, "Cache (estilo tmux)");
        ui.label(widgets::subtle("Workspaces, sessões e layout são salvos continuamente. Ao fechar, você escolhe se mantém pro próximo início.", &theme));
        if ui.button("Limpar cache agora (as sessões abertas continuam até fechar)").clicked() {
            let _ = app.store.clear_cache();
            let _ = app.store.mark_cache(true);
            app.persist_ws();
            app.toast("Cache limpo", ToastKind::Ok);
        }

        section(ui, &theme, "Atalhos");
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut app.settings.filter).hint_text("filtrar atalhos").desired_width(240.0));
            if ui.button("Restaurar padrões").clicked() {
                app.keymap.reset();
                let _ = app.keymap.save();
            }
            ui.label(widgets::subtle(format!("editáveis · salvos em {}", aic_core::paths::pretty(&actions::Keymap::path())), &theme).small());
        });
        ui.add_space(6.0);
        let f = app.settings.filter.to_lowercase();
        let mut group = "";
        egui::Grid::new("binds").num_columns(3).striped(true).spacing([20.0, 6.0]).show(ui, |ui| {
            for a in Action::all() {
                let m = a.meta();
                if !f.is_empty() && !m.name.to_lowercase().contains(&f) && !m.group.to_lowercase().contains(&f) {
                    continue;
                }
                if m.group != group {
                    group = m.group;
                    ui.label(RichText::new(group.to_uppercase()).font(FontId::monospace(11.0)).color(theme.accent));
                    ui.end_row();
                }
                ui.label(m.name);
                if app.settings.capturing == Some(a) {
                    ui.label(RichText::new("aperte a combinação… (Esc cancela, Backspace limpa)").color(theme.warn));
                } else {
                    widgets::kbd(ui, &app.keymap.label(a), &theme);
                }
                ui.horizontal(|ui| {
                    if ui.small_button("alterar").clicked() {
                        app.settings.capturing = Some(a);
                    }
                    if app.keymap.get(a) != m.default && ui.small_button("padrão").clicked() {
                        app.keymap.set(a, m.default);
                        let _ = app.keymap.save();
                    }
                });
                ui.end_row();
            }
            let mut ctx_group = "";
            for (g, keys, desc) in CONTEXT_KEYS {
                if !f.is_empty() && !desc.to_lowercase().contains(&f) && !g.to_lowercase().contains(&f) {
                    continue;
                }
                if *g != ctx_group {
                    ctx_group = g;
                    ui.label(RichText::new(format!("{} (contexto)", g.to_uppercase())).font(FontId::monospace(11.0)).color(theme.accent));
                    ui.end_row();
                }
                ui.label(*desc);
                widgets::kbd(ui, keys, &theme);
                ui.label("");
                ui.end_row();
            }
        });

        section(ui, &theme, "Sobre");
        ui.label(format!("AIC {} · fork {} · variante {}", env!("CARGO_PKG_VERSION"), app.skin.display_name(), app.theme.variant));
        for (label, p) in [
            ("config", aic_core::paths::config_dir()),
            ("dados", aic_core::paths::data_dir()),
            ("eventos/statusline", aic_core::paths::state_dir()),
        ] {
            ui.label(widgets::subtle(format!("{label}: {}", aic_core::paths::pretty(&p)), &theme).small());
        }
    });
}

/// Feature flags: coluna global + coluna do tema atual (herda / liga / desliga).
fn modules_panel(app: &mut App, ui: &mut Ui) {
    use aic_core::modules::{self, ALL};
    let theme = app.theme.clone();
    let tname = app.skin.name().to_string();
    ui.label(widgets::subtle(format!("Global vale pra todos os temas; a coluna \"{tname}\" sobrescreve só neste tema. Desligar some com a aba, os atalhos e as ações."), &theme).small());
    ui.add_space(4.0);
    let mut group = "";
    let mut changed = false;
    egui::Grid::new("mods").num_columns(4).striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
        ui.label("");
        ui.label(RichText::new("Global").strong());
        ui.label(RichText::new(&tname).strong());
        ui.label("");
        ui.end_row();
        for m in ALL {
            if m.group != group {
                group = m.group;
                ui.label(RichText::new(group.to_uppercase()).font(FontId::monospace(11.0)).color(theme.accent));
                ui.end_row();
            }
            ui.label(RichText::new(m.name)).on_hover_text(m.desc);
            let mut g = modules::enabled(m.module, &app.cfg.modules, None);
            if ui.checkbox(&mut g, "").changed() {
                app.cfg.modules.insert(m.id.to_string(), g);
                changed = true;
            }
            let cur = app.cfg.themes.get(&app.cfg.theme).and_then(|t| t.modules.get(m.id).copied());
            let label = match cur {
                None => "herda",
                Some(true) => "ligado",
                Some(false) => "desligado",
            };
            if ui.button(label).on_hover_text("clique pra alternar: herda → ligado → desligado").clicked() {
                let next = match cur {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                let tc = app.cfg.theme_cfg();
                match next {
                    Some(v) => {
                        tc.modules.insert(m.id.to_string(), v);
                    }
                    None => {
                        tc.modules.remove(m.id);
                    }
                }
                changed = true;
            }
            let on = app.cfg.on(m.module);
            ui.label(RichText::new(if on { "● ativo" } else { "○ desligado" }).small().color(if on { theme.ok } else { theme.faint }));
            ui.end_row();
        }
    });
    if changed {
        let _ = app.cfg.save();
    }
}

fn section(ui: &mut Ui, theme: &crate::theme::Theme, title: &str) {
    ui.add_space(16.0);
    ui.label(RichText::new(title.to_uppercase()).font(FontId::monospace(13.0)).color(theme.accent_hi).strong());
    ui.separator();
}

/// Lista de sons com ▶ pra ouvir cada um. `attention` = campo "precisa de você" (aceita "nenhum").
fn sound_picker(app: &mut App, ui: &mut Ui, id: &str, attention: bool, default_done: &str) {
    let current = if attention { app.cfg.sound.attention.clone() } else { app.cfg.sound.done.clone() };
    let label = match &current {
        Some(id) => crate::sounds::name_of(id),
        None if attention => "nenhum".into(),
        None => format!("padrão do tema ({})", crate::sounds::name_of(default_done)),
    };
    ui.horizontal(|ui| {
        let mut pick: Option<Option<String>> = None;
        egui::ComboBox::from_id_salt(id).selected_text(label).width(300.0).show_ui(ui, |ui| {
            if attention {
                if ui.selectable_label(current.is_none(), "nenhum").clicked() {
                    pick = Some(None);
                }
            } else if ui.selectable_label(current.is_none(), format!("padrão do tema ({})", crate::sounds::name_of(default_done))).clicked() {
                pick = Some(None);
            }
            for s in crate::sounds::catalog() {
                let r = ui.selectable_label(current.as_deref() == Some(&s.id), format!("{}  ·  {}", s.name, s.desc));
                if r.clicked() {
                    pick = Some(Some(s.id.clone()));
                }
                if r.hovered() && ui.input(|i| i.key_pressed(egui::Key::Space)) {
                    if let Some(p) = crate::sounds::resolve(&s.id) {
                        crate::notify::play(&p, app.cfg.sound.volume);
                    }
                }
            }
        });
        if let Some(v) = pick {
            if attention {
                app.cfg.sound.attention = v;
            } else {
                app.cfg.sound.done = v;
            }
            let _ = app.cfg.save();
        }
        let play = current.clone().or_else(|| (!attention).then(|| default_done.to_string()));
        if ui.add_enabled(play.is_some(), egui::Button::new("▶ ouvir")).clicked() {
            match play.as_deref().and_then(crate::sounds::resolve) {
                Some(p) => crate::notify::play(&p, app.cfg.sound.volume),
                None => app.toast("Esse som ainda não foi baixado (rode o install.sh do AIC)", ToastKind::Warn),
            }
        }
        if ui.button("📁 pasta").on_hover_text("Coloque .mp3/.ogg/.wav aqui e eles aparecem na lista").clicked() {
            let _ = open::that_detached(aic_core::paths::sounds_dir());
        }
    });
}

#[allow(dead_code)]
fn path_field(ui: &mut Ui, value: &mut Option<std::path::PathBuf>, hint: &str) {
    let mut s = value.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if ui.add(TextEdit::singleline(&mut s).hint_text(hint).desired_width(360.0)).changed() {
        *value = (!s.trim().is_empty()).then(|| s.trim().into());
    }
}

/// Captura a próxima combinação de teclas pra ação em edição.
fn capture(app: &mut App, ui: &Ui) {
    let Some(a) = app.settings.capturing else { return };
    let got = ui.input_mut(|i| {
        let mut out = None;
        i.events.retain(|e| {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e {
                if out.is_none() {
                    out = Some((*key, *modifiers));
                }
                return false;
            }
            true
        });
        out
    });
    if let Some((key, mods)) = got {
        match key {
            Key::Escape if mods.is_none() => {}
            Key::Backspace if mods.is_none() => {
                app.keymap.set(a, None);
                let _ = app.keymap.save();
            }
            _ => {
                app.keymap.set(a, Some(KeyboardShortcut::new(mods, key)));
                let _ = app.keymap.save();
                app.toast(&format!("{} → {}", a.meta().name, app.keymap.label(a)), ToastKind::Ok);
            }
        }
        app.settings.capturing = None;
    }
}
