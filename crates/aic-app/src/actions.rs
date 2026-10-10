//! Toda ação do app passa por aqui: id, nome, atalho padrão. A paleta de comandos e
//! o catálogo de binds nas configurações são gerados desta lista — nunca ficam velhos.
//!
//! Atalhos do usuário ficam em `~/.config/aic/keybinds.toml` (`acao = "Ctrl+Shift+N"`).

use std::collections::BTreeMap;

use egui::{Key, KeyboardShortcut, Modifiers};
use aic_core::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    // Sessões
    NewSession,
    NewShell,
    CloseSession,
    RenameSession,
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
    FocusNotes,
    OpenCard,
    ToggleMaximize,
    ReleaseTerminal,
    // Tarefas
    QuickAdd,
    LinkTaskToSession,
    // Workspaces
    Workspaces,
    NewWorkspace,
    NextWorkspace,
    PrevWorkspace,
    // Visões
    ViewSessions,
    ViewBoard,
    ViewWorktrees,
    ViewUsage,
    ViewSettings,
    ViewForkHelp,
    Palette,
    // Visual
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CycleThemeVariant,
    SwitchFork,
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
const A: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };
const N: Modifiers = Modifiers::NONE;

fn ks(m: Modifiers, k: Key) -> Option<KeyboardShortcut> {
    Some(KeyboardShortcut::new(m, k))
}

impl Action {
    pub fn all() -> Vec<Action> {
        use Action::*;
        let mut v = vec![
            NewSession, NewShell, CloseSession, RenameSession, NextSession, PrevSession, NextWaiting, FocusLeft,
            FocusRight, FocusUp, FocusDown,
        ];
        v.extend((1..=9).map(Goto));
        v.extend([
            ToggleLayout, CycleView, NewBlock, CloseBlock, RenameBlock, CycleSplit, ToggleBlockSide, FocusNotes, OpenCard, ToggleMaximize, ReleaseTerminal, QuickAdd, LinkTaskToSession, Workspaces, NewWorkspace,
            NextWorkspace, PrevWorkspace, ViewSessions, ViewBoard, ViewWorktrees, ViewUsage, ViewSettings,
            ViewForkHelp, Palette, ZoomIn, ZoomOut, ZoomReset, CycleThemeVariant, SwitchFork, ToggleSidebar, Quit,
        ]);
        v
    }

    pub fn meta(self) -> Meta {
        use Action::*;
        let (id, name, group, default) = match self {
            NewSession => ("new_session", "Novo Claude no bloco (escolhe pasta/worktree)", "Sessões", ks(CS, Key::N)),
            NewShell => ("new_shell", "Novo terminal na mesma pasta (split)", "Sessões", ks(CS, Key::T)),
            CloseSession => ("close_session", "Fechar sessão", "Sessões", ks(CS, Key::W)),
            RenameSession => ("rename_session", "Renomear sessão", "Sessões", ks(CS, Key::R)),
            NextSession => ("next_session", "Próxima sessão", "Sessões", ks(C, Key::PageDown)),
            PrevSession => ("prev_session", "Sessão anterior", "Sessões", ks(C, Key::PageUp)),
            NextWaiting => ("next_waiting", "Ir pra próxima que precisa de você", "Sessões", ks(CS, Key::A)),
            FocusLeft => ("focus_left", "Foco à esquerda", "Sessões", ks(CS, Key::ArrowLeft)),
            FocusRight => ("focus_right", "Foco à direita", "Sessões", ks(CS, Key::ArrowRight)),
            FocusUp => ("focus_up", "Foco acima", "Sessões", ks(CS, Key::ArrowUp)),
            FocusDown => ("focus_down", "Foco abaixo", "Sessões", ks(CS, Key::ArrowDown)),
            Goto(n) => {
                // Blocos/Abas: vai pro bloco N; Foco/Grade: pra sessão N
                let key = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9]
                    [(n.clamp(1, 9) - 1) as usize];
                let id: &'static str = ["goto_1", "goto_2", "goto_3", "goto_4", "goto_5", "goto_6", "goto_7", "goto_8", "goto_9"]
                    [(n.clamp(1, 9) - 1) as usize];
                let name: &'static str = [
                    "Ir pro bloco/sessão 1", "Ir pro bloco/sessão 2", "Ir pro bloco/sessão 3", "Ir pro bloco/sessão 4", "Ir pro bloco/sessão 5",
                    "Ir pro bloco/sessão 6", "Ir pro bloco/sessão 7", "Ir pro bloco/sessão 8", "Ir pro bloco/sessão 9",
                ][(n.clamp(1, 9) - 1) as usize];
                (id, name, "Sessões", ks(A, key))
            }
            ToggleLayout => ("toggle_layout", "Alternar foco / grade", "Sessões", ks(CS, Key::L)),
            CycleView => ("cycle_view", "Próxima visualização (blocos → abas → foco → grade)", "Sessões", ks(CS, Key::V)),
            NewBlock => ("new_block", "Novo bloco (contexto)", "Blocos", ks(CS, Key::Enter)),
            CloseBlock => ("close_block", "Fechar bloco (todos os terminais dele)", "Blocos", ks(CS, Key::X)),
            RenameBlock => ("rename_block", "Renomear bloco", "Blocos", ks(CS, Key::F2)),
            CycleSplit => ("cycle_split", "Arranjo dos terminais do bloco (lado a lado → principal → empilhado → grade)", "Blocos", ks(CS, Key::Backslash)),
            ToggleBlockSide => ("toggle_block_side", "Painel do bloco (card, notas, tarefas)", "Blocos", ks(CS, Key::I)),
            FocusNotes => ("focus_notes", "Escrever nas notas do bloco", "Blocos", ks(CS, Key::E)),
            OpenCard => ("open_card", "Abrir card (link → bloco montado com worktrees e Claudes)", "Blocos", ks(CS, Key::J)),
            ToggleMaximize => ("toggle_maximize", "Maximizar sessão (zoom)", "Sessões", ks(CS, Key::M)),
            ReleaseTerminal => ("release_terminal", "Soltar o teclado do terminal", "Sessões", ks(CS, Key::Escape)),
            QuickAdd => ("quick_add", "Nova tarefa (captura rápida)", "Tarefas", ks(CS, Key::Space)),
            LinkTaskToSession => ("link_task", "Vincular tarefa à sessão", "Tarefas", ks(CS, Key::K)),
            Workspaces => ("workspaces", "Trocar de workspace", "Workspaces", ks(CS, Key::O)),
            NewWorkspace => ("new_workspace", "Novo workspace", "Workspaces", None),
            NextWorkspace => ("next_workspace", "Próximo workspace", "Workspaces", ks(CS, Key::PageDown)),
            PrevWorkspace => ("prev_workspace", "Workspace anterior", "Workspaces", ks(CS, Key::PageUp)),
            ViewSessions => ("view_sessions", "Aba Sessões", "Visões", ks(CS, Key::S)),
            ViewBoard => ("view_board", "Aba Board", "Visões", ks(CS, Key::B)),
            ViewWorktrees => ("view_worktrees", "Aba Worktrees", "Visões", ks(CS, Key::G)),
            ViewUsage => ("view_usage", "Aba Tokens", "Visões", ks(CS, Key::U)),
            ViewSettings => ("view_settings", "Configurações e binds", "Visões", ks(C, Key::Comma)),
            ViewForkHelp => ("view_fork_help", "Ajuda do fork", "Visões", ks(N, Key::F1)),
            Palette => ("palette", "Paleta de comandos", "Visões", ks(CS, Key::P)),
            ZoomIn => ("zoom_in", "Aumentar fonte", "Visual", ks(CS, Key::Equals)),
            ZoomOut => ("zoom_out", "Diminuir fonte", "Visual", ks(CS, Key::Minus)),
            ZoomReset => ("zoom_reset", "Fonte padrão", "Visual", ks(CS, Key::Num0)),
            CycleThemeVariant => ("cycle_theme", "Próxima variante de tema", "Visual", ks(CS, Key::Y)),
            SwitchFork => ("switch_theme", "Trocar de tema (AIC ⇄ Plankton)", "Visual", None),
            ToggleSidebar => ("toggle_sidebar", "Mostrar/esconder a lista lateral", "Visual", ks(CS, Key::D)),
            Quit => ("quit", "Sair", "Visual", ks(CS, Key::Q)),
        };
        Meta { id, name, group, default }
    }
}

/// Atalhos de contexto (só valem numa visão, com o terminal sem foco). Vão pro catálogo.
pub const CONTEXT_KEYS: &[(&str, &str, &str)] = &[
    ("Board", "j / k  ou  ↓ / ↑", "Navegar entre cartões"),
    ("Board", "h / l  ou  ← / →", "Navegar entre colunas"),
    ("Board", "Shift+H / Shift+L", "Mover o cartão de coluna"),
    ("Board", "Enter", "Abrir/criar a sessão da tarefa (no worktree certo)"),
    ("Board", "e", "Editar a tarefa"),
    ("Board", "x", "Concluir / reabrir"),
    ("Board", "d", "Arquivar"),
    ("Board", "Space", "Marcar várias"),
    ("Board", "t  ou  n", "Nova tarefa nesta coluna"),
    ("Board", "Shift+P", "Promover pro Notion"),
    ("Board", "1 / 2 / 3", "Filtro: Tudo / Notion / Minhas"),
    ("Board", "/", "Buscar"),
    ("Editor de tarefa", "Tab", "Nova subtarefa a partir da linha"),
    ("Editor de tarefa", "Ctrl+Enter", "Salvar e fechar"),
    ("Captura rápida", "#repo @hoje !alta +workspace ^DEV-1 ~tag", "Marcadores inline"),
    ("Captura rápida", "Enter / Esc", "Criar / cancelar"),
    ("Sessões", "t", "Nova tarefa (com o terminal sem foco)"),
    ("Sessões", "Clique no terminal", "Devolver o teclado ao terminal"),
    ("Terminal", "Ctrl+Shift+C / Ctrl+Shift+V", "Copiar / colar"),
    ("Terminal", "Shift+Enter", "Nova linha no prompt do Claude"),
    ("Terminal", "Ctrl+clique", "Abrir link"),
    ("Worktrees", "n", "Novo worktree no repo selecionado"),
    ("Worktrees", "Enter", "Abrir sessão no worktree"),
    ("Worktrees", "Delete", "Remover worktree (a branch fica)"),
];

pub struct Keymap {
    map: BTreeMap<Action, Option<KeyboardShortcut>>,
}

impl Keymap {
    pub fn path() -> std::path::PathBuf {
        paths::config_dir().join("keybinds.toml")
    }

    pub fn load() -> Self {
        let mut map: BTreeMap<Action, Option<KeyboardShortcut>> =
            Action::all().into_iter().map(|a| (a, a.meta().default)).collect();
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            if let Ok(table) = text.parse::<toml::Table>() {
                for a in Action::all() {
                    if let Some(v) = table.get(a.meta().id).and_then(|v| v.as_str()) {
                        map.insert(a, if v.is_empty() || v == "nenhum" { None } else { parse_shortcut(v) });
                    }
                }
            }
        }
        Self { map }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let mut out = String::from("# Atalhos do Plankton. Formato: acao = \"Ctrl+Shift+N\" (vazio desliga)\n");
        for (a, s) in &self.map {
            if *s != a.meta().default {
                let v = s.map(|s| format_shortcut(&s)).unwrap_or_default();
                out.push_str(&format!("{} = \"{}\"\n", a.meta().id, v));
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
        }
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

    #[test]
    fn ids_unicos() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::all() {
            assert!(seen.insert(a.meta().id));
        }
    }
}
