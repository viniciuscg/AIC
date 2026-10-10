//! Feature flags. Cada módulo pode ser ligado/desligado globalmente (`[modules]` no
//! config.toml) e por tema (`[themes.<id>.modules]`). Precedência: tema > global > padrão.
//!
//! Desligar um módulo some com a aba, os atalhos e as ações dele.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Module {
    /// Board de tarefas (kanban) e captura rápida.
    Board,
    /// Notas por bloco.
    Notes,
    /// Aba de worktrees e worktree por sessão.
    Worktrees,
    /// Aba de tokens, limites e histórico de custo.
    Usage,
    /// Cards do Notion no board e no bloco.
    Notion,
    /// Threads do Slack nas tarefas/blocos.
    Slack,
    /// "Abrir card": link do card → bloco montado (worktrees, Claudes, shell, notas).
    Launcher,
    /// Terminais sobre tmux: sobrevivem ao fechar o app.
    Tmux,
    /// Vários workspaces.
    Workspaces,
    /// Mascote animado.
    Mascot,
    /// Efeito animado de fundo.
    Background,
    /// Som quando uma sessão termina / precisa de você.
    Sound,
    /// Notificação do sistema.
    Notifications,
    /// Barra de status estilo statusline do Claude.
    Statusbar,
    /// Dicas de atalho (tecla líder + barra de dicas).
    Hints,
}

pub struct ModuleInfo {
    pub module: Module,
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub group: &'static str,
    pub default: bool,
    /// Só funciona com estes ligados.
    pub requires: &'static [Module],
}

use Module::*;

pub const ALL: &[ModuleInfo] = &[
    ModuleInfo { module: Tmux, id: "tmux", name: "Persistência (tmux)", desc: "Terminais rodam num servidor tmux próprio do AIC: fechar a janela não mata nada, e ao abrir ele reconecta.", group: "Sessões", default: true, requires: &[] },
    ModuleInfo { module: Workspaces, id: "workspaces", name: "Workspaces", desc: "Vários contextos de trabalho, cada um com seus blocos e layout.", group: "Sessões", default: true, requires: &[] },
    ModuleInfo { module: Launcher, id: "launcher", name: "Abrir card", desc: "Cola o link do card e o AIC monta o bloco: worktrees, um Claude por repo, shell e notas.", group: "Sessões", default: true, requires: &[] },
    ModuleInfo { module: Board, id: "board", name: "Board de tarefas", desc: "Kanban com tarefas locais e cards, captura rápida com marcadores (#repo @hoje !alta).", group: "Organização", default: true, requires: &[] },
    ModuleInfo { module: Notes, id: "notes", name: "Notas", desc: "Anotações em Markdown por bloco, salvas junto do bloco.", group: "Organização", default: true, requires: &[] },
    ModuleInfo { module: Worktrees, id: "worktrees", name: "Worktrees", desc: "Aba de worktrees e opção de isolar cada sessão num worktree.", group: "Organização", default: true, requires: &[] },
    ModuleInfo { module: Usage, id: "usage", name: "Tokens e custo", desc: "Limites de 5h/semana e histórico de custo lido dos transcripts.", group: "Organização", default: true, requires: &[] },
    ModuleInfo { module: Notion, id: "notion", name: "Notion", desc: "Cards do Notion no board e no bloco (via Claude Code ou token).", group: "Integrações", default: true, requires: &[] },
    ModuleInfo { module: Slack, id: "slack", name: "Slack", desc: "Threads do Slack vinculadas a tarefas e blocos (via Claude Code).", group: "Integrações", default: true, requires: &[] },
    ModuleInfo { module: Statusbar, id: "statusbar", name: "Barra de status", desc: "Modelo, contexto, branch, limites e custo da sessão em foco.", group: "Interface", default: true, requires: &[] },
    ModuleInfo { module: Hints, id: "hints", name: "Dicas de atalho", desc: "Tecla líder com menu de atalhos e barra de dicas no rodapé.", group: "Interface", default: true, requires: &[] },
    ModuleInfo { module: Mascot, id: "mascot", name: "Mascote", desc: "Mascote animado conforme o estado de cada sessão.", group: "Interface", default: true, requires: &[] },
    ModuleInfo { module: Background, id: "background", name: "Fundo animado", desc: "Efeito de fundo do tema (poeira, bolhas…).", group: "Interface", default: true, requires: &[] },
    ModuleInfo { module: Sound, id: "sound", name: "Som", desc: "Toca um som quando uma sessão termina ou pede permissão.", group: "Avisos", default: true, requires: &[] },
    ModuleInfo { module: Notifications, id: "notifications", name: "Notificações", desc: "Notificação do sistema quando uma sessão termina ou precisa de você.", group: "Avisos", default: true, requires: &[] },
];

pub fn info(m: Module) -> &'static ModuleInfo {
    ALL.iter().find(|i| i.module == m).expect("módulo registrado")
}

pub fn by_id(id: &str) -> Option<Module> {
    ALL.iter().find(|i| i.id == id).map(|i| i.module)
}

/// Overrides por id (`"notion" = false`).
pub type Flags = BTreeMap<String, bool>;

/// Ligado? tema > global > padrão, e só se as dependências também estiverem.
pub fn enabled(m: Module, global: &Flags, theme: Option<&Flags>) -> bool {
    let i = info(m);
    let own = theme
        .and_then(|t| t.get(i.id).copied())
        .or_else(|| global.get(i.id).copied())
        .unwrap_or(i.default);
    own && i.requires.iter().all(|r| enabled(*r, global, theme))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedencia() {
        let mut g = Flags::new();
        assert!(enabled(Module::Notion, &g, None));
        g.insert("notion".into(), false);
        assert!(!enabled(Module::Notion, &g, None));
        let mut t = Flags::new();
        t.insert("notion".into(), true);
        assert!(enabled(Module::Notion, &g, Some(&t)));
    }

    #[test]
    fn ids_unicos_e_resolvem() {
        let mut seen = std::collections::HashSet::new();
        for i in ALL {
            assert!(seen.insert(i.id));
            assert_eq!(by_id(i.id), Some(i.module));
        }
    }
}
