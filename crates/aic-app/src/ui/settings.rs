//! Aba Config: geral, som, Layne, cache e o catálogo de atalhos (editável).

use egui::{FontId, Key, KeyboardShortcut, RichText, ScrollArea, TextEdit, Ui};

use super::widgets;
use crate::actions::{self, Action, CONTEXT_KEYS};
use crate::app::{App, ToastKind};

#[derive(Default)]
pub struct SettingsState {
    /// Ação esperando a próxima combinação de teclas (atalho direto).
    pub capturing: Option<Action>,
    /// Ação esperando a próxima tecla de prefixo.
    pub capturing_prefix: Option<Action>,
    /// Gravando a tecla líder.
    pub capturing_leader: bool,
    /// Gravando o prefixo (Ctrl+B).
    pub capturing_prefix_key: bool,
    /// Busca no topo da Config.
    pub search: String,
    /// Pedido de foco na busca (`Ctrl+B /` ou `/`).
    pub focus_search: bool,
}

impl SettingsState {
    pub fn is_capturing(&self) -> bool {
        self.capturing.is_some()
            || self.capturing_prefix.is_some()
            || self.capturing_leader
            || self.capturing_prefix_key
    }

    fn stop_capture(&mut self) {
        self.capturing = None;
        self.capturing_prefix = None;
        self.capturing_leader = false;
        self.capturing_prefix_key = false;
    }
}

/// Filtro da busca: todas as palavras precisam aparecer (sem diferenciar maiúsculas e acentos).
struct Find {
    words: Vec<String>,
}

fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

impl Find {
    fn new(q: &str) -> Self {
        Self {
            words: fold(q).split_whitespace().map(str::to_string).collect(),
        }
    }

    fn empty(&self) -> bool {
        self.words.is_empty()
    }

    fn hit(&self, texts: &[&str]) -> bool {
        let hay = fold(&texts.join(" "));
        self.words.iter().all(|w| hay.contains(w))
    }

    /// Linha visível: busca vazia, a seção bate ou a própria linha bate.
    fn row(&self, section: &str, label: &str, extra: &str) -> bool {
        self.empty() || self.hit(&[section, label, extra])
    }

    fn any(&self, section: &str, rows: &[(&str, &str)]) -> bool {
        self.empty() || rows.iter().any(|(l, e)| self.row(section, l, e))
    }
}

const GERAL: &[(&str, &str)] = &[
    ("Visualização", "blocos abas foco grade layout sessões"),
    ("Assistente", "primeiro uso setup"),
    ("Fonte do terminal", "font tamanho zoom"),
    ("Escala da interface", "zoom ui tamanho"),
    ("Pastas de projetos", "repos repositórios diretórios git"),
];
const SOM: &[(&str, &str)] = &[
    ("Som", "volume tocar"),
    ("Quando terminar", "som terminou acabou done"),
    ("Quando precisar de você", "som permissão atenção"),
    ("Notificação", "notify-send desktop sistema foco"),
];
const CLAUDE: &[(&str, &str)] = &[("Injeção por sessão", "settings plugin hooks statusline")];
const LAYNE: &[(&str, &str)] = &[
    (
        "Sessão do Layne",
        "assistente agente iniciar modelo conversa",
    ),
    (
        "Microfone",
        "mic áudio voz gravar entrada pulseaudio pipewire",
    ),
    (
        "Transcrição",
        "whisper api openai groq comando idioma voz texto",
    ),
];
const CACHE: &[(&str, &str)] = &[("Limpar cache", "tmux sessões salvar layout restaurar")];
const SOBRE: &[(&str, &str)] = &[("Versão e pastas", "versão config dados eventos caminho")];

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    capture(app, ui);

    // ── busca (fora da rolagem, sempre à mão) ──
    let typing = ui.ctx().egui_wants_keyboard_input();
    if !typing
        && app.modal.is_none()
        && !app.settings.is_capturing()
        && ui.input_mut(|i| actions::consume_exact(i, egui::Modifiers::NONE, Key::Slash))
    {
        app.settings.focus_search = true;
        ui.input_mut(|i| {
            i.events
                .retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"))
        });
    }
    ui.horizontal(|ui| {
        widgets::heading(ui, &format!("{}  Configurações", widgets::I_GEAR), &theme);
        ui.add_space(12.0);
        let r = ui.add(
            TextEdit::singleline(&mut app.settings.search)
                .hint_text("/  pesquisar (fonte, som, prefixo, workspace…)")
                .desired_width(420.0)
                .margin(egui::vec2(8.0, 5.0)),
        );
        if std::mem::take(&mut app.settings.focus_search) {
            r.request_focus();
        }
        if r.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            app.settings.search.clear();
            r.surrender_focus();
        }
        if !app.settings.search.is_empty()
            && widgets::icon_button(ui, widgets::I_CLOSE, "Limpar busca", "Limpar busca (Esc)")
                .clicked()
        {
            app.settings.search.clear();
        }
    });
    ui.label(
        widgets::subtle(
            format!(
                "salvas em {}",
                aic_core::paths::pretty(&aic_core::Config::path())
            ),
            &theme,
        )
        .small(),
    );
    ui.add_space(4.0);

    let f = Find::new(&app.settings.search);
    let mut shown = 0;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(1080.0);

        if f.any("Geral", GERAL) {
            shown += 1;
            section(ui, &theme, "Geral");
            egui::Grid::new("geral").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                let row = |i: usize| f.row("Geral", GERAL[i].0, GERAL[i].1);
                if row(0) {
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
                }
                if row(1) {
                    ui.label("Assistente");
                    if ui.button("Rodar o assistente de primeiro uso de novo").clicked() {
                        app.phase = crate::app::Phase::Setup(Default::default());
                    }
                    ui.end_row();
                }
                if row(2) {
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
                }
                if row(3) {
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
                }
                if row(4) {
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
                }
            });
        }

        let mods: Vec<(&str, &str)> = aic_core::modules::ALL.iter().map(|m| (m.name, m.desc)).collect();
        if f.any("Módulos", &mods) {
            shown += 1;
            section(ui, &theme, "Módulos");
            modules_panel(app, ui, &f);
        }

        if f.any("Som e notificações", SOM) {
            shown += 1;
            section(ui, &theme, "Som e notificações");
            egui::Grid::new("som").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                let row = |i: usize| f.row("Som e notificações", SOM[i].0, SOM[i].1);
                if row(0) {
                    ui.label("Som");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut app.cfg.sound.enabled, "tocar som");
                        ui.add(egui::Slider::new(&mut app.cfg.sound.volume, 0.0..=1.5).text("volume"));
                    });
                    ui.end_row();
                }
                if row(1) {
                    ui.label("Quando terminar");
                    sound_picker(app, ui, "som-done", false, crate::sounds::DEFAULT_DONE);
                    ui.end_row();
                }
                if row(2) {
                    ui.label("Quando precisar de você");
                    sound_picker(app, ui, "som-attn", true, crate::sounds::DEFAULT_DONE);
                    ui.end_row();
                }
                if row(3) {
                    ui.label("Notificação");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut app.cfg.notify.desktop, "notificação do sistema (notify-send)");
                        ui.checkbox(&mut app.cfg.notify.only_when_unfocused, "só se a janela estiver sem foco ou a sessão escondida");
                        if ui.button("testar notificação").clicked() {
                            crate::notify::desktop(&format!("{} — teste", crate::brand::NAME), "Se você está vendo isso, funciona.", crate::app::icon_path());
                        }
                    });
                    ui.end_row();
                }
            });
            if ui.button("Salvar").clicked() {
                match app.cfg.save() {
                    Ok(()) => app.toast("Configurações salvas", ToastKind::Ok),
                    Err(e) => app.toast(&format!("{e}"), ToastKind::Err),
                }
            }
        }

        if f.any("Claude Code", CLAUDE) {
            shown += 1;
            section(ui, &theme, "Claude Code");
            egui::Grid::new("claude").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                if f.row("Claude Code", CLAUDE[0].0, CLAUDE[0].1) {
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
                        ui.label(widgets::subtle("hooks e statusline (que só grava os dados da barra de cima, sem imprimir no terminal) valem só nas sessões abertas pelo AIC; seu ~/.claude não é alterado.", &theme).small());
                    });
                    ui.end_row();
                }
            });
        }

        if f.any("Layne (assistente)", LAYNE) {
            shown += 1;
            section(ui, &theme, "Layne (assistente)");
            crate::layne::settings(app, ui, |i| f.row("Layne (assistente)", LAYNE[i].0, LAYNE[i].1));
        }

        if f.any("Cache (estilo tmux)", CACHE) {
            shown += 1;
            section(ui, &theme, "Cache (estilo tmux)");
            ui.label(widgets::subtle("Workspaces, sessões e layout são salvos continuamente. Ao fechar, você escolhe se mantém pro próximo início.", &theme));
            if ui.button("Limpar cache agora (as sessões abertas continuam até fechar)").clicked() {
                let _ = app.store.clear_cache();
                let _ = app.store.mark_cache(true);
                app.persist_ws();
                app.toast("Cache limpo", ToastKind::Ok);
            }
        }

        if binds(app, ui, &f) {
            shown += 1;
        }

        if f.any("Sobre", SOBRE) {
            shown += 1;
            section(ui, &theme, "Sobre");
            ui.label(format!("{} {} · {}", crate::brand::NAME, env!("CARGO_PKG_VERSION"), crate::brand::TAGLINE));
            for (label, p) in [
                ("config", aic_core::paths::config_dir()),
                ("dados", aic_core::paths::data_dir()),
                ("eventos/statusline", aic_core::paths::state_dir()),
            ] {
                ui.label(widgets::subtle(format!("{label}: {}", aic_core::paths::pretty(&p)), &theme).small());
            }
        }

        if shown == 0 {
            ui.add_space(24.0);
            ui.label(RichText::new(format!("Nada encontrado para \"{}\"", app.settings.search.trim())).color(theme.dim));
        }
    });
}

/// Seção Atalhos: prefixo, tecla líder, cada ação (atalho direto + tecla de prefixo) e os de contexto.
/// Devolve se mostrou alguma coisa.
fn binds(app: &mut App, ui: &mut Ui, f: &Find) -> bool {
    let theme = app.theme.clone();
    let sec = "Atalhos";
    let pfx = super::prefix::label(app);
    let rows: Vec<(String, String)> = Action::all()
        .into_iter()
        .map(|a| {
            (
                a.meta().name.to_string(),
                format!(
                    "{} {} {} {pfx} {}",
                    a.meta().group,
                    a.meta().id,
                    app.keymap.label(a),
                    app.keymap.prefix_label(a)
                ),
            )
        })
        .collect();
    let ctx_hit = CONTEXT_KEYS
        .iter()
        .any(|(g, k, d)| f.row(sec, d, &format!("{g} {k}")));
    let fixed_hit = f.row(sec, "Prefixo", "tmux ctrl+b tecla")
        || f.row(sec, "Tecla líder", "catálogo menu which-key");
    if !f.empty() && !ctx_hit && !fixed_hit && !rows.iter().any(|(n, e)| f.row(sec, n, e)) {
        return false;
    }
    section(ui, &theme, sec);
    ui.horizontal(|ui| {
        if ui.button("Restaurar padrões").clicked() {
            app.keymap.reset();
            let _ = app.keymap.save();
        }
        ui.label(
            widgets::subtle(
                format!(
                    "editáveis · salvos em {}",
                    aic_core::paths::pretty(&actions::Keymap::path())
                ),
                &theme,
            )
            .small(),
        );
    });
    ui.add_space(6.0);
    let mut group = "";
    egui::Grid::new("binds")
        .num_columns(3)
        .striped(true)
        .spacing([20.0, 6.0])
        .show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("Atalho direto").small().color(theme.dim));
            ui.label(
                RichText::new(format!("Depois do prefixo ({pfx})"))
                    .small()
                    .color(theme.dim),
            );
            ui.end_row();

            // prefixo e tecla líder (ficam no config.toml, não no keybinds.toml)
            if f.row(sec, "Prefixo", "tmux ctrl+b tecla")
                || f.row(sec, "Tecla líder", "catálogo menu which-key")
            {
                ui.label(
                    RichText::new("PREFIXO E CATÁLOGO")
                        .font(FontId::monospace(11.0))
                        .color(theme.accent),
                );
                ui.end_row();
            }
            if f.row(sec, "Prefixo", "tmux ctrl+b tecla") {
                ui.label("Prefixo (aperte, solte e aperte a tecla da ação)");
                ui.horizontal(|ui| {
                    if app.settings.capturing_prefix_key {
                        ui.label(
                            RichText::new("aperte a combinação… (Esc cancela)").color(theme.warn),
                        );
                    } else {
                        widgets::kbd(ui, &pfx, &theme);
                    }
                    if labeled(ui.small_button("alterar"), "Alterar o prefixo").clicked() {
                        app.settings.stop_capture();
                        app.settings.capturing_prefix_key = true;
                    }
                    let default = aic_core::Config::default().prefix;
                    if app.cfg.prefix != default
                        && labeled(ui.small_button("padrão"), "Prefixo padrão").clicked()
                    {
                        app.cfg.prefix = default;
                        let _ = app.cfg.save();
                    }
                });
                ui.label("");
                ui.end_row();
            }
            if f.row(sec, "Tecla líder", "catálogo menu which-key") {
                ui.label("Catálogo de atalhos (tecla líder)");
                ui.horizontal(|ui| {
                    if app.settings.capturing_leader {
                        ui.label(
                            RichText::new("aperte a combinação… (Esc cancela)").color(theme.warn),
                        );
                    } else {
                        widgets::kbd(ui, &super::leader::label(app), &theme);
                    }
                    if labeled(ui.small_button("alterar"), "Alterar a tecla líder").clicked() {
                        app.settings.stop_capture();
                        app.settings.capturing_leader = true;
                    }
                    let default = aic_core::Config::default().leader;
                    if app.cfg.leader != default
                        && labeled(ui.small_button("padrão"), "Tecla líder padrão").clicked()
                    {
                        app.cfg.leader = default;
                        let _ = app.cfg.save();
                    }
                    if !app.cfg.on(aic_core::Module::Hints) {
                        ui.label(widgets::subtle("(módulo Catálogo desligado)", &theme).small());
                    }
                });
                ui.label("");
                ui.end_row();
            }

            for (a, (name, extra)) in Action::all().into_iter().zip(rows.iter()) {
                if !f.row(sec, name, extra) {
                    continue;
                }
                let m = a.meta();
                if m.group != group {
                    group = m.group;
                    ui.label(
                        RichText::new(group.to_uppercase())
                            .font(FontId::monospace(11.0))
                            .color(theme.accent),
                    );
                    ui.end_row();
                }
                let off = !a.enabled(&app.cfg);
                ui.label(if off {
                    RichText::new(format!("{}  (módulo desligado)", m.name)).color(theme.faint)
                } else {
                    RichText::new(m.name)
                });
                ui.horizontal(|ui| {
                    if app.settings.capturing == Some(a) {
                        ui.label(
                            RichText::new("aperte a combinação… (Esc cancela, Backspace limpa)")
                                .color(theme.warn),
                        );
                    } else {
                        widgets::kbd(ui, &app.keymap.label(a), &theme);
                    }
                    if labeled(
                        ui.small_button("alterar"),
                        &format!("Alterar atalho: {}", m.name),
                    )
                    .clicked()
                    {
                        app.settings.stop_capture();
                        app.settings.capturing = Some(a);
                    }
                    if app.keymap.get(a) != m.default
                        && labeled(
                            ui.small_button("padrão"),
                            &format!("Atalho padrão: {}", m.name),
                        )
                        .clicked()
                    {
                        app.keymap.set(a, m.default);
                        let _ = app.keymap.save();
                    }
                });
                ui.horizontal(|ui| {
                    if app.settings.capturing_prefix == Some(a) {
                        ui.label(
                            RichText::new("aperte a tecla… (Esc cancela, Backspace limpa)")
                                .color(theme.warn),
                        );
                    } else {
                        let keys = app.keymap.prefix_keys(a);
                        if keys.is_empty() {
                            widgets::kbd(ui, "", &theme);
                        }
                        for k in keys.to_vec() {
                            widgets::kbd(ui, &k, &theme);
                        }
                    }
                    if labeled(
                        ui.small_button("alterar"),
                        &format!("Alterar tecla de prefixo: {}", m.name),
                    )
                    .clicked()
                    {
                        app.settings.stop_capture();
                        app.settings.capturing_prefix = Some(a);
                    }
                    let default: Vec<String> =
                        a.prefix_default().iter().map(|s| s.to_string()).collect();
                    if app.keymap.prefix_keys(a) != default.as_slice()
                        && labeled(
                            ui.small_button("padrão"),
                            &format!("Tecla de prefixo padrão: {}", m.name),
                        )
                        .clicked()
                    {
                        app.keymap.set_prefix(a, default);
                        let _ = app.keymap.save();
                    }
                });
                ui.end_row();
            }
            let mut ctx_group = "";
            for (g, keys, desc) in CONTEXT_KEYS {
                if !f.row(sec, desc, &format!("{g} {keys}")) {
                    continue;
                }
                if *g != ctx_group {
                    ctx_group = g;
                    ui.label(
                        RichText::new(format!("{} (contexto)", g.to_uppercase()))
                            .font(FontId::monospace(11.0))
                            .color(theme.accent),
                    );
                    ui.end_row();
                }
                ui.label(*desc);
                widgets::kbd(ui, keys, &theme);
                ui.label("");
                ui.end_row();
            }
        });
    true
}

/// Feature flags: liga/desliga cada módulo.
fn modules_panel(app: &mut App, ui: &mut Ui, f: &Find) {
    use aic_core::modules::ALL;
    let theme = app.theme.clone();
    ui.label(
        widgets::subtle(
            "Desligar um módulo some com a aba, os atalhos e as ações dele.",
            &theme,
        )
        .small(),
    );
    ui.add_space(4.0);
    let mut group = "";
    let mut changed = false;
    egui::Grid::new("mods")
        .num_columns(3)
        .striped(true)
        .spacing([14.0, 6.0])
        .show(ui, |ui| {
            for m in ALL {
                if !f.row("Módulos", m.name, &format!("{} {}", m.desc, m.group)) {
                    continue;
                }
                if m.group != group {
                    group = m.group;
                    ui.label(
                        RichText::new(group.to_uppercase())
                            .font(FontId::monospace(11.0))
                            .color(theme.accent),
                    );
                    ui.end_row();
                }
                let mut on = aic_core::modules::enabled(m.module, &app.cfg.modules);
                if ui.checkbox(&mut on, RichText::new(m.name)).changed() {
                    app.cfg.modules.insert(m.id.to_string(), on);
                    changed = true;
                }
                ui.label(widgets::subtle(m.desc, &theme).small());
                let on = app.cfg.on(m.module);
                ui.label(
                    RichText::new(if on { "● ativo" } else { "○ desligado" })
                        .small()
                        .color(if on { theme.ok } else { theme.faint }),
                );
                ui.end_row();
            }
        });
    if changed {
        let _ = app.cfg.save();
    }
}

fn section(ui: &mut Ui, theme: &crate::theme::Theme, title: &str) {
    ui.add_space(16.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .font(FontId::monospace(13.0))
            .color(theme.accent_hi)
            .strong(),
    );
    ui.separator();
}

/// Lista de sons com ▶ pra ouvir cada um. `attention` = campo "precisa de você" (aceita "nenhum").
fn sound_picker(app: &mut App, ui: &mut Ui, id: &str, attention: bool, default_done: &str) {
    let current = if attention {
        app.cfg.sound.attention.clone()
    } else {
        app.cfg.sound.done.clone()
    };
    let label = match &current {
        Some(id) => crate::sounds::name_of(id),
        None if attention => "nenhum".into(),
        None => format!("padrão ({})", crate::sounds::name_of(default_done)),
    };
    ui.horizontal(|ui| {
        let mut pick: Option<Option<String>> = None;
        egui::ComboBox::from_id_salt(id)
            .selected_text(label)
            .width(300.0)
            .show_ui(ui, |ui| {
                if attention {
                    if ui.selectable_label(current.is_none(), "nenhum").clicked() {
                        pick = Some(None);
                    }
                } else if ui
                    .selectable_label(
                        current.is_none(),
                        format!("padrão ({})", crate::sounds::name_of(default_done)),
                    )
                    .clicked()
                {
                    pick = Some(None);
                }
                for s in crate::sounds::catalog() {
                    let r = ui.selectable_label(
                        current.as_deref() == Some(&s.id),
                        format!("{}  ·  {}", s.name, s.desc),
                    );
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
        let play = current
            .clone()
            .or_else(|| (!attention).then(|| default_done.to_string()));
        if ui
            .add_enabled(play.is_some(), egui::Button::new("▶ ouvir"))
            .clicked()
        {
            match play.as_deref().and_then(crate::sounds::resolve) {
                Some(p) => crate::notify::play(&p, app.cfg.sound.volume),
                None => app.toast(
                    "Esse som não foi encontrado na pasta de sons",
                    ToastKind::Warn,
                ),
            }
        }
        if ui
            .button("📁 pasta")
            .on_hover_text("Coloque .mp3/.ogg/.wav aqui e eles aparecem na lista")
            .clicked()
        {
            let _ = open::that_detached(aic_core::paths::sounds_dir());
        }
    });
}

#[allow(dead_code)]
fn path_field(ui: &mut Ui, value: &mut Option<std::path::PathBuf>, hint: &str) {
    let mut s = value
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if ui
        .add(
            TextEdit::singleline(&mut s)
                .hint_text(hint)
                .desired_width(360.0),
        )
        .changed()
    {
        *value = (!s.trim().is_empty()).then(|| s.trim().into());
    }
}

/// Rótulo acessível pra botões repetidos ("alterar" em toda linha).
fn labeled(resp: egui::Response, label: &str) -> egui::Response {
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// Captura a próxima tecla/combinação pra ação em edição, a tecla líder ou o prefixo.
fn capture(app: &mut App, ui: &Ui) {
    if !app.settings.is_capturing() {
        return;
    }
    // tecla de prefixo de uma ação: uma tecla só, sem modificador (texto vence: `(`, `$`, `%`)
    if let Some(a) = app.settings.capturing_prefix {
        let events: Vec<egui::Event> = ui.input_mut(|i| {
            let mut out = vec![];
            i.events.retain(|e| match e {
                egui::Event::Key { .. } | egui::Event::Text(_) => {
                    out.push(e.clone());
                    false
                }
                _ => true,
            });
            out
        });
        if events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                }
            )
        }) {
            app.settings.capturing_prefix = None;
            app.keymap.set_prefix(a, vec![]);
            let _ = app.keymap.save();
            return;
        }
        match super::prefix::token(&events) {
            None => {}
            Some(Err(())) => app.settings.capturing_prefix = None,
            Some(Ok(k)) => {
                app.settings.capturing_prefix = None;
                let before = app.keymap.prefix_action(&k).filter(|b| *b != a);
                app.keymap.set_prefix(a, vec![k.clone()]);
                let _ = app.keymap.save();
                let pfx = super::prefix::label(app);
                match before {
                    Some(b) => app.toast(
                        &format!(
                            "{pfx} {k} → {} (saiu de \"{}\")",
                            a.meta().name,
                            b.meta().name
                        ),
                        ToastKind::Ok,
                    ),
                    None => app.toast(&format!("{pfx} {k} → {}", a.meta().name), ToastKind::Ok),
                }
            }
        }
        return;
    }
    let got = ui.input_mut(|i| {
        let mut out = None;
        i.events.retain(|e| {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = e
            {
                if out.is_none() {
                    out = Some((*key, *modifiers));
                }
                return false;
            }
            !matches!(e, egui::Event::Text(_))
        });
        out
    });
    let Some((key, mods)) = got else { return };
    let s = KeyboardShortcut::new(mods, key);
    let is = |cfg: &str| {
        actions::parse_shortcut(cfg)
            .is_some_and(|l| l.logical_key == key && actions::same_mods(l.modifiers, mods))
    };
    let clash = Action::all().into_iter().find(|a| {
        app.keymap
            .get(*a)
            .is_some_and(|k| k.logical_key == key && actions::same_mods(k.modifiers, mods))
    });
    if app.settings.capturing_leader || app.settings.capturing_prefix_key {
        let leader = app.settings.capturing_leader;
        app.settings.stop_capture();
        if key == Key::Escape && mods.is_none() {
            return;
        }
        let other_taken = if leader {
            is(&app.cfg.prefix)
        } else {
            is(&app.cfg.leader)
        };
        match clash {
            _ if other_taken => app.toast(
                &format!(
                    "{} já é {}",
                    actions::format_shortcut(&s),
                    if leader {
                        "o prefixo"
                    } else {
                        "a tecla líder"
                    }
                ),
                ToastKind::Warn,
            ),
            _ if mods.is_none() => app.toast(
                "Use uma combinação com Ctrl ou Alt (senão a tecla some do terminal)",
                ToastKind::Warn,
            ),
            Some(a) => app.toast(
                &format!(
                    "{} já é de \"{}\"",
                    actions::format_shortcut(&s),
                    a.meta().name
                ),
                ToastKind::Warn,
            ),
            None if leader => {
                app.cfg.leader = actions::format_shortcut(&s);
                let _ = app.cfg.save();
                app.toast(&format!("Tecla líder → {}", app.cfg.leader), ToastKind::Ok);
            }
            None => {
                app.cfg.prefix = actions::format_shortcut(&s);
                let _ = app.cfg.save();
                app.toast(&format!("Prefixo → {}", app.cfg.prefix), ToastKind::Ok);
            }
        }
        return;
    }
    let Some(a) = app.settings.capturing else {
        return;
    };
    app.settings.capturing = None;
    match key {
        Key::Escape if mods.is_none() => {}
        Key::Backspace if mods.is_none() => {
            app.keymap.set(a, None);
            let _ = app.keymap.save();
        }
        _ if is(&app.cfg.leader) => app.toast("Essa combinação é a tecla líder", ToastKind::Warn),
        _ if is(&app.cfg.prefix) => app.toast("Essa combinação é o prefixo", ToastKind::Warn),
        _ => {
            app.keymap.set(a, Some(s));
            let _ = app.keymap.save();
            app.toast(
                &format!("{} → {}", a.meta().name, app.keymap.label(a)),
                ToastKind::Ok,
            );
        }
    }
}
