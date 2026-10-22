//! Toda ação do app passa por aqui: id, nome, atalho direto e tecla de prefixo. A paleta de
//! comandos, o catálogo (tecla líder) e a lista de binds na Config saem desta lista.
//!
//! Dois jeitos de disparar uma ação:
//! - **atalho direto**: `Ctrl+Shift+N`;
//! - **prefixo** (estilo tmux): aperta `Ctrl+B`, solta, aperta a tecla (`Ctrl+B x`).
//!
//! Os do usuário ficam em `~/.config/aic/keybinds.toml`: `acao = "Ctrl+Shift+N"` no topo e
//! `acao = "x"` (ou `["h", "←"]`) na tabela `[prefix]`. Vazio desliga.

use std::collections::BTreeMap;

use egui::{Key, KeyboardShortcut, Modifiers};
use aic_core::{Module, paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    // Foco
    /// Fecha o que está em foco: terminal, bloco vazio ou a aba atual.
    CloseFocused,
    // Sessões
    NewSession,
    /// Painel do Layne (assistente).
    ToggleLayne,
    /// Gravar/parar a voz pro Layne.
    LayneVoice,
    NewShell,
    CloseSession,
    RenameSession,
    RestartSession,
    SendContext,
    NextSession,
    PrevSession,
    NextWaiting,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    Goto(u8),
    ToggleLayout,
    CycleView,
    NewBlock,
    CloseBlock,
    RenameBlock,
    CycleSplit,
    ToggleBlockSide,
    CollapseBlock,
    MoveBlockUp,
    MoveBlockDown,
    ToggleNotes,
    ToggleMaximize,
    ReleaseTerminal,
    // Tarefas
    QuickAdd,
    // Workspaces
    Workspaces,
    NewWorkspace,
    NextWorkspace,
    PrevWorkspace,
    /// Workspace N (como o `$mod+N` do i3).
    GotoWorkspace(u8),
    /// Volta pro workspace anterior (vai e volta).
    LastWorkspace,
    RenameWorkspace,
    // Visões
    ViewSessions,
    ViewWorktrees,
    ViewChanges,
    ViewUsage,
    ViewSettings,
    ViewHelp,
    NextView,
    PrevView,
    SearchSettings,
    Catalog,
    Palette,
    // Visual
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleSidebar,
    Quit,
}

pub struct Meta {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub default: Option<KeyboardShortcut>,
}

const CS: Modifiers = Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };
const C: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const CA: Modifiers = Modifiers { alt: true, ctrl: true, shift: false, mac_cmd: false, command: true };
const A: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };
const N: Modifiers = Modifiers::NONE;

fn ks(m: Modifiers, k: Key) -> Option<KeyboardShortcut> {
    Some(KeyboardShortcut::new(m, k))
}

impl Action {
    pub fn all() -> Vec<Action> {
        use Action::*;
        let mut v = vec![
            CloseFocused, NewSession, ToggleLayne, LayneVoice, NewShell, CloseSession, RenameSession, RestartSession, SendContext, NextSession, PrevSession, NextWaiting,
            FocusLeft, FocusRight, FocusUp, FocusDown,
        ];
        v.extend((1..=9).map(Goto));
        v.extend([
            ToggleLayout, CycleView, ToggleMaximize, ReleaseTerminal, NewBlock, CloseBlock, RenameBlock, CycleSplit, ToggleBlockSide,
            CollapseBlock, MoveBlockUp, MoveBlockDown, ToggleNotes, QuickAdd, Workspaces, NewWorkspace,
            NextWorkspace, PrevWorkspace,
        ]);
        v.extend((1..=9).map(GotoWorkspace));
        v.extend([
            LastWorkspace, RenameWorkspace, ViewSessions, ViewWorktrees, ViewChanges, ViewUsage, ViewSettings, ViewHelp, NextView, PrevView,
            SearchSettings, Catalog, Palette, ZoomIn, ZoomOut, ZoomReset, ToggleSidebar, Quit,
        ]);
        v
    }

    /// Módulo de que a ação depende: com ele desligado a ação fica inerte e some dos menus.
    pub fn module(self) -> Option<Module> {
        use Action::*;
        match self {
            QuickAdd | SendContext => Some(Module::Tasks),
            ViewWorktrees => Some(Module::Worktrees),
            ViewChanges => Some(Module::Changes),
            ViewUsage => Some(Module::Usage),
            Workspaces | NewWorkspace | NextWorkspace | PrevWorkspace | GotoWorkspace(_) | LastWorkspace | RenameWorkspace => Some(Module::Workspaces),
            ToggleNotes => Some(Module::Notes),
            ToggleLayne | LayneVoice => Some(Module::Layne),
            Catalog => Some(Module::Hints),
            _ => None,
        }
    }

    /// Ação disponível com a config atual (módulo ligado).
    pub fn enabled(self, cfg: &aic_core::Config) -> bool {
        self.module().is_none_or(|m| cfg.on(m))
    }

    pub fn meta(self) -> Meta {
        use Action::*;
        let (id, name, group, default) = match self {
            CloseFocused => ("close_focused", "Fechar o que está em foco (terminal, bloco vazio ou a aba)", "Foco", None),
            NewSession => ("new_session", "Novo Claude no bloco atual (escolhe pasta/worktree)", "Sessões", ks(CS, Key::N)),
            ToggleLayne => ("toggle_layne", "Layne: abrir/fechar o assistente (ou pegar o teclado dele)", "Layne", ks(CA, Key::L)),
            LayneVoice => ("layne_voice", "Layne: falar (grava; de novo para e manda)", "Layne", ks(CA, Key::V)),
            NewShell => ("new_shell", "Novo terminal ao lado, na mesma pasta (split)", "Sessões", ks(CS, Key::T)),
            CloseSession => ("close_session", "Fechar o terminal em foco", "Sessões", ks(CS, Key::W)),
            RenameSession => ("rename_session", "Renomear o terminal em foco", "Sessões", ks(CS, Key::R)),
            RestartSession => ("restart_session", "Reiniciar o terminal em foco (o Claude retoma a conversa)", "Sessões", ks(CS, Key::F5)),
            SendContext => ("send_context", "Mandar o contexto da tarefa pro Claude em foco", "Sessões", ks(CA, Key::K)),
            NextSession => ("next_session", "Ir pro próximo terminal", "Sessões", ks(C, Key::PageDown)),
            PrevSession => ("prev_session", "Ir pro terminal anterior", "Sessões", ks(C, Key::PageUp)),
            NextWaiting => ("next_waiting", "Ir pro próximo terminal que precisa de você", "Sessões", ks(CS, Key::A)),
            FocusLeft => ("focus_left", "Mover o foco pro terminal à esquerda", "Sessões", ks(CS, Key::ArrowLeft)),
            FocusRight => ("focus_right", "Mover o foco pro terminal à direita", "Sessões", ks(CS, Key::ArrowRight)),
            FocusUp => ("focus_up", "Mover o foco pro terminal de cima", "Sessões", ks(CS, Key::ArrowUp)),
            FocusDown => ("focus_down", "Mover o foco pro terminal de baixo", "Sessões", ks(CS, Key::ArrowDown)),
            Goto(n) => {
                // Blocos/Abas: vai pro bloco N; Foco/Grade: pro terminal N
                let i = (n.clamp(1, 9) - 1) as usize;
                let key = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9][i];
                let id: &'static str = ["goto_1", "goto_2", "goto_3", "goto_4", "goto_5", "goto_6", "goto_7", "goto_8", "goto_9"][i];
                let name: &'static str = [
                    "Ir pro bloco 1 (ou terminal 1 no Foco/Grade)",
                    "Ir pro bloco 2 (ou terminal 2 no Foco/Grade)",
                    "Ir pro bloco 3 (ou terminal 3 no Foco/Grade)",
                    "Ir pro bloco 4 (ou terminal 4 no Foco/Grade)",
                    "Ir pro bloco 5 (ou terminal 5 no Foco/Grade)",
                    "Ir pro bloco 6 (ou terminal 6 no Foco/Grade)",
                    "Ir pro bloco 7 (ou terminal 7 no Foco/Grade)",
                    "Ir pro bloco 8 (ou terminal 8 no Foco/Grade)",
                    "Ir pro bloco 9 (ou terminal 9 no Foco/Grade)",
                ][i];
                (id, name, "Sessões", ks(A, key))
            }
            ToggleLayout => ("toggle_layout", "Alternar foco ⇄ grade", "Sessões", ks(CS, Key::L)),
            // Ctrl+Shift+V é o colar do terminal: a troca de visualização fica no F
            CycleView => ("cycle_view", "Trocar a visualização (blocos → abas → foco → grade)", "Sessões", ks(CS, Key::F)),
            ToggleMaximize => ("toggle_maximize", "Maximizar/restaurar o terminal em foco", "Sessões", ks(CS, Key::M)),
            ReleaseTerminal => ("release_terminal", "Soltar o teclado do terminal (atalhos de tela voltam a valer)", "Sessões", ks(CS, Key::Escape)),
            NewBlock => ("new_block", "Novo bloco (contexto)", "Blocos", ks(CS, Key::Enter)),
            CloseBlock => ("close_block", "Fechar o bloco e todos os terminais dele", "Blocos", ks(CS, Key::X)),
            RenameBlock => ("rename_block", "Renomear o bloco", "Blocos", ks(CS, Key::F2)),
            CycleSplit => ("cycle_split", "Mudar o arranjo dos terminais (lado a lado → principal → empilhado → grade)", "Blocos", ks(CS, Key::Backslash)),
            ToggleBlockSide => ("toggle_block_side", "Mostrar/esconder o painel do bloco (tarefas)", "Blocos", ks(CS, Key::I)),
            CollapseBlock => ("collapse_block", "Recolher/expandir o bloco", "Blocos", ks(CA, Key::C)),
            MoveBlockUp => ("move_block_up", "Subir o bloco na lista", "Blocos", ks(CA, Key::ArrowUp)),
            MoveBlockDown => ("move_block_down", "Descer o bloco na lista", "Blocos", ks(CA, Key::ArrowDown)),
            ToggleNotes => ("toggle_notes", "Notas da sessão em foco: abrir (e escrever) / fechar", "Sessões", ks(CS, Key::E)),
            QuickAdd => ("quick_add", "Nova tarefa (captura rápida)", "Tarefas", ks(CS, Key::Space)),
            Workspaces => ("workspaces", "Trocar de workspace", "Workspaces", ks(CS, Key::O)),
            NewWorkspace => ("new_workspace", "Criar workspace", "Workspaces", ks(CA, Key::O)),
            NextWorkspace => ("next_workspace", "Ir pro próximo workspace", "Workspaces", ks(CS, Key::PageDown)),
            PrevWorkspace => ("prev_workspace", "Ir pro workspace anterior", "Workspaces", ks(CS, Key::PageUp)),
            GotoWorkspace(n) => {
                let i = (n.clamp(1, 9) - 1) as usize;
                let key = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9][i];
                let id: &'static str = ["workspace_1", "workspace_2", "workspace_3", "workspace_4", "workspace_5", "workspace_6", "workspace_7", "workspace_8", "workspace_9"][i];
                let name: &'static str = [
                    "Ir pro workspace 1",
                    "Ir pro workspace 2",
                    "Ir pro workspace 3",
                    "Ir pro workspace 4",
                    "Ir pro workspace 5",
                    "Ir pro workspace 6",
                    "Ir pro workspace 7",
                    "Ir pro workspace 8",
                    "Ir pro workspace 9",
                ][i];
                (id, name, "Workspaces", ks(CA, key))
            }
            LastWorkspace => ("last_workspace", "Voltar pro workspace anterior (vai e volta)", "Workspaces", None),
            RenameWorkspace => ("rename_workspace", "Renomear o workspace atual", "Workspaces", None),
            ViewSessions => ("view_sessions", "Ir pra aba Sessões", "Visões", ks(CS, Key::S)),
            ViewWorktrees => ("view_worktrees", "Ir pra aba Worktrees", "Visões", ks(CS, Key::G)),
            ViewChanges => ("view_changes", "Ir pra aba Changes (arquivos mudados e diff)", "Visões", ks(CA, Key::G)),
            ViewUsage => ("view_usage", "Ir pra aba Tokens (limites e custo)", "Visões", ks(CS, Key::U)),
            ViewSettings => ("view_settings", "Abrir configurações e atalhos", "Visões", ks(C, Key::Comma)),
            ViewHelp => ("view_help", "Abrir o manual", "Visões", ks(N, Key::F1)),
            NextView => ("next_view", "Próxima aba da barra (Sessões → Worktrees → …)", "Visões", ks(C, Key::Tab)),
            PrevView => ("prev_view", "Aba anterior da barra", "Visões", ks(CS, Key::Tab)),
            SearchSettings => ("search_settings", "Pesquisar nas configurações", "Visões", None),
            Catalog => ("catalog", "Catálogo de atalhos (o mesmo da tecla líder)", "Visões", None),
            Palette => ("palette", "Paleta de comandos (busca ações, terminais e tarefas)", "Visões", ks(CS, Key::P)),
            ZoomIn => ("zoom_in", "Aumentar a fonte do terminal", "Visual", ks(CS, Key::Equals)),
            ZoomOut => ("zoom_out", "Diminuir a fonte do terminal", "Visual", ks(CS, Key::Minus)),
            ZoomReset => ("zoom_reset", "Voltar a fonte do terminal ao padrão", "Visual", ks(CS, Key::Num0)),
            ToggleSidebar => ("toggle_sidebar", "Mostrar/esconder a lista lateral (visualização Foco)", "Visual", ks(CS, Key::D)),
            Quit => ("quit", "Sair do AIC", "Visual", ks(CS, Key::Q)),
        };
        Meta { id, name, group, default }
    }

    /// Teclas padrão depois do prefixo (`Ctrl+B`). Segue o tmux onde dá: `c` cria, `%` divide,
    /// `x` fecha, `z` maximiza, `n`/`p` próxima/anterior, `(`/`)` workspaces, `$` renomeia, `?` catálogo.
    pub fn prefix_default(self) -> &'static [&'static str] {
        use Action::*;
        match self {
            CloseFocused => &["x"],
            NewSession => &["c"],
            ToggleLayne => &["L"],
            LayneVoice => &["m"],
            NewShell => &["%", "\""],
            CloseBlock => &["&"],
            RenameSession => &["r"],
            RenameBlock => &["R"],
            NextWaiting => &["a"],
            ToggleMaximize => &["z"],
            NextSession => &["o"],
            PrevSession => &["O"],
            FocusLeft => &["h", "←"],
            FocusDown => &["j", "↓"],
            FocusUp => &["k", "↑"],
            FocusRight => &["l", "→"],
            ReleaseTerminal => &["["],
            CycleSplit => &["Space"],
            CycleView => &["v"],
            NewBlock => &["B"],
            MoveBlockUp => &["{"],
            MoveBlockDown => &["}"],
            ToggleBlockSide => &["i"],
            CollapseBlock => &["-"],
            ToggleNotes => &["e"],
            QuickAdd => &["t"],
            SendContext => &["K"],
            Workspaces => &["w"],
            NewWorkspace => &["W"],
            NextWorkspace => &[")"],
            PrevWorkspace => &["("],
            GotoWorkspace(n) => {
                const KEYS: [&[&str]; 9] = [&["1"], &["2"], &["3"], &["4"], &["5"], &["6"], &["7"], &["8"], &["9"]];
                KEYS[(n.clamp(1, 9) - 1) as usize]
            }
            LastWorkspace => &["Tab"],
            RenameWorkspace => &["$"],
            ViewSessions => &["s"],
            ViewWorktrees => &["g"],
            ViewChanges => &["G"],
            ViewUsage => &["u"],
            ViewSettings => &[","],
            NextView => &["n"],
            PrevView => &["p"],
            SearchSettings => &["/"],
            Catalog => &["?"],
            Palette => &[":"],
            ToggleSidebar => &["d"],
            _ => &[],
        }
    }
}

/// Normaliza uma tecla de prefixo vinda do usuário ("space" → "Space", "left" → "←").
pub fn norm_prefix_key(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().count() == 1 {
        return Some(t.to_string());
    }
    Some(
        match t.to_lowercase().as_str() {
            "space" | "espaço" => "Space",
            "tab" => "Tab",
            "enter" | "return" => "Enter",
            "left" | "arrowleft" => "←",
            "right" | "arrowright" => "→",
            "up" | "arrowup" => "↑",
            "down" | "arrowdown" => "↓",
            "pageup" => "PageUp",
            "pagedown" => "PageDown",
            "delete" => "Delete",
            _ => return None,
        }
        .to_string(),
    )
}

/// Atalhos de contexto (só valem numa visão, com o terminal sem foco). Vão pro catálogo.
pub const CONTEXT_KEYS: &[(&str, &str, &str)] = &[
    ("Prefixo", "Ctrl+B (configurável)", "Aperte, solte e aperte a tecla da ação (como no tmux); funciona com o terminal focado"),
    ("Prefixo", "Ctrl+B Ctrl+B", "Manda um Ctrl+B de verdade pro terminal"),
    ("Prefixo", "Ctrl+B Esc", "Cancela"),
    ("Tecla líder", "Ctrl+Space (configurável)", "Abre o catálogo de atalhos agrupados (funciona com o terminal focado)"),
    ("Tecla líder", "letra", "Entra no grupo / roda a ação"),
    ("Tecla líder", "Backspace", "Volta um nível"),
    ("Tecla líder", "Esc", "Fecha o menu"),
    ("Notas", "Esc", "Devolve o teclado pro terminal (a nota se salva sozinha)"),
    ("Captura rápida", "#repo @hoje !alta +workspace ^DEV-1 ~tag", "Marcadores inline"),
    ("Captura rápida", "Enter / Esc", "Criar / cancelar"),
    ("Captura rápida", "Alt+L", "Vincular (ou não) ao terminal em foco"),
    ("Nova sessão", "Alt+1 / Alt+2 / Alt+3", "Claude / Shell / Comando"),
    ("Nova sessão", "Alt+W", "Isolar num worktree"),
    ("Diálogos", "Enter / Esc", "Confirmar / cancelar"),
    ("Diálogos", "↑ / ↓  ou  Ctrl+N / Ctrl+P", "Escolher na lista"),
    ("Sessões", "t", "Nova tarefa (com o terminal sem foco)"),
    ("Sessões", "Clique no terminal", "Devolver o teclado ao terminal"),
    ("Sessões", "Enter (terminal encerrado)", "Reabrir o processo"),
    ("Terminal", "Ctrl+Shift+C / Ctrl+Shift+V", "Copiar / colar"),
    ("Terminal", "Shift+Enter", "Nova linha no prompt do Claude"),
    ("Terminal", "Ctrl+clique", "Abrir link"),
    ("Worktrees", "j / k  ou  ↓ / ↑", "Escolher worktree"),
    ("Worktrees", "n", "Novo worktree no repo selecionado"),
    ("Worktrees", "Enter", "Abrir sessão no worktree"),
    ("Worktrees", "Delete", "Remover worktree (a branch fica)"),
    ("Worktrees", "r", "Atualizar a lista"),
    ("Config", "alterar → combinação", "Gravar atalho (Esc cancela, Backspace limpa)"),
    ("Config", "/", "Pesquisar uma configuração"),
];

/// Consome do input a tecla com EXATAMENTE estes modificadores. O `consume_key` do egui
/// ignora Shift/Alt a mais (`j` também pega `Shift+J`), o que engolia os atalhos com Shift.
pub fn consume_exact(i: &mut egui::InputState, m: Modifiers, k: Key) -> bool {
    let mut hit = false;
    i.events.retain(|e| match e {
        egui::Event::Key { key, pressed: true, modifiers, .. } if !hit && *key == k && same_mods(*modifiers, m) => {
            hit = true;
            false
        }
        _ => true,
    });
    hit
}

/// Mesmos Ctrl/Shift/Alt (no Linux `command` espelha o Ctrl).
pub fn same_mods(a: Modifiers, b: Modifiers) -> bool {
    (a.ctrl || a.command) == (b.ctrl || b.command) && a.shift == b.shift && a.alt == b.alt
}

pub struct Keymap {
    map: BTreeMap<Action, Option<KeyboardShortcut>>,
    /// Teclas depois do prefixo.
    prefix: BTreeMap<Action, Vec<String>>,
}

fn prefix_defaults(a: Action) -> Vec<String> {
    a.prefix_default().iter().map(|s| s.to_string()).collect()
}

impl Keymap {
    pub fn path() -> std::path::PathBuf {
        paths::config_dir().join("keybinds.toml")
    }

    pub fn load() -> Self {
        let map: BTreeMap<Action, Option<KeyboardShortcut>> =
            Action::all().into_iter().map(|a| (a, a.meta().default)).collect();
        let prefix: BTreeMap<Action, Vec<String>> = Action::all().into_iter().map(|a| (a, prefix_defaults(a))).collect();
        let mut km = Self { map, prefix };
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            if let Ok(table) = text.parse::<toml::Table>() {
                for a in Action::all() {
                    if let Some(v) = table.get(a.meta().id).and_then(|v| v.as_str()) {
                        km.map.insert(a, if v.is_empty() || v == "nenhum" { None } else { parse_shortcut(v) });
                    }
                }
                if let Some(pt) = table.get("prefix").and_then(|v| v.as_table()) {
                    for a in Action::all() {
                        let keys: Option<Vec<String>> = match pt.get(a.meta().id) {
                            Some(toml::Value::String(s)) => Some(norm_prefix_key(s).into_iter().collect()),
                            Some(toml::Value::Array(v)) => Some(v.iter().filter_map(|x| x.as_str()).filter_map(norm_prefix_key).collect()),
                            _ => None,
                        };
                        if let Some(keys) = keys {
                            // a tecla sai da ação padrão que a usava
                            km.set_prefix(a, keys);
                        }
                    }
                }
            }
        }
        km
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let mut out = String::from("# Atalhos do AIC. Formato: acao = \"Ctrl+Shift+N\" (vazio desliga)\n");
        for (a, s) in &self.map {
            if *s != a.meta().default {
                let v = s.map(|s| format_shortcut(&s)).unwrap_or_default();
                out.push_str(&format!("{} = \"{}\"\n", a.meta().id, v));
            }
        }
        let changed: Vec<_> = self.prefix.iter().filter(|(a, k)| **k != prefix_defaults(**a)).collect();
        if !changed.is_empty() {
            out.push_str("\n# Teclas depois do prefixo (Ctrl+B). Formato: acao = \"x\" ou [\"h\", \"←\"]\n[prefix]\n");
            for (a, keys) in changed {
                let v = keys.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(", ");
                out.push_str(&format!("{} = [{v}]\n", a.meta().id));
            }
        }
        std::fs::create_dir_all(paths::config_dir())?;
        std::fs::write(Self::path(), out)?;
        Ok(())
    }

    pub fn get(&self, a: Action) -> Option<KeyboardShortcut> {
        self.map.get(&a).copied().flatten()
    }

    pub fn set(&mut self, a: Action, s: Option<KeyboardShortcut>) {
        // um atalho só pode ter uma ação
        if let Some(s) = s {
            for v in self.map.values_mut() {
                if *v == Some(s) {
                    *v = None;
                }
            }
        }
        self.map.insert(a, s);
    }

    pub fn reset(&mut self) {
        for a in Action::all() {
            self.map.insert(a, a.meta().default);
            self.prefix.insert(a, prefix_defaults(a));
        }
    }

    /// Teclas de prefixo da ação.
    pub fn prefix_keys(&self, a: Action) -> &[String] {
        self.prefix.get(&a).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Troca as teclas de prefixo de uma ação; quem usava a mesma tecla perde ela.
    pub fn set_prefix(&mut self, a: Action, keys: Vec<String>) {
        for k in &keys {
            for v in self.prefix.values_mut() {
                v.retain(|x| x != k);
            }
        }
        self.prefix.insert(a, keys);
    }

    /// Ação da tecla apertada depois do prefixo.
    pub fn prefix_action(&self, key: &str) -> Option<Action> {
        self.prefix.iter().find(|(_, keys)| keys.iter().any(|k| k == key)).map(|(a, _)| *a)
    }

    /// "x" / "h ←" (teclas de prefixo da ação, sem o prefixo).
    pub fn prefix_label(&self, a: Action) -> String {
        self.prefix_keys(a).join(" ")
    }

    /// Consome do input os atalhos disparados neste frame (o terminal não os recebe).
    pub fn pressed(&self, ctx: &egui::Context) -> Vec<Action> {
        let mut out = Vec::new();
        ctx.input_mut(|i| {
            // mais modificadores primeiro, pra Ctrl+Shift+X não ser engolido por Ctrl+X
            let mut entries: Vec<_> = self.map.iter().filter_map(|(a, s)| s.map(|s| (*a, s))).collect();
            entries.sort_by_key(|(_, s)| std::cmp::Reverse(modifier_count(s.modifiers)));
            for (a, s) in entries {
                if i.consume_shortcut(&s) {
                    out.push(a);
                }
            }
        });
        out
    }

    pub fn label(&self, a: Action) -> String {
        self.get(a).map(|s| format_shortcut(&s)).unwrap_or_default()
    }

    /// Texto de dica: "Fechar bloco  (Ctrl+Shift+X · prefixo &)".
    pub fn tip(&self, a: Action, text: &str) -> String {
        let mut keys = vec![];
        if let Some(k) = Some(self.label(a)).filter(|k| !k.is_empty()) {
            keys.push(k);
        }
        if let Some(k) = self.prefix_keys(a).first() {
            keys.push(format!("prefixo {k}"));
        }
        if keys.is_empty() { text.to_string() } else { format!("{text}  ({})", keys.join(" · ")) }
    }
}

fn modifier_count(m: Modifiers) -> u8 {
    m.ctrl as u8 + m.shift as u8 + m.alt as u8
}

pub fn format_shortcut(s: &KeyboardShortcut) -> String {
    let mut parts = Vec::new();
    if s.modifiers.ctrl || s.modifiers.command {
        parts.push("Ctrl".to_string());
    }
    if s.modifiers.alt {
        parts.push("Alt".into());
    }
    if s.modifiers.shift {
        parts.push("Shift".into());
    }
    let key = match s.logical_key {
        Key::Comma => ",".to_string(),
        Key::Equals => "=".to_string(),
        Key::Minus => "-".to_string(),
        Key::Num0 | Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 | Key::Num5 | Key::Num6 | Key::Num7 | Key::Num8 | Key::Num9 => {
            s.logical_key.name().to_string()
        }
        Key::ArrowLeft => "←".into(),
        Key::ArrowRight => "→".into(),
        Key::ArrowUp => "↑".into(),
        Key::ArrowDown => "↓".into(),
        k => k.name().to_string(),
    };
    parts.push(key);
    parts.join("+")
}

pub fn parse_shortcut(s: &str) -> Option<KeyboardShortcut> {
    let mut m = Modifiers::NONE;
    let mut key = None;
    for part in s.split('+').map(str::trim) {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "cmd" => {
                m.ctrl = true;
                m.command = true;
            }
            "shift" => m.shift = true,
            "alt" => m.alt = true,
            "," => key = Some(Key::Comma),
            "=" => key = Some(Key::Equals),
            "-" => key = Some(Key::Minus),
            "←" => key = Some(Key::ArrowLeft),
            "→" => key = Some(Key::ArrowRight),
            "↑" => key = Some(Key::ArrowUp),
            "↓" => key = Some(Key::ArrowDown),
            other => key = Key::from_name(part).or_else(|| Key::from_name(&other.to_uppercase())),
        }
    }
    key.map(|k| KeyboardShortcut::new(m, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta_dos_atalhos() {
        for a in Action::all() {
            if let Some(s) = a.meta().default {
                let txt = format_shortcut(&s);
                assert_eq!(parse_shortcut(&txt), Some(s), "{txt}");
            }
        }
    }

    #[test]
    fn sem_atalhos_repetidos() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::all() {
            if let Some(s) = a.meta().default {
                assert!(seen.insert(format_shortcut(&s)), "repetido: {}", format_shortcut(&s));
            }
        }
    }

    /// Toda ação dá pra disparar pelo teclado: atalho direto ou prefixo.
    #[test]
    fn toda_acao_tem_atalho() {
        for a in Action::all() {
            assert!(a.meta().default.is_some() || !a.prefix_default().is_empty(), "sem atalho: {}", a.meta().id);
        }
    }

    #[test]
    fn prefixo_sem_teclas_repetidas() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::all() {
            for k in a.prefix_default() {
                assert!(seen.insert(*k), "tecla de prefixo repetida: {k}");
                assert_eq!(norm_prefix_key(k).as_deref(), Some(*k), "tecla de prefixo inválida: {k}");
            }
        }
    }

    #[test]
    fn prefixo_ida_e_volta_no_toml() {
        let mut k = Keymap { map: BTreeMap::new(), prefix: Action::all().into_iter().map(|a| (a, prefix_defaults(a))).collect() };
        k.set_prefix(Action::CloseFocused, vec!["q".into(), "\"".into()]);
        assert_eq!(k.prefix_action("q"), Some(Action::CloseFocused));
        assert_eq!(k.prefix_action("x"), None);
        k.set_prefix(Action::NewSession, vec!["q".into()]);
        assert_eq!(k.prefix_keys(Action::CloseFocused), ["\""], "a tecla sai de quem tinha");
        assert_eq!(k.prefix_action("\""), Some(Action::CloseFocused), "aspas tiram o split antigo");
    }

    /// Ctrl+letra sem Shift é do terminal (Ctrl+C, Ctrl+D, Ctrl+R, Ctrl+L…), assim como
    /// Ctrl+Shift+C/V (copiar/colar). Nenhum atalho padrão pode roubar essas teclas.
    #[test]
    fn nao_rouba_teclas_do_terminal() {
        let letra = |k: Key| k.name().len() == 1 && k.name().chars().all(|c| c.is_ascii_alphabetic());
        for a in Action::all() {
            let Some(s) = a.meta().default else { continue };
            let m = s.modifiers;
            let txt = format_shortcut(&s);
            assert!(!(m.ctrl && !m.shift && !m.alt && letra(s.logical_key)), "rouba Ctrl+letra do terminal: {txt}");
            assert!(!(m.ctrl && m.shift && !m.alt && matches!(s.logical_key, Key::C | Key::V)), "rouba copiar/colar: {txt}");
            assert!(!(m.is_none() && letra(s.logical_key)), "letra sem modificador: {txt}");
        }
    }

    #[test]
    fn lider_padrao_nao_colide() {
        let leader = parse_shortcut(&aic_core::Config::default().leader).expect("líder válida");
        for a in Action::all() {
            if let Some(s) = a.meta().default {
                assert!(!(s.logical_key == leader.logical_key && same_mods(s.modifiers, leader.modifiers)), "colide com a líder: {}", a.meta().id);
            }
        }
    }

    #[test]
    fn consome_so_a_combinacao_exata() {
        let key = |m: Modifiers| egui::Event::Key { key: Key::L, physical_key: None, pressed: true, repeat: false, modifiers: m };
        let mut i = egui::InputState::default();
        i.events = vec![key(Modifiers::SHIFT)];
        assert!(!consume_exact(&mut i, Modifiers::NONE, Key::L), "l não pode engolir Shift+L");
        assert!(consume_exact(&mut i, Modifiers::SHIFT, Key::L));
        assert!(i.events.is_empty());
    }

    #[test]
    fn ids_unicos() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::all() {
            assert!(seen.insert(a.meta().id));
        }
    }
}
