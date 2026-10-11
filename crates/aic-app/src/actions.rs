//! Toda ação do app passa por aqui: id, nome, atalho padrão. A paleta de comandos e
//! o catálogo de binds nas configurações são gerados desta lista — nunca ficam velhos.
//!
//! Atalhos do usuário ficam em `~/.config/aic/keybinds.toml` (`acao = "Ctrl+Shift+N"`).

use std::collections::BTreeMap;

use egui::{Key, KeyboardShortcut, Modifiers};
use aic_core::{Module, paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    // Sessões
    NewSession,
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
    ViewThemeHelp,
    Palette,
    // Visual
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CycleThemeVariant,
    SwitchTheme,
    ToggleSidebar,
    ToggleHints,
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
            NewSession, NewShell, CloseSession, RenameSession, RestartSession, SendContext, NextSession, PrevSession, NextWaiting,
            FocusLeft, FocusRight, FocusUp, FocusDown,
        ];
        v.extend((1..=9).map(Goto));
        v.extend([
            ToggleLayout, CycleView, ToggleMaximize, ReleaseTerminal, NewBlock, CloseBlock, RenameBlock, CycleSplit, ToggleBlockSide,
            CollapseBlock, MoveBlockUp, MoveBlockDown, FocusNotes, OpenCard, QuickAdd, LinkTaskToSession, Workspaces, NewWorkspace,
            NextWorkspace, PrevWorkspace, ViewSessions, ViewBoard, ViewWorktrees, ViewUsage, ViewSettings, ViewThemeHelp, Palette, ZoomIn,
            ZoomOut, ZoomReset, CycleThemeVariant, SwitchTheme, ToggleSidebar, ToggleHints, Quit,
        ]);
        v
    }

    /// Módulo de que a ação depende: com ele desligado a ação fica inerte e some dos menus.
    pub fn module(self) -> Option<Module> {
        use Action::*;
        match self {
            QuickAdd | LinkTaskToSession | SendContext | ViewBoard => Some(Module::Board),
            ViewWorktrees => Some(Module::Worktrees),
            ViewUsage => Some(Module::Usage),
            Workspaces | NewWorkspace | NextWorkspace | PrevWorkspace => Some(Module::Workspaces),
            FocusNotes => Some(Module::Notes),
            OpenCard => Some(Module::Launcher),
            ToggleHints => Some(Module::Hints),
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
            NewSession => ("new_session", "Novo Claude no bloco atual (escolhe pasta/worktree)", "Sessões", ks(CS, Key::N)),
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
            ToggleBlockSide => ("toggle_block_side", "Mostrar/esconder o painel do bloco (card, notas, tarefas)", "Blocos", ks(CS, Key::I)),
            CollapseBlock => ("collapse_block", "Recolher/expandir o bloco", "Blocos", ks(CA, Key::C)),
            MoveBlockUp => ("move_block_up", "Subir o bloco na lista", "Blocos", ks(CA, Key::ArrowUp)),
            MoveBlockDown => ("move_block_down", "Descer o bloco na lista", "Blocos", ks(CA, Key::ArrowDown)),
            FocusNotes => ("focus_notes", "Escrever nas notas do bloco", "Blocos", ks(CS, Key::E)),
            OpenCard => ("open_card", "Abrir card (link → bloco montado com worktrees e Claudes)", "Blocos", ks(CS, Key::J)),
            QuickAdd => ("quick_add", "Nova tarefa (captura rápida)", "Tarefas", ks(CS, Key::Space)),
            LinkTaskToSession => ("link_task", "Vincular uma tarefa ao terminal em foco", "Tarefas", ks(CS, Key::K)),
            Workspaces => ("workspaces", "Trocar de workspace", "Workspaces", ks(CS, Key::O)),
            NewWorkspace => ("new_workspace", "Criar workspace", "Workspaces", ks(CA, Key::O)),
            NextWorkspace => ("next_workspace", "Ir pro próximo workspace", "Workspaces", ks(CS, Key::PageDown)),
            PrevWorkspace => ("prev_workspace", "Ir pro workspace anterior", "Workspaces", ks(CS, Key::PageUp)),
            ViewSessions => ("view_sessions", "Ir pra aba Sessões", "Visões", ks(CS, Key::S)),
            ViewBoard => ("view_board", "Ir pra aba Board (tarefas)", "Visões", ks(CS, Key::B)),
            ViewWorktrees => ("view_worktrees", "Ir pra aba Worktrees", "Visões", ks(CS, Key::G)),
            ViewUsage => ("view_usage", "Ir pra aba Tokens (limites e custo)", "Visões", ks(CS, Key::U)),
            ViewSettings => ("view_settings", "Abrir configurações e atalhos", "Visões", ks(C, Key::Comma)),
            ViewThemeHelp => ("view_theme_help", "Abrir a ajuda do tema", "Visões", ks(N, Key::F1)),
            Palette => ("palette", "Paleta de comandos (busca ações, terminais e tarefas)", "Visões", ks(CS, Key::P)),
            ZoomIn => ("zoom_in", "Aumentar a fonte do terminal", "Visual", ks(CS, Key::Equals)),
            ZoomOut => ("zoom_out", "Diminuir a fonte do terminal", "Visual", ks(CS, Key::Minus)),
            ZoomReset => ("zoom_reset", "Voltar a fonte do terminal ao padrão", "Visual", ks(CS, Key::Num0)),
            CycleThemeVariant => ("cycle_theme", "Próxima variante de cor do tema", "Visual", ks(CS, Key::Y)),
            SwitchTheme => ("switch_theme", "Trocar de tema (AIC ⇄ Plankton)", "Visual", ks(CA, Key::Y)),
            ToggleSidebar => ("toggle_sidebar", "Mostrar/esconder a lista lateral (visualização Foco)", "Visual", ks(CS, Key::D)),
            ToggleHints => ("toggle_hints", "Mostrar/esconder a barra de dicas do rodapé", "Visual", ks(CS, Key::H)),
            Quit => ("quit", "Sair do AIC", "Visual", ks(CS, Key::Q)),
        };
        Meta { id, name, group, default }
    }
}

/// Atalhos de contexto (só valem numa visão, com o terminal sem foco). Vão pro catálogo.
pub const CONTEXT_KEYS: &[(&str, &str, &str)] = &[
    ("Tecla líder", "Ctrl+Space (configurável)", "Abre o menu de atalhos agrupados (funciona com o terminal focado)"),
    ("Tecla líder", "letra", "Entra no grupo / roda a ação"),
    ("Tecla líder", "Backspace", "Volta um nível"),
    ("Tecla líder", "Esc", "Fecha o menu"),
    ("Board", "j / k  ou  ↓ / ↑", "Navegar entre cartões"),
    ("Board", "h / l  ou  ← / →", "Navegar entre colunas"),
    ("Board", "Shift+H / Shift+L", "Mover o cartão de coluna"),
    ("Board", "Enter", "Abrir/criar a sessão da tarefa (no worktree certo)"),
    ("Board", "e", "Editar o título da tarefa"),
    ("Board", "x", "Concluir / reabrir"),
    ("Board", "d", "Arquivar"),
    ("Board", "Delete", "Apagar (pede confirmação)"),
    ("Board", "Space", "Marcar várias"),
    ("Board", "t  ou  n", "Nova tarefa nesta coluna"),
    ("Board", "Shift+P", "Promover pro Notion"),
    ("Board", "1 / 2 / 3", "Filtro: Tudo / Notion / Minhas"),
    ("Board", "/", "Buscar"),
    ("Board", "Esc", "Fechar o editor / desmarcar"),
    ("Editor de tarefa", "Tab", "Nova subtarefa a partir da linha"),
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
        let mut out = String::from("# Atalhos do AIC. Formato: acao = \"Ctrl+Shift+N\" (vazio desliga)\n");
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

    /// Texto de dica: "Fechar bloco  (Ctrl+Shift+X)".
    pub fn tip(&self, a: Action, text: &str) -> String {
        match self.label(a) {
            k if k.is_empty() => text.to_string(),
            k => format!("{text}  ({k})"),
        }
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
    fn toda_acao_tem_atalho() {
        for a in Action::all() {
            assert!(a.meta().default.is_some(), "sem atalho: {}", a.meta().id);
        }
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
